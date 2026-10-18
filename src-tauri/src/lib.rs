pub mod big;
pub mod browser;
pub mod background;
pub mod backups;
pub mod catalog;
pub mod config;
pub mod download;
pub mod error;
pub mod extract;
pub mod game;
pub mod google;
pub mod game_files;
pub mod gamedata;
pub mod health;
pub mod journal;
pub mod layout;
pub mod launch;
pub mod install;
pub mod library;
pub mod maps;
pub mod options;
pub mod links;
pub mod index;
pub mod migrate;
pub mod plan;
pub mod presets;
pub mod record;
pub mod store;
pub mod update;
pub mod target;
pub mod version;

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use catalog::{Catalog, ExtraInfos, Item};
use error::{Error, Result};
use game::GameInstall;
use launch::Target;
use index::Entry;
use store::{AppConfig, Settings};

struct Inner {
    config_path: PathBuf,
    config: AppConfig,
    install: Option<GameInstall>,
    catalog: Option<Catalog>,
    /// Patch/addon/exe records per mod id, kept from `load_extras` (a launch needs the exe flags).
    extras: HashMap<String, ExtraInfos>,
}

struct AppState {
    inner: Mutex<Inner>,
    /// For the mod index (small text files).
    http: reqwest::Client,
    /// A Google access token and when to stop trusting it.
    google_access: Mutex<Option<(String, std::time::Instant)>>,
    /// True from the start of a launch until the game folder is restored again.
    launching: AtomicBool,
    /// Closing the window hides it (the launcher keeps running in the tray) instead of quitting.
    close_to_tray: AtomicBool,
    /// Installs in progress, by item id.
    downloads: std::sync::Mutex<HashMap<String, download::Cancel>>,
    /// No overall timeout (mods are gigabytes); stalls are detected per chunk instead.
    download_client: reqwest::Client,
    /// What the built-in browser's single download is doing; an install can wait for it to finish.
    browser_session: std::sync::Mutex<BrowserSession>,
    /// A preset the program was started to play (`--play <id>`), taken by the interface once it is ready.
    pending_play: std::sync::Mutex<Option<String>>,
}

