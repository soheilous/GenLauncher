pub mod big;
pub mod catalog;
pub mod download;
pub mod error;
pub mod extract;
pub mod game;
pub mod game_files;
pub mod journal;
pub mod layout;
pub mod launch;
pub mod install;
pub mod library;
pub mod links;
pub mod manifest;
pub mod migrate;
pub mod plan;
pub mod s3;
pub mod sigv4;
pub mod store;
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
use manifest::{Fetcher, VersionInfo};
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
    fetcher: Fetcher,
    /// True from the start of a launch until the game folder is restored again.
    launching: AtomicBool,
    /// Installs in progress, by item id.
    downloads: std::sync::Mutex<HashMap<String, download::Cancel>>,
    /// No overall timeout (mods are gigabytes); stalls are detected per chunk instead.
    download_client: reqwest::Client,
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
    launcher_version: Option<String>,
    /// Why the live catalog could not be used (the cached copy is shown instead).
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

    let (catalog, online, error) = match catalog::fetch(&state.fetcher, install.game.manifest_url()).await {
        Ok(c) => {
            if let Err(e) = store::save(&cache_path, &c) {
                log::warn!("could not cache catalog: {e}");
            }
            (c, true, None)
        }
        Err(e) => (store::load::<Catalog>(&cache_path), false, Some(e.to_string())),
    };

    let disk = scan_disk(install.dir.clone()).await?;
    let items = catalog::mod_items(&catalog, &disk);
    let launcher_version = catalog.manifest.launcher_version.clone();
    state.inner.lock().await.catalog = Some(catalog);
    Ok(CatalogResponse { items, online, launcher_version, error })
}

/// Patches, addons and executables of one mod (fetched on demand, like 1.x did).
#[tauri::command]
async fn load_extras(state: State<'_, AppState>, mod_id: String) -> Result<Vec<Item>> {
    let (install, catalog) = {
        let inner = state.inner.lock().await;
        (require_install(&inner)?, inner.catalog.clone().ok_or_else(|| Error::msg("Catalog not loaded"))?)
    };
    let disk = scan_disk(install.dir.clone()).await?;
    let entry = catalog.manifest.mod_datas.iter().find(|m| catalog::slug(&m.name) == mod_id);

    if entry.is_none() && mod_id != catalog::ORIGINAL_GAME_ID {
        // Local-only mod: nothing remote, only what is on disk.
        let d = disk.iter().find(|d| catalog::slug(&d.name) == mod_id);
        return Ok(catalog::extra_items(&mod_id, &Default::default(), d, &disk));
    }

    let infos = catalog::fetch_extras(&state.fetcher, &catalog.manifest, entry).await;
    let wanted = entry.map(|e| e.name.as_str()).unwrap_or(catalog::ORIGINAL_GAME);
    let d = disk.iter().find(|d| d.name.eq_ignore_ascii_case(wanted));
    let items = catalog::extra_items(&mod_id, &infos, d, &disk);
    state.inner.lock().await.extras.insert(mod_id, infos);
    Ok(items)
}

#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<()> {
    let install = require_install(&*state.inner.lock().await)?;
    store::save(&settings_path(&install), &settings)
}

fn exe_infos_for(settings: &Settings, extras: &HashMap<String, ExtraInfos>) -> HashMap<String, VersionInfo> {
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

/// Install (or update to) the latest catalog version of an item. Returns at once; progress and the
/// result arrive as `install-progress` / `install-finished` events.
#[tauri::command]
async fn install_item(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    let (install, catalog, extras) = {
        let inner = state.inner.lock().await;
        (require_install(&inner)?, inner.catalog.clone().ok_or_else(|| Error::msg("Catalog not loaded"))?, inner.extras.clone())
    };
    let disk = scan_disk(install.dir.clone()).await?;
    let loc = target::locate(&install.dir, &catalog, &extras, &disk, &id)?;
    let (job, version) = target::install_job(&install.dir, &loc)?;
    let settings: Settings = store::load(&settings_path(&install));

    let cancel = download::Cancel::new();
    {
        let mut running = state.downloads.lock().unwrap();
        if running.contains_key(&id) {
            return Err(Error::msg("This is already being installed"));
        }
        running.insert(id.clone(), cancel.clone());
    }

    let client = state.download_client.clone();
    let game_dir = install.dir.clone();
    tauri::async_runtime::spawn(async move {
        let report: install::Report = {
            let (app, id) = (app.clone(), id.clone());
            std::sync::Arc::new(move |p: install::Progress| {
                let _ = app.emit("install-progress", InstallProgress { id: id.clone(), phase: p.phase, done: p.done, total: p.total });
            })
        };
        let result = install::run(&client, &job, &cancel, report, &download::Options::default()).await;

        if result.is_ok() && settings.auto_delete_old_versions {
            for old in loc.installed.iter().filter(|v| !v.eq_ignore_ascii_case(&version)) {
                match target::version_dir(&game_dir, &loc, old) {
                    Ok(dir) => {
                        if let Err(e) = std::fs::remove_dir_all(&dir) {
                            log::warn!("could not remove old version {}: {e}", dir.display());
                        }
                    }
                    Err(e) => log::warn!("not removing old version {old}: {e}"),
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
    blocking(move || Ok(std::fs::remove_dir_all(dir)?)).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("app.json");
            let config: AppConfig = store::load(&config_path);
            app.manage(AppState {
                inner: Mutex::new(Inner { config_path, config, install: None, catalog: None, extras: HashMap::new() }),
                fetcher: Fetcher::new()?,
                launching: AtomicBool::new(false),
                downloads: Default::default(),
                download_client: reqwest::Client::builder()
                    .user_agent(concat!("GenLauncher/", env!("CARGO_PKG_VERSION")))
                    .connect_timeout(std::time::Duration::from_secs(15))
                    .build()?,
            });
            Ok(())
        })
        // Closing the launcher while the game runs would leave the game folder modified.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<AppState>().launching.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.emit("close-blocked", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap, select_install, catalog_load, load_extras, save_settings, launch_game, install_item, cancel_install, migrate_legacy,
            uninstall_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
