//! Builds the library the UI shows by merging the remote catalog with what is on disk.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::library::{Extra, InstalledMod};
use crate::index::{Entry, Index, ModEntry, VersionInfo};
use crate::version;

pub const ORIGINAL_GAME: &str = "Original Game";
pub const ORIGINAL_GAME_ID: &str = "original-game";

/// The mod index (see `index.rs`) and when this copy was fetched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Catalog {
    pub index: Index,
    pub fetched_at: u64,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Download the index from the project's repository.
pub async fn fetch(http: &reqwest::Client, url: &str) -> Result<Catalog> {
    let text = http.get(url).send().await?.error_for_status()?.text().await?;
    Ok(Catalog { index: crate::index::parse(&text)?, fetched_at: now() })
}

/// The copy of the index that ships inside the app: used when the repository cannot be reached and nothing was cached.
pub fn bundled(game: crate::game::Game) -> Catalog {
    Catalog { index: crate::index::parse(game.bundled_index()).unwrap_or_default(), fetched_at: 0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mod,
    Patch,
    Addon,
    Exe,
}

impl Kind {
    fn slug(self) -> &'static str {
        match self {
            Kind::Mod => "mod",
            Kind::Patch => "patch",
            Kind::Addon => "addon",
            Kind::Exe => "exe",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionDto {
    pub v: String,
    pub installed: bool,
    /// The web page this version was downloaded from (saved in its source record), when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// A picture of this version, when the catalog has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// The author's direct download of this very version, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct: Option<Direct>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Links {
    pub moddb: Option<String>,
    pub discord: Option<String>,
    pub news: Option<String>,
    pub network: Option<String>,
    pub support: Option<String>,
}

/// A direct download from the catalog (the file's address is the author's, `via` names where they published it).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Direct {
    pub url: String,
    pub via: Option<String>,
    pub size: Option<u64>,
}

fn direct_of(info: Option<&Entry>) -> Option<Direct> {
    let d = info?.download.as_ref().filter(|d| d.url.starts_with("https://"))?;
    Some(Direct { url: d.url.clone(), via: d.via.clone(), size: d.size })
}

/// One row/card in the UI. Mirrors `Item` in `src/lib/types.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub summary: String,
    pub hue: u16,
    pub versions: Vec<VersionDto>,
    pub latest: String,
    pub parent: Option<String>,
    pub depends_on: Option<String>,
    pub links: Links,
    pub cover: Option<String>,
    pub deprecated: bool,
    /// Set when the catalog has a direct download for this item (one-click install).
    pub direct: Option<Direct>,
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

pub fn extra_id(mod_id: &str, kind: Kind, name: &str) -> String {
    format!("{mod_id}/{}/{}", kind.slug(), slug(name))
}

/// Hue (0-359) of `#RRGGBB` / `#AARRGGBB`; named colors are not supported.
pub fn hue_from_color(c: &str) -> Option<u16> {
    let hex = c.trim().strip_prefix('#')?;
    let rgb = match hex.len() {
        6 => hex,
        8 => &hex[2..],
        _ => return None,
    };
    let v = u32::from_str_radix(rgb, 16).ok()?;
    let (r, g, b) = (((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    if d < 0.05 {
        return None; // grey has no hue
    }
    let h = if max == r { ((g - b) / d).rem_euclid(6.0) } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    Some(((h * 60.0).round() as u16) % 360)
}

fn name_hue(name: &str) -> u16 {
    let h = name.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    (h % 360) as u16
}

fn hue_for(info: Option<&Entry>, name: &str) -> u16 {
    info.and_then(|i| i.colors.as_ref())
        .and_then(|c| c.active.as_deref().and_then(hue_from_color).or_else(|| c.border.as_deref().and_then(hue_from_color)))
        .unwrap_or_else(|| name_hue(name))
}

/// `others`: the versions the catalog lists besides the newest (they are offered even when not installed).
fn versions_dto(installed: &[String], latest: Option<&str>, others: &[VersionInfo]) -> (Vec<VersionDto>, String) {
    let mut all: Vec<String> = installed.to_vec();
    for v in latest.into_iter().chain(others.iter().map(|o| o.version.as_str())).filter(|v| !v.trim().is_empty()) {
        if !all.iter().any(|a| a.eq_ignore_ascii_case(v)) {
            all.push(v.to_string());
        }
    }
    version::sort_desc(&mut all);
    let dto = all
        .iter()
        .map(|v| {
            let listed = others.iter().find(|o| o.version.eq_ignore_ascii_case(v));
            let is_installed = installed.iter().any(|i| i.eq_ignore_ascii_case(v));
            VersionDto {
                v: v.clone(),
                installed: is_installed,
                // for an installed version the page is filled in later from its source record; a version not installed yet offers the catalog's page for it
                page: if is_installed { None } else { listed.and_then(|o| o.page.clone()) },
                image: listed.and_then(|o| o.image.clone()),
                direct: listed.and_then(|o| o.download.as_ref()).filter(|d| d.url.starts_with("https://")).map(|d| Direct { url: d.url.clone(), via: d.via.clone(), size: d.size }),
                date: listed.and_then(|o| o.date.clone()),
            }
        })
        .collect::<Vec<_>>();
    let newest = latest.map(str::to_string).or_else(|| all.first().cloned()).unwrap_or_default();
    (dto, newest)
}

fn links_of(info: Option<&Entry>) -> Links {
    info.map_or_else(Links::default, |i| Links {
        moddb: i.moddb_link.clone(),
        discord: i.discord_link.clone(),
        news: i.news_link.clone(),
        network: i.network_info.clone(),
        support: i.support_link.clone(),
    })
}

fn find_installed<'a>(disk: &'a [InstalledMod], names: &[&str]) -> Option<&'a InstalledMod> {
    disk.iter().find(|d| names.iter().any(|n| d.name.eq_ignore_ascii_case(n)))
}

/// All mods: catalog entries merged with disk, then disk-only (manually added) mods, then the original game.
pub fn mod_items(catalog: &Catalog, disk: &[InstalledMod]) -> Vec<Item> {
    let mut items = Vec::new();
    let mut matched: Vec<&str> = Vec::new();

    for m in &catalog.index.mods {
        let info = Some(&m.entry);
        let display = m.entry.name.as_str();
        let on_disk = find_installed(disk, &[display]);
        if let Some(d) = on_disk {
            matched.push(&d.name);
        }
        let installed = on_disk.map(|d| d.versions.clone()).unwrap_or_default();
        let latest = m.entry.version.as_deref();
        let (versions, latest) = versions_dto(&installed, latest, &m.entry.versions);
        items.push(Item {
            id: slug(display),
            kind: Kind::Mod,
            name: display.to_string(),
            summary: String::new(), // the index has no description field; `network` is a link, exposed under links.network
            hue: hue_for(info, display),
            versions,
            latest,
            parent: None,
            depends_on: None,
            links: links_of(info),
            cover: m.entry.image_link.clone(),
            deprecated: m.entry.deprecated,
            direct: direct_of(Some(&m.entry)),
        });
    }

    for d in disk.iter().filter(|d| !matched.contains(&d.name.as_str()) && !d.versions.is_empty()) {
        let (versions, latest) = versions_dto(&d.versions, None, &[]);
        items.push(Item {
            id: slug(&d.name),
            kind: Kind::Mod,
            name: d.name.clone(),
            summary: String::new(),
            hue: name_hue(&d.name),
            versions,
            latest,
            parent: None,
            depends_on: None,
            links: Links::default(),
            cover: None,
            deprecated: false,
            direct: None,
        });
    }

    items.push(Item {
        id: ORIGINAL_GAME_ID.into(),
        kind: Kind::Mod,
        name: ORIGINAL_GAME.into(),
        summary: String::new(),
        hue: 220,
        versions: vec![VersionDto { v: "Stock".into(), installed: true, page: None, image: None, direct: None, date: None }],
        latest: "Stock".into(),
        parent: None,
        depends_on: None,
        links: Links::default(),
        cover: None,
        deprecated: false,
        direct: None,
    });
    items
}

fn extras_of(d: &InstalledMod, kind: Kind) -> &[Extra] {
    match kind {
        Kind::Patch => &d.patches,
        Kind::Addon => &d.addons,
        _ => &d.exes,
    }
}

/// `disks`: every on-disk list the item may live in (its mod, and the folder named by `DependenceName`).
fn extra_item(mod_id: &str, kind: Kind, info: &Entry, disks: &[&[Extra]]) -> Item {
    let mut installed: Vec<String> = Vec::new();
    for list in disks {
        for x in list.iter().filter(|x| x.name.eq_ignore_ascii_case(&info.name)) {
            for v in &x.versions {
                if !installed.iter().any(|i| i.eq_ignore_ascii_case(v)) {
                    installed.push(v.clone());
                }
            }
        }
    }
    let (versions, latest) = versions_dto(&installed, info.version.as_deref(), &info.versions);
    Item {
        id: extra_id(mod_id, kind, &info.name),
        kind,
        name: info.name.clone(),
        summary: String::new(),
        hue: hue_for(Some(info), &info.name),
        versions,
        latest,
        parent: Some(mod_id.to_string()),
        depends_on: info.dependence_name.clone().filter(|d| !d.is_empty()),
        links: links_of(Some(info)),
        cover: None,
        deprecated: info.deprecated,
        direct: direct_of(Some(info)),
    }
}

/// The index's records for one mod's patches/addons/executables.
#[derive(Debug, Clone, Default)]
pub struct ExtraInfos {
    pub patches: Vec<Entry>,
    pub addons: Vec<Entry>,
    pub exes: Vec<Entry>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// The patches, addons and executables the index lists for `entry` (or for the original game when `None`).
pub fn extras_for(index: &Index, entry: Option<&ModEntry>) -> ExtraInfos {
    let (patches, addons) = match entry {
        Some(e) => (e.patches.clone(), e.addons.clone()),
        None => (index.original_game.patches.clone(), index.original_game.addons.clone()),
    };
    let exes = index
        .executables
        .iter()
        .filter(|x| match (&x.for_mod, entry) {
            (Some(dep), Some(e)) if !dep.is_empty() && !same_name(dep, ORIGINAL_GAME) => same_name(dep, &e.entry.name),
            // no mod named (or "Original Game"): usable with any mod, like in 1.x
            _ => true,
        })
        .map(|x| x.entry.clone())
        .collect();
    ExtraInfos { patches, addons, exes }
}

/// Items for a mod's tabs. `disk` is the on-disk record of that mod (if any).
pub fn extra_items(mod_id: &str, infos: &ExtraInfos, disk: Option<&InstalledMod>, all_disk: &[InstalledMod]) -> Vec<Item> {
    let mut out = Vec::new();
    for (kind, list) in [(Kind::Patch, &infos.patches), (Kind::Addon, &infos.addons), (Kind::Exe, &infos.exes)] {
        let own: &[Extra] = disk.map(|d| extras_of(d, kind)).unwrap_or_default();
        for info in list.iter().filter(|i| !i.name.is_empty()) {
            // 1.x stored an extra under the folder named by DependenceName (a mod, or the patch an addon needs).
            let owner: &[Extra] = info
                .dependence_name
                .as_deref()
                .and_then(|dep| all_disk.iter().find(|d| same_name(&d.name, dep)))
                .map(|d| extras_of(d, kind))
                .unwrap_or_default();
            out.push(extra_item(mod_id, kind, info, &[own, owner]));
        }
        // Manually added extras that are not in the catalog.
        for x in own {
            if !list.iter().any(|i| same_name(&i.name, &x.name)) {
                let synthetic = Entry { name: x.name.clone(), ..Default::default() };
                out.push(extra_item(mod_id, kind, &synthetic, &[own]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Colors;

    fn entry(name: &str) -> ModEntry {
        ModEntry { entry: info(name, "1"), ..Default::default() }
    }

    fn info(name: &str, version: &str) -> Entry {
        Entry { name: name.into(), version: Some(version.into()), ..Default::default() }
    }

    fn disk_mod(name: &str, versions: &[&str]) -> InstalledMod {
        InstalledMod { name: name.into(), versions: versions.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    /// The UI (`src/lib/types.ts` Item) reads these exact camelCase keys; renaming one here breaks it silently.
    #[test]
    fn item_json_matches_ui_contract() {
        let mut c = Catalog::default();
        c.index.mods = vec![ModEntry { entry: info("Contra", "1.0"), ..Default::default() }];
        let items = mod_items(&c, &[]);
        let json = serde_json::to_value(&items[0]).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["cover", "dependsOn", "deprecated", "direct", "hue", "id", "kind", "latest", "links", "name", "parent", "summary", "versions"]);
        assert_eq!(json["kind"], "mod");
        assert_eq!(json["versions"][0], serde_json::json!({ "v": "1.0", "installed": false }));
        let exe = extra_item("contra", Kind::Exe, &info("E", "2"), &[]);
        assert_eq!(serde_json::to_value(&exe).unwrap()["kind"], "exe");
    }

    #[test]
    fn the_catalogs_other_versions_are_offered_with_their_own_picture_and_download() {
        use crate::index::{Download, VersionInfo};
        let others = vec![
            VersionInfo { version: "1.5".into(), image: Some("https://i/x.png".into()), page: Some("https://p/1.5".into()), ..Default::default() },
            VersionInfo { version: "1.0".into(), download: Some(Download { url: "https://h/1.0.zip".into(), ..Default::default() }), ..Default::default() },
        ];
        let (v, latest) = versions_dto(&["1.0".to_string()], Some("2.0"), &others);
        assert_eq!(latest, "2.0");
        assert_eq!(v.iter().map(|x| x.v.as_str()).collect::<Vec<_>>(), ["2.0", "1.5", "1.0"]);
        let v15 = &v[1];
        assert!(!v15.installed && v15.image.as_deref() == Some("https://i/x.png") && v15.page.as_deref() == Some("https://p/1.5"));
        // an installed version keeps no catalog page (its own source record fills it in) but still has its download offered for a reinstall
        let v10 = &v[2];
        assert!(v10.installed && v10.page.is_none() && v10.direct.as_ref().is_some_and(|d| d.url == "https://h/1.0.zip"));
    }

    #[test]
    fn slugs_are_stable() {
        assert_eq!(slug("Rise Of The Reds"), "rise-of-the-reds");
        assert_eq!(slug("  C&C: Contra (009) "), "c-c-contra-009");
    }

    #[test]
    fn parses_colors() {
        assert_eq!(hue_from_color("#ff0000"), Some(0));
        assert_eq!(hue_from_color("#00ff00"), Some(120));
        assert_eq!(hue_from_color("#FFFF0000"), Some(0)); // AARRGGBB: opaque red
        assert_eq!(hue_from_color("#FF0000ff"), Some(240)); // opaque blue, not red with alpha ff
        assert_eq!(hue_from_color("#808080"), None);
        assert_eq!(hue_from_color("DarkGray"), None);
    }

    #[test]
    fn merges_catalog_with_disk() {
        let mut c = Catalog::default();
        c.index.mods = vec![
            ModEntry { entry: Entry { colors: Some(Colors { active: Some("#00ff00".into()), ..Default::default() }), ..info("Contra", "009 Final") }, ..Default::default() },
            entry("Shockwave"),
        ];
        let disk = vec![disk_mod("contra", &["009 Beta 3"]), disk_mod("My Local Mod", &["1.0"])];
        let items = mod_items(&c, &disk);

        let contra = items.iter().find(|i| i.id == "contra").unwrap();
        assert_eq!(contra.hue, 120);
        assert_eq!(contra.latest, "009 Final");
        assert_eq!(contra.versions, [
            VersionDto { v: "009 Final".into(), installed: false, page: None, image: None, direct: None, date: None },
            VersionDto { v: "009 Beta 3".into(), installed: true, page: None, image: None, direct: None, date: None },
        ]);

        let shock = items.iter().find(|i| i.id == "shockwave").unwrap();
        assert!(shock.versions.is_empty() || shock.versions.iter().all(|v| !v.installed));

        let local = items.iter().find(|i| i.id == "my-local-mod").unwrap();
        assert!(local.versions[0].installed);
        assert_eq!(items.last().unwrap().id, ORIGINAL_GAME_ID);
    }

    #[test]
    fn extras_merge_disk_and_keep_manual_ones() {
        let infos = ExtraInfos { patches: vec![info("Hotfix", "4")], ..Default::default() };
        let mut d = disk_mod("Contra", &["1"]);
        d.patches = vec![
            Extra { name: "Hotfix".into(), versions: vec!["3".into()] },
            Extra { name: "Homebrew".into(), versions: vec!["0.1".into()] },
        ];
        let items = extra_items("contra", &infos, Some(&d), &[]);
        let hotfix = items.iter().find(|i| i.name == "Hotfix").unwrap();
        assert_eq!(hotfix.id, "contra/patch/hotfix");
        assert_eq!(hotfix.latest, "4");
        assert!(hotfix.versions.iter().any(|v| v.v == "3" && v.installed));
        assert!(items.iter().any(|i| i.name == "Homebrew" && i.versions[0].installed));
    }

    #[test]
    fn exe_scoping_follows_dependency() {
        use crate::index::ExeEntry;
        let mut idx = Index::default();
        idx.executables = vec![
            ExeEntry { entry: info("global", "1"), for_mod: None },
            ExeEntry { entry: info("contra-only", "1"), for_mod: Some("Contra".into()) },
            ExeEntry { entry: info("other", "1"), for_mod: Some("Shockwave".into()) },
            ExeEntry { entry: info("stock", "1"), for_mod: Some("Original Game".into()) },
        ];
        let names = |e: Option<&ModEntry>| extras_for(&idx, e).exes.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(Some(&entry("Contra"))), ["global", "contra-only", "stock"]);
        assert_eq!(names(None), ["global", "contra-only", "other", "stock"]);
    }

    /// The copies that ship inside the app must be readable and hold no secrets; a direct link is https and says where the author published it.
    #[test]
    fn bundled_indexes_are_readable_and_hold_no_secrets() {
        for game in [crate::game::Game::Zh, crate::game::Game::Generals] {
            let text = game.bundled_index();
            let c = bundled(game);
            assert_eq!(c.index.game, game.key());
            assert!(!c.index.mods.is_empty());
            assert!(c.index.mods.iter().all(|m| !m.entry.name.trim().is_empty()));
            let lower = text.to_lowercase();
            for banned in ["s3host", "downloadlink", "secretkey", "publickey"] {
                assert!(!lower.contains(banned), "{banned} must not be in the index");
            }
            for m in &c.index.mods {
                for e in std::iter::once(&m.entry).chain(&m.patches).chain(&m.addons) {
                    if let Some(d) = &e.download {
                        assert!(d.url.starts_with("https://"), "{}: direct links must be https", e.name);
                        assert!(d.via.as_deref().is_some_and(|v| v.starts_with("https://")), "{}: a direct link must say where the author published it", e.name);
                    }
                }
            }
        }
    }

    #[test]
    fn a_direct_download_reaches_the_item_only_when_it_is_https() {
        let mut e = Entry { name: "X".into(), ..Default::default() };
        assert!(direct_of(Some(&e)).is_none());
        e.download = Some(crate::index::Download { url: "http://insecure/x.zip".into(), ..Default::default() });
        assert!(direct_of(Some(&e)).is_none(), "plain http is never offered");
        e.download = Some(crate::index::Download { url: "https://host/x.zip".into(), via: Some("https://a/b.yaml".into()), size: Some(5), sha256: None, asset: None });
        let d = direct_of(Some(&e)).unwrap();
        assert_eq!((d.url.as_str(), d.size), ("https://host/x.zip", Some(5)));
    }
}