/// State of the file the built-in browser is receiving, shared between the window's download hooks and
/// an install that was confirmed while the file was still arriving.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
enum BrowserSession {
    #[default]
    Idle,
    Downloading { target: PathBuf },
    Done { path: PathBuf },
    Failed { message: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Recovered {
    ops_undone: usize,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    installs: Vec<GameInstall>,
    active: Option<GameInstall>,
    settings: Settings,
    /// Set when an unfinished session (crash, power loss) was found and undone at startup.
    recovered: Option<Recovered>,
    /// Set when the folder still has GenLauncher 1.x data to move and clean up.
    legacy: Option<LegacySummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacySummary {
    mods: usize,
    bytes: u64,
    deletions: usize,
    notes: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogResponse {
    items: Vec<Item>,
    online: bool,
    /// Why the live index could not be used (the cached or bundled copy is shown instead).
    error: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LaunchFinished {
    error: Option<String>,
    warnings: Vec<String>,
    played_secs: u64,
}

fn settings_path(i: &GameInstall) -> PathBuf {
    store::data_dir(&i.dir).join("settings.json")
}

fn catalog_cache_path(i: &GameInstall) -> PathBuf {
    store::data_dir(&i.dir).join(format!("catalog-{}.json", i.game.key()))
}

fn require_install(inner: &Inner) -> Result<GameInstall> {
    inner.install.clone().ok_or_else(|| Error::msg("No game folder selected"))
}

fn known_dirs(inner: &Inner) -> Vec<PathBuf> {
    let mut extra: Vec<PathBuf> = inner.config.last_game_dir.clone().into_iter().collect();
    extra.extend(inner.config.extra_game_dirs.iter().cloned());
    extra
}

fn recover_install(install: Option<&GameInstall>) -> Option<Recovered> {
    let install = install?;
    match journal::recover(&install.dir) {
        Ok(Some(r)) => Some(Recovered { ops_undone: r.ops_undone, warnings: r.warnings }),
        Ok(None) => None,
        Err(e) => Some(Recovered { ops_undone: 0, warnings: vec![e.to_string()] }),
    }
}

fn bootstrap_of(inner: &Inner, installs: Vec<GameInstall>) -> Bootstrap {
    let settings = inner.install.as_ref().map(|i| store::load(&settings_path(i))).unwrap_or_default();
    let recovered = recover_install(inner.install.as_ref());
    let legacy = inner.install.as_ref().map(|i| migrate::plan(&i.dir)).filter(|p| !p.is_empty()).map(|p| LegacySummary {
        mods: p.mods_to_move,
        bytes: p.bytes_to_move,
        deletions: p.steps.iter().filter(|s| !matches!(s, migrate::Step::Move { .. })).count(),
        notes: p.notes.clone(),
    });
    Bootstrap { installs, active: inner.install.clone(), settings, recovered, legacy }
}

/// Each installed version of a mod remembers the page it was downloaded from (its source record),
/// so the UI can link back to the right page for every version.
fn attach_pages(game_dir: &std::path::Path, items: &mut [Item], disk: &[library::InstalledMod]) {
    for item in items.iter_mut().filter(|i| i.kind == catalog::Kind::Mod) {
        let Some(d) = disk.iter().find(|d| catalog::slug(&d.name) == item.id) else { continue };
        for v in item.versions.iter_mut().filter(|v| v.installed) {
            let Some(folder) = d.versions.iter().find(|n| n.eq_ignore_ascii_case(&v.v)) else { continue };
            let dir = library::mods_root(game_dir).join(&d.name).join(folder);
            v.page = record::read(&dir).and_then(|r| r.page_url).filter(|p| !p.trim().is_empty());
        }
    }
}

async fn scan_disk(dir: PathBuf) -> Result<Vec<library::InstalledMod>> {
    blocking(move || Ok(library::scan(&dir))).await
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::msg(e.to_string()))?
}

#[tauri::command]
async fn bootstrap(state: State<'_, AppState>) -> Result<Bootstrap> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("A game session is running"));
    }
    let mut inner = state.inner.lock().await;
    let installs = game::discover(&known_dirs(&inner));
    inner.install = inner.config.last_game_dir.as_deref().and_then(game::detect).or_else(|| installs.first().cloned());
    inner.catalog = None;
    inner.extras.clear();
    Ok(bootstrap_of(&inner, installs))
}

/// Switch to another game folder (from the discovered list or the folder picker).
#[tauri::command]
async fn select_install(state: State<'_, AppState>, dir: PathBuf) -> Result<Bootstrap> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game before switching installs"));
    }
    let install = game::detect(&dir).ok_or_else(|| {
        Error::msg("That folder does not look like a Generals or Zero Hour install (generals.exe, BINKW32.DLL and Window(ZH).big are needed)")
    })?;
    let mut inner = state.inner.lock().await;
    inner.config.last_game_dir = Some(install.dir.clone());
    if !inner.config.extra_game_dirs.contains(&install.dir) {
        inner.config.extra_game_dirs.push(install.dir.clone());
    }
    store::save(&inner.config_path, &inner.config)?;
    inner.install = Some(install);
    inner.catalog = None;
    inner.extras.clear();
    let installs = game::discover(&known_dirs(&inner));
    Ok(bootstrap_of(&inner, installs))
}

#[tauri::command]
async fn catalog_load(state: State<'_, AppState>) -> Result<CatalogResponse> {
    let install = require_install(&*state.inner.lock().await)?;
    let cache_path = catalog_cache_path(&install);

    let (catalog, online, error) = match catalog::fetch(&state.http, install.game.index_url()).await {
        Ok(c) => {
            if let Err(e) = store::save(&cache_path, &c) {
                log::warn!("could not cache catalog: {e}");
            }
            (c, true, None)
        }
        Err(e) => {
            // offline: the copy cached last time, or the one that shipped with this build
            let cached = store::load::<Catalog>(&cache_path);
            let c = if cached.index.mods.is_empty() { catalog::bundled(install.game) } else { cached };
            (c, false, Some(e.to_string()))
        }
    };

    let disk = scan_disk(install.dir.clone()).await?;
    let mut items = catalog::mod_items(&catalog, &disk);
    attach_pages(&install.dir, &mut items, &disk);
    state.inner.lock().await.catalog = Some(catalog);
    Ok(CatalogResponse { items, online, error })
}

/// Patches, addons and executables of one mod, from the index.
#[tauri::command]
async fn load_extras(state: State<'_, AppState>, mod_id: String) -> Result<Vec<Item>> {
    let (install, catalog) = {
        let inner = state.inner.lock().await;
        (require_install(&inner)?, inner.catalog.clone().ok_or_else(|| Error::msg("Catalog not loaded"))?)
    };
    let disk = scan_disk(install.dir.clone()).await?;
    let entry = catalog.index.mods.iter().find(|m| catalog::slug(&m.entry.name) == mod_id);

    if entry.is_none() && mod_id != catalog::ORIGINAL_GAME_ID {
        // Local-only mod: nothing remote, only what is on disk.
        let d = disk.iter().find(|d| catalog::slug(&d.name) == mod_id);
        return Ok(catalog::extra_items(&mod_id, &Default::default(), d, &disk));
    }

    let infos = catalog::extras_for(&catalog.index, entry);
    let wanted = entry.map(|e| e.entry.name.as_str()).unwrap_or(catalog::ORIGINAL_GAME);
    let d = disk.iter().find(|d| d.name.eq_ignore_ascii_case(wanted));
    let items = catalog::extra_items(&mod_id, &infos, d, &disk);
    // a launch needs the executable flags of the active mod's extras
    state.inner.lock().await.extras.insert(mod_id, infos);
    Ok(items)
}

/// Re-merge the catalog already in memory with what is on disk (no network): used after an install,
/// removal or move so the screen updates at once.
#[tauri::command]
async fn catalog_reload(state: State<'_, AppState>) -> Result<Vec<Item>> {
    let (install, catalog) = {
        let inner = state.inner.lock().await;
        (require_install(&inner)?, inner.catalog.clone().ok_or_else(|| Error::msg("Catalog not loaded"))?)
    };
    let disk = scan_disk(install.dir.clone()).await?;
    let mut items = catalog::mod_items(&catalog, &disk);
    attach_pages(&install.dir, &mut items, &disk);
    Ok(items)
}

/// Messages from the project. Best effort: offline (or nothing published) simply means no messages.
#[tauri::command]
async fn announcements_load(state: State<'_, AppState>) -> Result<Vec<index::Announcement>> {
    let text = match state.http.get(index::ANNOUNCEMENTS_URL).send().await.and_then(|r| r.error_for_status()) {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return Ok(Vec::new()),
    };
    Ok(index::parse_announcements(&text).unwrap_or_default())
}

/// Everything that belongs in a configuration file: the settings of each game found on this computer, plus the
/// look-and-feel the UI hands over.
async fn collect_config(state: &AppState, ui: serde_json::Value) -> Result<config::ConfigFile> {
    let inner = state.inner.lock().await;
    let mut games = std::collections::BTreeMap::new();
    for i in game::discover(&known_dirs(&inner)) {
        games.entry(i.game.key().to_string()).or_insert_with(|| store::load::<Settings>(&settings_path(&i)));
    }
    Ok(config::ConfigFile::new(games, ui))
}

/// Save the settings of every game in `cfg` that is installed here. Returns the games that were updated.
async fn apply_config(state: &AppState, cfg: &config::ConfigFile) -> Result<Vec<String>> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game before importing a configuration"));
    }
    let inner = state.inner.lock().await;
    let mut applied = Vec::new();
    for i in game::discover(&known_dirs(&inner)) {
        let key = i.game.key();
        if applied.iter().any(|a| a == key) {
            continue;
        }
        if let Some(settings) = cfg.games.get(key) {
            store::save(&settings_path(&i), settings)?;
            applied.push(key.to_string());
        }
    }
    Ok(applied)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportedConfig {
    /// Games whose settings were replaced.
    games: Vec<String>,
    ui: serde_json::Value,
}

/// Write the configuration to `path`. `ui` is the look-and-feel the UI wants to include.
#[tauri::command]
async fn config_export(state: State<'_, AppState>, path: String, ui: serde_json::Value) -> Result<Vec<String>> {
    let cfg = collect_config(&state, ui).await?;
    let json = cfg.to_json()?;
    let written: Vec<String> = cfg.games.keys().cloned().collect();
    blocking(move || Ok(std::fs::write(&path, json)?)).await?;
    Ok(written)
}

async fn backups_dir(state: &AppState) -> Result<PathBuf> {
    let inner = state.inner.lock().await;
    let dir = inner.config_path.parent().ok_or_else(|| Error::msg("The settings folder is unknown"))?;
    Ok(backups::dir(dir))
}

/// Keep the current configuration as a backup (what an import or restore is about to replace).
async fn back_up_current(state: &AppState, ui: serde_json::Value, label: &str) -> Result<backups::Backup> {
    let cfg = collect_config(state, ui).await?;
    let dir = backups_dir(state).await?;
    let label = label.to_string();
    blocking(move || backups::create(&dir, &cfg, &label)).await
}

/// Replace the settings of the installed games with those in the file at `path`. The current settings are
/// kept as a backup first. `current_ui` is the look the UI has now, for that backup.
#[tauri::command]
async fn config_import(state: State<'_, AppState>, path: String, current_ui: serde_json::Value) -> Result<ImportedConfig> {
    let text = blocking(move || {
        if std::fs::metadata(&path)?.len() > config::MAX_BYTES {
            return Err(Error::msg("That file is too large to be a GenLauncher configuration"));
        }
        Ok(std::fs::read_to_string(&path)?)
    })
    .await?;
    let cfg = config::ConfigFile::parse(&text)?;
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game before importing a configuration"));
    }
    back_up_current(&state, current_ui, "before-import").await?;
    let games = apply_config(&state, &cfg).await?;
    Ok(ImportedConfig { games, ui: cfg.ui })
}

// ---------- Google sign-in and Drive backups ----------

async fn google_session_path(state: &AppState) -> Result<PathBuf> {
    let inner = state.inner.lock().await;
    Ok(inner.config_path.parent().ok_or_else(|| Error::msg("The settings folder is unknown"))?.join("google.json"))
}

/// A valid access token: the cached one, or a fresh one from the saved sign-in.
async fn google_token(state: &AppState) -> Result<String> {
    if let Some((t, until)) = state.google_access.lock().await.as_ref() {
        if std::time::Instant::now() < *until {
            return Ok(t.clone());
        }
    }
    let path = google_session_path(state).await?;
    let session: google::Session = store::load(&path);
    if session.refresh_token.is_empty() {
        return Err(Error::msg("Sign in to Google first"));
    }
    let secret = google::require_client_secret()?;
    match google::refresh(&state.http, &google::Endpoints::google(), &secret, &session.refresh_token).await {
        Ok(t) => {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(t.expires_in.saturating_sub(60).max(30));
            *state.google_access.lock().await = Some((t.access_token.clone(), until));
            Ok(t.access_token)
        }
        Err(Error::SignedOut) => {
            let _ = std::fs::remove_file(&path);
            *state.google_access.lock().await = None;
            Err(Error::SignedOut)
        }
        Err(e) => Err(e),
    }
}

#[derive(Serialize)]
struct Account {
    email: String,
}

/// Who is signed in (read from this computer; no network).
#[tauri::command]
async fn google_status(state: State<'_, AppState>) -> Result<Option<Account>> {
    let session: google::Session = store::load(&google_session_path(&state).await?);
    Ok((!session.refresh_token.is_empty()).then_some(Account { email: session.email }))
}

/// Sign in with Google in the system browser. Returns when the person has finished (or gave up).
#[tauri::command]
async fn google_sign_in(app: AppHandle, state: State<'_, AppState>) -> Result<Account> {
    use tauri_plugin_opener::OpenerExt;
    let secret = google::require_client_secret()?;
    let ep = google::Endpoints::google();
    let verifier = google::random_string(64)?;
    let nonce = google::random_string(32)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| Error::msg(format!("Could not listen for Google's answer: {e}")))?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let url = google::auth_url(&ep, &redirect, &nonce, &google::pkce_challenge(&verifier))?;
    app.opener().open_url(url, None::<&str>).map_err(|e| Error::msg(format!("Could not open your browser: {e}")))?;

    let code = google::wait_for_redirect(listener, &nonce, std::time::Duration::from_secs(300)).await?;
    let tokens = google::exchange_code(&state.http, &ep, &secret, &code, &verifier, &redirect).await?;
    if !tokens.drive_granted() {
        google::revoke(&state.http, &ep, tokens.refresh_token.as_deref().unwrap_or(&tokens.access_token)).await;
        return Err(Error::msg("The permission to keep settings in your Google Drive was not ticked. Sign in again and tick it: GenLauncher can only see its own hidden folder there."));
    }
    let refresh_token = tokens.refresh_token.clone().ok_or_else(|| Error::msg("Google did not allow GenLauncher to stay signed in. Try again and accept every permission."))?;
    let email = tokens.id_token.as_deref().and_then(google::email_from_id_token).unwrap_or_default();
    store::save(&google_session_path(&state).await?, &google::Session { refresh_token, email: email.clone() })?;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(tokens.expires_in.saturating_sub(60).max(30));
    *state.google_access.lock().await = Some((tokens.access_token, until));
    Ok(Account { email })
}

/// Is a newer GenLauncher published? (The address and the signing key are in tauri.conf.json.)
#[tauri::command]
async fn update_check(app: AppHandle, state: State<'_, AppState>) -> Result<Option<update::Update>> {
    let cfg = update::settings(&app)?;
    update::check(&state.http, &cfg.endpoint, &app.package_info().version.to_string()).await
}

/// Download the newer version, check its signature, then run its setup program and quit so it can replace this one.
#[tauri::command]
async fn update_install(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("The game is running. Update after it has closed so your game folder is restored first."));
    }
    let cfg = update::settings(&app)?;
    let Some(up) = update::check(&state.http, &cfg.endpoint, &app.package_info().version.to_string()).await? else {
        return Ok(());
    };
    let setup = update::download(&state.download_client, &up, &cfg.pubkey).await?;
    // passive: shows progress only; /R starts the new version when it is done; /UPDATE keeps the user's choices
    std::process::Command::new(&setup).args(["/P", "/R", "/UPDATE"]).spawn().map_err(|e| Error::msg(format!("Could not start the update: {e}")))?;
    app.exit(0);
    Ok(())
}

/// Forget the sign-in on this computer and tell Google to drop it. Backups already in Drive stay there.
#[tauri::command]
async fn google_sign_out(state: State<'_, AppState>) -> Result<()> {
    let path = google_session_path(&state).await?;
    let session: google::Session = store::load(&path);
    *state.google_access.lock().await = None;
    let _ = std::fs::remove_file(&path);
    if !session.refresh_token.is_empty() {
        google::revoke(&state.http, &google::Endpoints::google(), &session.refresh_token).await;
    }
    Ok(())
}

/// Save the current configuration to the hidden app folder in Google Drive.
#[tauri::command]
async fn cloud_backup_create(state: State<'_, AppState>, ui: serde_json::Value, label: Option<String>) -> Result<google::CloudBackup> {
    let token = google_token(&state).await?;
    let cfg = collect_config(&state, ui).await?;
    let name = backups::file_name(cfg.exported_at, label.as_deref().unwrap_or("manual"));
    let ep = google::Endpoints::google();
    let made = google::upload(&state.http, &ep, &token, &name, &cfg.to_json()?).await?;
    if let Err(e) = google::prune(&state.http, &ep, &token, backups::KEEP).await {
        log::warn!("could not prune old cloud backups: {e}");
    }
    Ok(made)
}

#[tauri::command]
async fn cloud_backup_list(state: State<'_, AppState>) -> Result<Vec<google::CloudBackup>> {
    let token = google_token(&state).await?;
    google::list(&state.http, &google::Endpoints::google(), &token).await
}

#[tauri::command]
async fn cloud_backup_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    let token = google_token(&state).await?;
    google::delete(&state.http, &google::Endpoints::google(), &token, &id).await
}

/// Go back to a backup in Drive. The current settings are backed up (on this computer) first.
#[tauri::command]
async fn cloud_backup_restore(state: State<'_, AppState>, id: String, current_ui: serde_json::Value) -> Result<ImportedConfig> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game before restoring a backup"));
    }
    let token = google_token(&state).await?;
    let text = google::download(&state.http, &google::Endpoints::google(), &token, &id).await?;
    let cfg = config::ConfigFile::parse(&text)?;
    back_up_current(&state, current_ui, "before-restore").await?;
    let games = apply_config(&state, &cfg).await?;
    Ok(ImportedConfig { games, ui: cfg.ui })
}

/// Save the current configuration as a backup.
#[tauri::command]
async fn backup_create(state: State<'_, AppState>, ui: serde_json::Value) -> Result<backups::Backup> {
    back_up_current(&state, ui, "manual").await
}

#[tauri::command]
async fn backup_list(state: State<'_, AppState>) -> Result<Vec<backups::Backup>> {
    let dir = backups_dir(&state).await?;
    blocking(move || Ok(backups::list(&dir))).await
}

#[tauri::command]
async fn backup_delete(state: State<'_, AppState>, name: String) -> Result<()> {
    let dir = backups_dir(&state).await?;
    blocking(move || backups::delete(&dir, &name)).await
}

/// Go back to a backup. What is replaced is backed up first, so a restore can be undone the same way.
#[tauri::command]
async fn backup_restore(state: State<'_, AppState>, name: String, current_ui: serde_json::Value) -> Result<ImportedConfig> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game before restoring a backup"));
    }
    let dir = backups_dir(&state).await?;
    let cfg = {
        let dir = dir.clone();
        blocking(move || backups::read(&dir, &name)).await?
    };
    back_up_current(&state, current_ui, "before-restore").await?;
    let games = apply_config(&state, &cfg).await?;
    Ok(ImportedConfig { games, ui: cfg.ui })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OptionsResponse {
    path: String,
    /// False until the game has run once (or the options were saved here).
    exists: bool,
    graphics: options::Graphics,
}

async fn options_file(app: &AppHandle, state: &AppState) -> Result<PathBuf> {
    let game = require_install(&*state.inner.lock().await)?.game;
    let documents = app.path().document_dir().map_err(|e| Error::msg(format!("Could not find your Documents folder: {e}")))?;
    Ok(options::path_for(&documents, game))
}

/// The game's own graphics settings (`Options.ini` in Documents).
#[tauri::command]
async fn options_load(app: AppHandle, state: State<'_, AppState>) -> Result<OptionsResponse> {
    let file = options_file(&app, &state).await?;
    let path = file.to_string_lossy().into_owned();
    let (exists, graphics) = blocking(move || options::load(&file)).await?;
    Ok(OptionsResponse { path, exists, graphics })
}

#[tauri::command]
async fn options_save(app: AppHandle, state: State<'_, AppState>, graphics: options::Graphics) -> Result<()> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game first: it rewrites its options when it exits"));
    }
    let file = options_file(&app, &state).await?;
    blocking(move || options::save(&file, &graphics)).await
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<()> {
    let install = require_install(&*state.inner.lock().await)?;
    store::save(&settings_path(&install), &settings)
}

fn exe_infos_for(settings: &Settings, extras: &HashMap<String, ExtraInfos>) -> HashMap<String, Entry> {
    let mod_id = settings.active_mod.as_deref().filter(|m| !m.is_empty()).unwrap_or(catalog::ORIGINAL_GAME_ID);
    extras
        .get(mod_id)
        .map(|e| e.exes.iter().map(|i| (catalog::extra_id(mod_id, catalog::Kind::Exe, &i.name), i.clone())).collect())
        .unwrap_or_default()
}

/// Prepare the game folder, start the game and return. A background task waits for the game to
/// exit, restores the folder and emits `launch-finished`.
#[tauri::command]
async fn launch_game(app: AppHandle, state: State<'_, AppState>, target: String) -> Result<()> {
    if state.launching.swap(true, Ordering::SeqCst) {
        return Err(Error::msg("A game session is already running"));
    }
    let result = begin_session(&app, &state, if target == "worldbuilder" { Target::WorldBuilder } else { Target::Game }).await;
    if result.is_err() {
        state.launching.store(false, Ordering::SeqCst);
    }
    result
}

async fn begin_session(app: &AppHandle, state: &AppState, target: Target) -> Result<()> {
    let (install, exe_infos) = {
        let inner = state.inner.lock().await;
        let install = require_install(&inner)?;
        let settings: Settings = store::load(&settings_path(&install));
        (install, exe_infos_for(&settings, &inner.extras))
    };
    let settings: Settings = store::load(&settings_path(&install));
    let (dir, game) = (install.dir.clone(), install.game);

    let resolved = {
        let (dir, settings) = (dir.clone(), settings.clone());
        blocking(move || {
            // Clear anything an earlier session left behind before planning on top of it.
            journal::recover(&dir)?;
            let disk = library::scan(&dir);
            let resolved = launch::resolve(&dir, &settings, &disk, &exe_infos, target)?;
            let ops = plan::build(&dir, game, &resolved.layers, settings.camera_height)?;
            journal::apply(&dir, ops)?;
            Ok(resolved)
        })
        .await?
    };

    let child = match launch::spawn(&dir, &resolved.exe, &launch::game_args(&settings)) {
        Ok(c) => c,
        Err(e) => {
            let d = dir.clone();
            let _ = blocking(move || journal::recover(&d)).await;
            return Err(e);
        }
    };
    for helper in &resolved.helpers {
        if let Err(e) = launch::spawn(&dir, helper, &[]) {
            log::warn!("helper program {helper} did not start: {e}");
        }
    }

    for p in settings.extra_programs.iter().filter(|p| !p.path.trim().is_empty() && (!p.only_windowed || settings.windowed)) {
        if let Err(e) = launch::spawn_program(std::path::Path::new(p.path.trim())) {
            log::warn!("extra program: {e}");
            let _ = app.emit("extra-program-failed", p.path.clone());
        }
    }

    let hide = settings.hide_launcher_while_playing;
    if hide {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    }
    let _ = app.emit("launch-state", "running");
    tauri::async_runtime::spawn(finish_session(app.clone(), dir, child, hide));
    Ok(())
}

async fn finish_session(app: AppHandle, dir: PathBuf, mut child: std::process::Child, hidden: bool) {
    let started = Instant::now();
    let waited = tokio::task::spawn_blocking(move || child.wait()).await;
    let mut error = match waited {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(format!("lost track of the game process: {e}")),
        Err(e) => Some(e.to_string()),
    };

    let restored = tokio::task::spawn_blocking(move || journal::recover(&dir)).await;
    let warnings = match restored {
        Ok(Ok(Some(r))) => r.warnings,
        Ok(Ok(None)) => Vec::new(),
        Ok(Err(e)) => {
            error = Some(format!("could not restore the game folder: {e}"));
            Vec::new()
        }
        Err(e) => {
            error = Some(e.to_string());
            Vec::new()
        }
    };

    if hidden {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
    app.state::<AppState>().launching.store(false, Ordering::SeqCst);
    let _ = app.emit("launch-finished", LaunchFinished { error, warnings, played_secs: started.elapsed().as_secs() });
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    id: String,
    phase: install::Phase,
    done: u64,
    total: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstallFinished {
    id: String,
    ok: bool,
    cancelled: bool,
    error: Option<String>,
    version: Option<String>,
}

/// Everything needed to run one install in the background.
struct InstallRun {
    id: String,
    job: install::Job,
    version: String,
    /// Written next to the installed version when it succeeds.
    record: record::SourceRecord,
    /// Older versions of the same item to remove afterwards (when that setting is on).
    cleanup: Option<(PathBuf, target::Located)>,
    /// A file the launcher downloaded itself (built-in browser); deleted once installed.
    delete_after: Option<PathBuf>,
    /// The file comes from the built-in browser and may still be arriving: wait for it, then install.
    wait_for_browser: bool,
}

/// Wait until the built-in browser's download has finished and return the file. Progress is reported as
/// "downloading"; cancelling closes the browser window, which stops the download.
async fn wait_for_browser_download(app: &AppHandle, cancel: &download::Cancel, report: &install::Report) -> Result<PathBuf> {
    loop {
        if cancel.is_cancelled() {
            if let Some(w) = app.get_webview_window(BROWSER) {
                let _ = w.destroy();
            }
            *app.state::<AppState>().browser_session.lock().unwrap() = BrowserSession::Idle;
            return Err(Error::Cancelled);
        }
        let session = app.state::<AppState>().browser_session.lock().unwrap().clone();
        match session {
            BrowserSession::Done { path } => {
                *app.state::<AppState>().browser_session.lock().unwrap() = BrowserSession::Idle; // consumed
                return Ok(path);
            }
            BrowserSession::Failed { message } => {
                *app.state::<AppState>().browser_session.lock().unwrap() = BrowserSession::Idle;
                return Err(Error::msg(message));
            }
            BrowserSession::Idle => return Err(Error::msg("The download from the browser did not start")),
            BrowserSession::Downloading { target } => {
                let done = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
                report(install::Progress { phase: install::Phase::Downloading, done, total: 0 });
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }
    }
}

/// Register the install (one per item at a time) and run it on a background task.
fn start_install(app: &AppHandle, state: &AppState, run: InstallRun) -> Result<()> {
    let cancel = download::Cancel::new();
    {
        let mut running = state.downloads.lock().unwrap();
        if running.contains_key(&run.id) {
            return Err(Error::msg("This is already being installed"));
        }
        running.insert(run.id.clone(), cancel.clone());
    }

    let client = state.download_client.clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let InstallRun { id, mut job, version, mut record, cleanup, mut delete_after, wait_for_browser } = run;
        let report: install::Report = {
            let (app, id) = (app.clone(), id.clone());
            std::sync::Arc::new(move |p: install::Progress| {
                let _ = app.emit("install-progress", InstallProgress { id: id.clone(), phase: p.phase, done: p.done, total: p.total });
            })
        };
        let result = async {
            if wait_for_browser {
                // confirmed while the file may still be arriving: wait for it, then install it like any local file
                let path = wait_for_browser_download(&app, &cancel, &report).await?;
                job.source = install::Source::Local { path: path.clone() };
                delete_after = Some(path);
            }
            install::run(&client, &job, &cancel, report.clone(), &download::Options::default()).await
        }
        .await;

        if let Ok(outcome) = &result {
            record.archive_name = outcome.archive_name.clone();
            record.sha256 = outcome.sha256.clone();
            if let Err(e) = record::write(&job.dest, &record) {
                log::warn!("could not save the source record: {e}");
            }
            if let Some(file) = &delete_after {
                let _ = std::fs::remove_file(file);
            }
            if let Some((game_dir, loc)) = cleanup {
                for old in loc.installed.iter().filter(|v| !v.eq_ignore_ascii_case(&version)) {
                    match target::version_dir(&game_dir, &loc, old) {
                        Ok(dir) => {
                            if let Err(e) = std::fs::remove_dir_all(&dir) {
                                log::warn!("could not remove old version {}: {e}", dir.display());
                            }
                            record::remove(&dir);
                        }
                        Err(e) => log::warn!("not removing old version {old}: {e}"),
                    }
                }
            }
        }

        app.state::<AppState>().downloads.lock().unwrap().remove(&id);
        let cancelled = matches!(result, Err(Error::Cancelled));
        let _ = app.emit(
            "install-finished",
            InstallFinished {
                id,
                ok: result.is_ok(),
                cancelled,
                error: result.as_ref().err().filter(|_| !cancelled).map(|e| e.to_string()),
                version: result.is_ok().then_some(version),
            },
        );
    });
    Ok(())
}

#[derive(serde::Deserialize)]
// `rename_all` renames the variants (file, link); `rename_all_fields` the fields inside them (pageUrl).
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum AddSource {
    /// A file (archive, single file) or folder on this computer.
    File { path: PathBuf, page_url: Option<String> },
    /// A direct download link. `page_url` is where it came from (e.g. the ModDB page), for the record.
    Link { url: String, page_url: Option<String> },
    /// The file the built-in browser is receiving (or has received): installed as soon as it is complete.
    Browser { page_url: Option<String> },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddRequest {
    source: AddSource,
    name: String,
    version: String,
    kind: catalog::Kind,
    /// The mod a patch / addon / executable belongs to.
    parent: Option<String>,
}

/// Add something the user picked or linked themselves (not from the catalog). Returns the item's id;
/// progress and the result arrive as `install-progress` / `install-finished` like any install.
#[tauri::command]
async fn add_item(app: AppHandle, state: State<'_, AppState>, request: AddRequest) -> Result<String> {
    let (install, catalog, extras) = {
        let inner = state.inner.lock().await;
        (require_install(&inner)?, inner.catalog.clone().unwrap_or_default(), inner.extras.clone())
    };
    let version = request.version.trim().to_string();
    if version.is_empty() || version == "." || version == ".." {
        return Err(Error::msg("Enter a version, for example 1.0"));
    }
    let version_folder = target::sanitize_component(&version);

    let (source, mut record) = match &request.source {
        AddSource::File { path, page_url } => {
            if !path.exists() {
                return Err(Error::msg("That file or folder no longer exists"));
            }
            let page_url = page_url.clone().filter(|p| !p.trim().is_empty());
            // a file taken from a web page (built-in browser or the user's own) versus one that was already on disk
            let mut r = record::SourceRecord::now(if page_url.is_some() { "page" } else { "file" });
            r.page_url = page_url;
            (install::Source::Local { path: path.clone() }, r)
        }
        AddSource::Link { url, page_url } => {
            let parsed = reqwest::Url::parse(url.trim()).map_err(|_| Error::msg("That does not look like a web address"))?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(Error::msg("Only http and https links can be downloaded"));
            }
            let mut r = record::SourceRecord::now("link");
            r.url = Some(url.trim().to_string());
            r.page_url = page_url.clone();
            (install::Source::Archive { url: url.trim().to_string() }, r)
        }
        AddSource::Browser { page_url } => {
            if matches!(*state.browser_session.lock().unwrap(), BrowserSession::Idle) {
                return Err(Error::msg("There is no download from the browser to install. Open the page and click its download button first."));
            }
            let mut r = record::SourceRecord::now("page");
            r.page_url = page_url.clone().filter(|p| !p.trim().is_empty());
            // the real file is filled in by `wait_for_browser_download` once it has arrived
            (install::Source::Local { path: PathBuf::new() }, r)
        }
    };
    if let AddSource::File { path, .. } = &request.source {
        record.archive_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).filter(|_| path.is_file());
    }

    let disk = scan_disk(install.dir.clone()).await?;
    let (id, root) = target::new_item(&install.dir, &catalog, &extras, &disk, request.kind, &request.name, request.parent.as_deref())?;
    let dest = root.join(&version_folder);
    if dest.exists() {
        return Err(Error::msg(format!("{} {} is already installed. Remove it first, or choose another version.", request.name.trim(), version)));
    }
    let key = root.strip_prefix(library::mods_root(&install.dir)).unwrap_or(&root).to_string_lossy().replace(['/', '\\'], "_");
    let staging = layout::tmp(&install.dir).join(target::sanitize_component(&format!("{key} {version_folder}")));
    let job = install::Job { dest, staging, source, previous: None };
    let delete_after = match &request.source {
        // only files inside the launcher's own staging folder are ever deleted, never the user's
        AddSource::File { path, .. } if path.starts_with(layout::tmp(&install.dir)) => Some(path.clone()),
        _ => None,
    };
    let wait_for_browser = matches!(request.source, AddSource::Browser { .. });
    start_install(&app, &state, InstallRun { id: id.clone(), job, version, record, cleanup: None, delete_after, wait_for_browser })?;
    Ok(id)
}

#[tauri::command]
async fn cancel_install(state: State<'_, AppState>, id: String) -> Result<()> {
    if let Some(c) = state.downloads.lock().unwrap().get(&id) {
        c.cancel();
    }
    Ok(())
}

/// Move a GenLauncher 1.x library into the new layout and remove the old launcher's files
/// (see [`migrate`] for exactly what is and is not touched).
#[tauri::command]
async fn migrate_legacy(state: State<'_, AppState>) -> Result<migrate::Report> {
    if state.launching.load(Ordering::SeqCst) || !state.downloads.lock().unwrap().is_empty() {
        return Err(Error::msg("Wait until the game has closed and downloads have finished"));
    }
    let install = require_install(&*state.inner.lock().await)?;
    blocking(move || {
        let report = migrate::execute(&migrate::plan(&install.dir));
        match &report.error {
            Some(e) => Err(Error::msg(format!("Cleanup stopped: {e}. Nothing was deleted after that point."))),
            None => Ok(report),
        }
    })
    .await
}

/// Show an item's files in the system file manager: the chosen version's folder (or the newest installed
/// one). For the original game this is the game folder itself.
#[tauri::command]
async fn open_item_folder(app: AppHandle, state: State<'_, AppState>, id: String, version: Option<String>) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let (install, catalog, extras) = {
        let inner = state.inner.lock().await;
        (require_install(&inner)?, inner.catalog.clone().unwrap_or_default(), inner.extras.clone())
    };
    let path = if id == catalog::ORIGINAL_GAME_ID {
        install.dir.clone()
    } else {
        let disk = scan_disk(install.dir.clone()).await?;
        let loc = target::locate(&install.dir, &catalog, &extras, &disk, &id)?;
        let chosen = version.as_deref().filter(|v| loc.installed.iter().any(|i| i.eq_ignore_ascii_case(v)));
        match chosen.or(loc.installed.first().map(String::as_str)) {
            Some(v) => target::version_dir(&install.dir, &loc, v)?,
            None => return Err(Error::msg("It is not installed yet, so there is no folder to open.")),
        }
    };
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(format!("Could not open the folder: {e}")))
}

/// Delete one installed version (never anything outside `.genlauncher/mods/`).
#[tauri::command]
async fn uninstall_version(state: State<'_, AppState>, id: String, version: String) -> Result<()> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game before removing mod files"));
    }
    if state.downloads.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("This item is being installed right now"));
    }
    let (install, catalog, extras) = {
        let inner = state.inner.lock().await;
        (require_install(&inner)?, inner.catalog.clone().ok_or_else(|| Error::msg("Catalog not loaded"))?, inner.extras.clone())
    };
    let disk = scan_disk(install.dir.clone()).await?;
    let loc = target::locate(&install.dir, &catalog, &extras, &disk, &id)?;
    let dir = target::version_dir(&install.dir, &loc, &version)?;
    let mods_root = library::mods_root(&install.dir);
    blocking(move || {
        library::remove_version(&dir, &mods_root)
    })
    .await
}

async fn maps_folder(app: &AppHandle, state: &AppState) -> Result<PathBuf> {
    let game = require_install(&*state.inner.lock().await)?.game;
    let documents = app.path().document_dir().map_err(|e| Error::msg(format!("Could not find your Documents folder: {e}")))?;
    Ok(maps::maps_dir(&documents, game))
}

/// The custom maps in the game's Maps folder.
#[tauri::command]
async fn maps_list(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<maps::MapInfo>> {
    let dir = maps_folder(&app, &state).await?;
    blocking(move || Ok(maps::list(&dir))).await
}

/// Add the maps of a pack (an archive or a folder) to the game: each one in its own folder.
#[tauri::command]
async fn maps_import(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<maps::ImportReport> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game first"));
    }
    let dest = maps_folder(&app, &state).await?;
    let install = { let inner = state.inner.lock().await; require_install(&inner)? };
    let source = PathBuf::from(path);
    blocking(move || {
        if source.is_dir() {
            return maps::import_tree(&source, &dest);
        }
        if !extract::is_archive_name(&source.to_string_lossy()) {
            return Err(Error::msg("Choose a zip, 7z or rar file, or a folder with maps in it."));
        }
        let work = layout::tmp(&install.dir).join(format!("maps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let result = extract::extract(&source, &work, &|| false).and_then(|_| maps::import_tree(&work, &dest));
        let _ = std::fs::remove_dir_all(&work);
        result
    })
    .await
}

/// The preview picture (a .tga) of one map, as raw bytes.
#[tauri::command]
async fn maps_preview(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<tauri::ipc::Response> {
    let dir = maps_folder(&app, &state).await?;
    let bytes = blocking(move || maps::preview(&dir, &name)).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Delete one custom map.
#[tauri::command]
async fn maps_remove(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<()> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game first"));
    }
    let dir = maps_folder(&app, &state).await?;
    blocking(move || maps::remove(&dir, &name)).await
}

/// Open the Maps folder in the file manager.
#[tauri::command]
async fn maps_open_folder(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = maps_folder(&app, &state).await?;
    std::fs::create_dir_all(&dir)?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(format!("Could not open the folder: {e}")))
}

async fn gamedata_dirs(app: &AppHandle, state: &AppState) -> Result<(PathBuf, PathBuf)> {
    let game = require_install(&*state.inner.lock().await)?.game;
    let documents = app.path().document_dir().map_err(|e| Error::msg(format!("Could not find your Documents folder: {e}")))?;
    let config = app.path().app_config_dir().map_err(|e| Error::msg(e.to_string()))?;
    Ok((gamedata::data_dir(&documents, game), gamedata::backups_dir(&config, game)))
}

/// The backups of the game's saved games, replays and settings.
#[tauri::command]
async fn gamedata_list(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<gamedata::Backup>> {
    let (_, backups) = gamedata_dirs(&app, &state).await?;
    blocking(move || Ok(gamedata::list(&backups))).await
}

#[tauri::command]
async fn gamedata_backup(app: AppHandle, state: State<'_, AppState>) -> Result<gamedata::Backup> {
    let (data, backups) = gamedata_dirs(&app, &state).await?;
    blocking(move || gamedata::create(&data, &backups)).await
}

#[tauri::command]
async fn gamedata_restore(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<()> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game first: it writes its saved games and settings when it exits"));
    }
    let (data, backups) = gamedata_dirs(&app, &state).await?;
    blocking(move || gamedata::restore(&data, &backups, &name).map(|_| ())).await
}

#[tauri::command]
async fn gamedata_delete(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<()> {
    let (_, backups) = gamedata_dirs(&app, &state).await?;
    blocking(move || gamedata::delete(&backups, &name)).await
}

#[tauri::command]
async fn gamedata_open_folder(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let (data, _) = gamedata_dirs(&app, &state).await?;
    std::fs::create_dir_all(&data)?;
    app.opener().open_path(data.to_string_lossy(), None::<&str>).map_err(|e| Error::msg(format!("Could not open the folder: {e}")))
}

/// Look for the usual causes of a game that will not start or mods that look wrong.
#[tauri::command]
async fn health_check(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<health::Finding>> {
    let install = { let inner = state.inner.lock().await; require_install(&inner)? };
    let options = options_file(&app, &state).await.ok();
    // while a game is running its mods are linked into the folder: that is not a problem to report
    let unfinished = journal::journal_path(&install.dir).exists() || state.launching.load(Ordering::SeqCst);
    blocking(move || Ok(health::check(&install, options.as_deref(), unfinished))).await
}

/// Apply one of the fixes the check offered.
#[tauri::command]
async fn health_fix(state: State<'_, AppState>, id: String) -> Result<String> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game first"));
    }
    let install = { let inner = state.inner.lock().await; require_install(&inner)? };
    blocking(move || health::fix(&install, &id)).await
}

/// Rename one installed version (for example "1.87" to "1.87 my settings"). Returns the new name.
#[tauri::command]
async fn rename_version(state: State<'_, AppState>, id: String, version: String, new_version: String) -> Result<String> {
    if state.launching.load(Ordering::SeqCst) {
        return Err(Error::msg("Close the game before renaming mod files"));
    }
    if state.downloads.lock().unwrap().contains_key(&id) {
        return Err(Error::msg("This item is being installed right now"));
    }
    let (install, catalog, extras) = {
        let inner = state.inner.lock().await;
        (require_install(&inner)?, inner.catalog.clone().ok_or_else(|| Error::msg("Catalog not loaded"))?, inner.extras.clone())
    };
    let disk = scan_disk(install.dir.clone()).await?;
    let loc = target::locate(&install.dir, &catalog, &extras, &disk, &id)?;
    let dir = target::version_dir(&install.dir, &loc, &version)?;
    blocking(move || library::rename_version(&dir, &new_version)).await
}

const BROWSER: &str = "browser";

/// What happens in the built-in browser window, as seen by the main window.
#[derive(Serialize, Clone)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum BrowserEvent {
    /// The user clicked a download; the file is being saved to the launcher's own folder.
    Started { file_name: String },
    Progress { file_name: String, bytes: u64 },
    Finished { file_name: String, path: PathBuf },
    Failed { message: String },
    /// No data arrived for a long time: the attempt was cancelled and the window closed.
    Stalled { message: String },
    /// The window went away (finished, cancelled or closed by the user).
    Closed,
}

type ActiveDownload = std::sync::Arc<std::sync::Mutex<Option<(PathBuf, std::sync::Arc<AtomicBool>)>>>;

/// Open `url` in a launcher-owned browser window. The user picks the file and clicks its download
/// button; the launcher takes that download (nothing is fetched or scraped automatically) and
/// reports it with `browser-event`. The page cannot call any launcher command: only the main window
/// has permissions, and pop-ups and non-web addresses are blocked.
#[tauri::command]
async fn open_browser(app: AppHandle, state: State<'_, AppState>, url: String) -> Result<()> {
    use tauri::webview::{DownloadEvent, NewWindowResponse};
    let parsed = tauri::Url::parse(url.trim()).map_err(|_| Error::msg("That does not look like a web address"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(Error::msg("Only http and https pages can be opened"));
    }
    let install = require_install(&*state.inner.lock().await)?;
    {
        let mut session = state.browser_session.lock().unwrap();
        if matches!(*session, BrowserSession::Downloading { .. }) {
            return Err(Error::msg("A download from the browser is still in progress. Wait for it, or cancel it in Downloads, before opening another page."));
        }
        *session = BrowserSession::Idle;
    }
    let dir = browser::dir(&install.dir);
    browser::clear(&dir);
    std::fs::create_dir_all(&dir)?;
    if let Some(old) = app.get_webview_window(BROWSER) {
        let _ = old.destroy();
    }

    let active: ActiveDownload = Default::default();
    let stall_after = std::env::var("GENLAUNCHER_STALL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(60));
    let hook_app = app.clone();
    let hook_active = active.clone();
    let window = tauri::WebviewWindowBuilder::new(&app, BROWSER, tauri::WebviewUrl::External(parsed))
        .title("GenLauncher: find the version you want and click its download button")
        .inner_size(1100.0, 800.0)
        .on_navigation(browser::allowed_navigation)
        .on_new_window(|_url, _features| NewWindowResponse::Deny)
        .on_download(move |_webview, event| {
            match event {
                DownloadEvent::Requested { destination, .. } => {
                    let mut slot = hook_active.lock().unwrap();
                    if slot.is_some() {
                        return false; // one download at a time
                    }
                    let target = browser::destination(&dir, destination);
                    *destination = target.clone();
                    let file_name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let stop = std::sync::Arc::new(AtomicBool::new(false));
                    *slot = Some((target.clone(), stop.clone()));
                    *hook_app.state::<AppState>().browser_session.lock().unwrap() = BrowserSession::Downloading { target: target.clone() };
                    let _ = hook_app.emit("browser-event", BrowserEvent::Started { file_name: file_name.clone() });
                    // Get the page out of the way now that the launcher shows the progress. It is hidden, not
                    // closed: destroying the window would cancel the download, and the page's session is what
                    // makes the download link work.
                    let hide_app = hook_app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(w) = hide_app.get_webview_window(BROWSER) {
                            let _ = w.hide();
                        }
                    });
                    let app = hook_app.clone();
                    let poll_active = hook_active.clone();
                    tauri::async_runtime::spawn(async move {
                        let mut last_growth = (0u64, std::time::Instant::now());
                        while !stop.load(Ordering::SeqCst) {
                            let size = std::fs::metadata(&target).map(|m| m.len()).ok();
                            if let Some(bytes) = size {
                                let _ = app.emit("browser-event", BrowserEvent::Progress { file_name: file_name.clone(), bytes });
                                if bytes != last_growth.0 {
                                    last_growth = (bytes, std::time::Instant::now());
                                }
                            }
                            // The page is hidden while this runs, so a download that silently stops (connection
                            // cut, server gone) must not leave the user waiting for ever.
                            if last_growth.1.elapsed() > stall_after {
                                if poll_active.lock().unwrap().take().is_some() {
                                    stop.store(true, Ordering::SeqCst);
                                    let _ = std::fs::remove_file(&target);
                                    let message = format!("The download stopped receiving data for {} seconds, so it was cancelled. Open the page again and retry.", stall_after.as_secs());
                                    *app.state::<AppState>().browser_session.lock().unwrap() = BrowserSession::Failed { message: message.clone() };
                                    let _ = app.emit("browser-event", BrowserEvent::Stalled { message });
                                    if let Some(w) = app.get_webview_window(BROWSER) {
                                        let _ = w.destroy();
                                    }
                                }
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    });
                }
                DownloadEvent::Finished { path, success, .. } => {
                    let finished = hook_active.lock().unwrap().take();
                    if let Some((target, stop)) = finished {
                        stop.store(true, Ordering::SeqCst);
                        let path = path.unwrap_or(target);
                        if success {
                            let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            *hook_app.state::<AppState>().browser_session.lock().unwrap() = BrowserSession::Done { path: path.clone() };
                            let _ = hook_app.emit("browser-event", BrowserEvent::Finished { file_name, path });
                        } else {
                            let _ = std::fs::remove_file(&path);
                            let message = "The download did not finish. Open the page again and retry.".to_string();
                            *hook_app.state::<AppState>().browser_session.lock().unwrap() = BrowserSession::Failed { message: message.clone() };
                            let _ = hook_app.emit("browser-event", BrowserEvent::Failed { message });
                        }
                        // either way the page has done its job (hidden since the download began): close it
                        let app = hook_app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Some(w) = app.get_webview_window(BROWSER) {
                                let _ = w.destroy();
                            }
                        });
                    }
                }
                _ => {}
            }
            true
        })
        .build()
        .map_err(|e| Error::msg(format!("Could not open the browser window: {e}")))?;

    let closed_app = app.clone();
    window.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            if let Some((partial, stop)) = active.lock().unwrap().take() {
                stop.store(true, Ordering::SeqCst); // cancelled in the middle of a download
                let _ = std::fs::remove_file(partial);
                *closed_app.state::<AppState>().browser_session.lock().unwrap() =
                    BrowserSession::Failed { message: "The download was cancelled.".into() };
            }
            let _ = closed_app.emit("browser-event", BrowserEvent::Closed);
        }
    });
    Ok(())
}

#[tauri::command]
async fn close_browser(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    if let Some(w) = app.get_webview_window(BROWSER) {
        let _ = w.destroy(); // also stops a download in progress
    }
    // a file that arrived but was never confirmed must not be left behind (they can be large)
    let mut session = state.browser_session.lock().unwrap();
    if let BrowserSession::Done { path } = &*session {
        let _ = std::fs::remove_file(path);
    }
    if !matches!(*session, BrowserSession::Downloading { .. }) {
        *session = BrowserSession::Idle;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // a second start (a shortcut clicked while the tray icon is there) brings the running one forward
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            background::show_main(app);
            presets::play_in_running(app, &args);
        }))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![background::MINIMIZED])))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // GENLAUNCHER_CONFIG_DIR lets tests and side-by-side builds keep their own settings
            let config_dir = match std::env::var_os("GENLAUNCHER_CONFIG_DIR") {
                Some(d) => PathBuf::from(d),
                None => app.path().app_config_dir()?,
            };
            let config_path = config_dir.join("app.json");
            let config: AppConfig = store::load(&config_path);
            app.manage(AppState {
                inner: Mutex::new(Inner { config_path, config, install: None, catalog: None, extras: HashMap::new() }),
                google_access: Mutex::new(None),
                http: reqwest::Client::builder()
                    .user_agent(concat!("GenLauncher/", env!("CARGO_PKG_VERSION")))
                    .connect_timeout(std::time::Duration::from_secs(10))
                    .timeout(std::time::Duration::from_secs(30))
                    .build()?,
                launching: AtomicBool::new(false),
                close_to_tray: AtomicBool::new(true),
                downloads: Default::default(),
                browser_session: Default::default(),
                pending_play: std::sync::Mutex::new(presets::play_arg(&std::env::args().collect::<Vec<_>>())),
                download_client: reqwest::Client::builder()
                    .user_agent(concat!("GenLauncher/", env!("CARGO_PKG_VERSION")))
                    .connect_timeout(std::time::Duration::from_secs(15))
                    .build()?,
            });
            background::setup_tray(app.handle())?;
            // the window starts hidden (tauri.conf.json); a start with Windows keeps it that way
            if !std::env::args().any(|a| a == background::MINIMIZED) {
                background::show_main(app.handle());
            }
            Ok(())
        })
        // Closing the launcher while the game runs would leave the game folder modified.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                if window.state::<AppState>().close_to_tray.load(Ordering::SeqCst) {
                    // keep running in the tray (a running game's folder is still restored when it closes)
                    api.prevent_close();
                    let _ = window.hide();
                    if let Some(browser) = window.app_handle().get_webview_window(BROWSER) {
                        let _ = browser.hide();
                    }
                } else if window.state::<AppState>().launching.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.emit("close-blocked", ());
                } else if let Some(browser) = window.app_handle().get_webview_window(BROWSER) {
                    let _ = browser.destroy(); // the app exits when its last window closes
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap, select_install, catalog_load, catalog_reload, load_extras, announcements_load, options_load, options_save, config_export, config_import, backup_create, google_status, google_sign_in, google_sign_out, cloud_backup_create, cloud_backup_list, cloud_backup_delete, cloud_backup_restore, backup_list, backup_delete, backup_restore, save_settings, launch_game, add_item, cancel_install, migrate_legacy,
            uninstall_version, rename_version, health_check, health_fix, maps_list, maps_import, maps_remove, maps_preview, maps_open_folder, gamedata_list, gamedata_backup, gamedata_restore, gamedata_delete, gamedata_open_folder, open_browser, close_browser, open_item_folder, update_check, update_install,
            background::background_set, background::autostart_status, background::autostart_set, background::os_notify,
            presets::take_pending_play, presets::launcher_command, presets::create_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact JSON `src/lib/backend.ts` sends; a renamed or misspelled field would otherwise be
    /// dropped silently (serde ignores unknown fields) and only show up as missing data.
    #[test]
    fn add_request_json_matches_what_the_ui_sends() {
        let file: AddRequest = serde_json::from_str(
            r#"{"source":{"type":"file","path":"C:/tmp/Mod.zip","pageUrl":"https://www.moddb.com/mods/x"},"name":"X","version":"1.0","kind":"patch","parent":"contra"}"#,
        )
        .unwrap();
        match file.source {
            AddSource::File { path, page_url } => {
                assert_eq!(path, PathBuf::from("C:/tmp/Mod.zip"));
                assert_eq!(page_url.as_deref(), Some("https://www.moddb.com/mods/x"));
            }
            _ => panic!("expected a file source"),
        }
        assert_eq!(file.kind, catalog::Kind::Patch);
        assert_eq!(file.parent.as_deref(), Some("contra"));

        let link: AddRequest =
            serde_json::from_str(r#"{"source":{"type":"link","url":"https://e.org/a.zip","pageUrl":null},"name":"A","version":"2","kind":"exe","parent":null}"#).unwrap();
        assert!(matches!(link.source, AddSource::Link { ref url, page_url: None } if url == "https://e.org/a.zip"));
        assert_eq!(link.kind, catalog::Kind::Exe);

        let link_with_page: AddRequest =
            serde_json::from_str(r#"{"source":{"type":"link","url":"https://e.org/a.zip","pageUrl":"https://e.org/page"},"name":"A","version":"2","kind":"mod"}"#).unwrap();
        assert!(matches!(link_with_page.source, AddSource::Link { page_url: Some(ref p), .. } if p == "https://e.org/page"));

        // "install whatever the built-in browser is downloading"
        let browser: AddRequest =
            serde_json::from_str(r#"{"source":{"type":"browser","pageUrl":"https://www.moddb.com/mods/x"},"name":"X","version":"1.0","kind":"mod","parent":null}"#).unwrap();
        assert!(matches!(browser.source, AddSource::Browser { page_url: Some(ref p) } if p.contains("moddb")));
    }

    #[test]
    fn every_version_links_back_to_its_own_page() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path();
        // two versions of one mod, downloaded from different sites
        for (v, page) in [("2.0", Some("https://www.moddb.com/mods/my-mod/downloads/two")), ("1.0", Some("https://www.nexusmods.com/games/generals/mods/1"))] {
            let version = library::mods_root(g).join("My Mod").join(v);
            std::fs::create_dir_all(&version).unwrap();
            std::fs::write(version.join("a.gib"), b"x").unwrap();
            let mut rec = record::SourceRecord::now("page");
            rec.page_url = page.map(Into::into);
            record::write(&version, &rec).unwrap();
        }
        // a mod whose record names no page
        let other = library::mods_root(g).join("Plain").join("1");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("b.gib"), b"x").unwrap();

        let disk = library::scan(g);
        let mut items = catalog::mod_items(&Default::default(), &disk);
        attach_pages(g, &mut items, &disk);
        let page = |id: &str, v: &str| items.iter().find(|i| i.id == id).unwrap().versions.iter().find(|x| x.v == v).and_then(|x| x.page.clone());
        assert_eq!(page("my-mod", "2.0").as_deref(), Some("https://www.moddb.com/mods/my-mod/downloads/two"));
        assert_eq!(page("my-mod", "1.0").as_deref(), Some("https://www.nexusmods.com/games/generals/mods/1"));
        assert_eq!(page("plain", "1"), None);
        assert_eq!(page(catalog::ORIGINAL_GAME_ID, "Stock"), None);
    }

    #[test]
    fn browser_events_use_the_names_the_ui_listens_for() {
        let v = |e: &BrowserEvent| serde_json::to_value(e).unwrap();
        assert_eq!(v(&BrowserEvent::Started { file_name: "a.zip".into() }), serde_json::json!({"state":"started","fileName":"a.zip"}));
        assert_eq!(v(&BrowserEvent::Progress { file_name: "a.zip".into(), bytes: 5 }), serde_json::json!({"state":"progress","fileName":"a.zip","bytes":5}));
        assert_eq!(v(&BrowserEvent::Finished { file_name: "a.zip".into(), path: PathBuf::from("p") }), serde_json::json!({"state":"finished","fileName":"a.zip","path":"p"}));
        assert_eq!(v(&BrowserEvent::Failed { message: "m".into() }), serde_json::json!({"state":"failed","message":"m"}));
        assert_eq!(v(&BrowserEvent::Stalled { message: "m".into() }), serde_json::json!({"state":"stalled","message":"m"}));
        assert_eq!(v(&BrowserEvent::Closed), serde_json::json!({"state":"closed"}));
    }
}
