//! Builds the library the UI shows by merging the remote catalog with what is on disk.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::library::{Extra, InstalledMod};
use crate::manifest::{ExeEntry, Fetcher, Manifest, ModEntry, ModType, VersionInfo};
use crate::version;

pub const ORIGINAL_GAME: &str = "Original Game";
pub const ORIGINAL_GAME_ID: &str = "original-game";

/// Manifest plus the latest-version record of every mod (index-aligned with `manifest.mod_datas`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub manifest: Manifest,
    pub mods: Vec<Option<VersionInfo>>,
    pub fetched_at: u64,
}

pub async fn fetch(fetcher: &Fetcher, manifest_url: &str) -> Result<Catalog> {
    let manifest = fetcher.manifest(manifest_url).await?;
    let urls: Vec<String> = manifest.mod_datas.iter().map(|m| m.link.clone()).collect();
    let mods = fetcher.version_infos(&urls).await;
    let fetched_at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Ok(Catalog { manifest, mods, fetched_at })
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
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Links {
    pub moddb: Option<String>,
    pub discord: Option<String>,
    pub news: Option<String>,
    pub network: Option<String>,
    pub support: Option<String>,
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

fn hue_for(info: Option<&VersionInfo>, name: &str) -> u16 {
    info.and_then(|i| i.colors.as_ref())
        .and_then(|c| c.active.as_deref().and_then(hue_from_color).or_else(|| c.border.as_deref().and_then(hue_from_color)))
        .unwrap_or_else(|| name_hue(name))
}

fn versions_dto(installed: &[String], latest: Option<&str>) -> (Vec<VersionDto>, String) {
    let mut all: Vec<String> = installed.to_vec();
    if let Some(l) = latest {
        if !all.iter().any(|v| v.eq_ignore_ascii_case(l)) {
            all.push(l.to_string());
        }
    }
    version::sort_desc(&mut all);
    let dto = all
        .iter()
        .map(|v| VersionDto { v: v.clone(), installed: installed.iter().any(|i| i.eq_ignore_ascii_case(v)) })
        .collect::<Vec<_>>();
    let newest = latest.map(str::to_string).or_else(|| all.first().cloned()).unwrap_or_default();
    (dto, newest)
}

fn links_of(info: Option<&VersionInfo>) -> Links {
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

    for (entry, info) in catalog.manifest.mod_datas.iter().zip(catalog.mods.iter()) {
        let display = info.as_ref().map(|i| i.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(&entry.name);
        let on_disk = find_installed(disk, &[display, &entry.name]);
        if let Some(d) = on_disk {
            matched.push(&d.name);
        }
        let installed = on_disk.map(|d| d.versions.clone()).unwrap_or_default();
        let latest = info.as_ref().and_then(|i| i.version.as_deref());
        let (versions, latest) = versions_dto(&installed, latest);
        items.push(Item {
            id: slug(&entry.name),
            kind: Kind::Mod,
            name: display.to_string(),
            summary: String::new(), // the repo has no description field; NetworkInfo is a link, exposed under links.network
            hue: hue_for(info.as_ref(), display),
            versions,
            latest,
            parent: None,
            depends_on: None,
            links: links_of(info.as_ref()),
            cover: info.as_ref().and_then(|i| i.image_link.clone()),
            deprecated: info.as_ref().is_some_and(|i| i.deprecated),
        });
    }

    for d in disk.iter().filter(|d| !matched.contains(&d.name.as_str()) && !d.versions.is_empty()) {
        let (versions, latest) = versions_dto(&d.versions, None);
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
        });
    }

    items.push(Item {
        id: ORIGINAL_GAME_ID.into(),
        kind: Kind::Mod,
        name: ORIGINAL_GAME.into(),
        summary: String::new(),
        hue: 220,
        versions: vec![VersionDto { v: "Stock".into(), installed: true }],
        latest: "Stock".into(),
        parent: None,
        depends_on: None,
        links: Links::default(),
        cover: None,
        deprecated: false,
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
fn extra_item(mod_id: &str, kind: Kind, info: &VersionInfo, disks: &[&[Extra]]) -> Item {
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
    let (versions, latest) = versions_dto(&installed, info.version.as_deref());
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
    }
}

/// Fetched records for one mod's patches/addons/executables.
#[derive(Debug, Clone, Default)]
pub struct ExtraInfos {
    pub patches: Vec<VersionInfo>,
    pub addons: Vec<VersionInfo>,
    pub exes: Vec<VersionInfo>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Which manifest lists and executables belong to `mod_name` (or the original game when `None`).
pub fn extra_urls<'a>(manifest: &'a Manifest, entry: Option<&'a ModEntry>) -> (Vec<String>, Vec<String>, Vec<&'a ExeEntry>) {
    let (patches, addons) = match entry {
        Some(e) => (e.patches.clone(), e.addons.clone()),
        None => (manifest.original_game_patches.clone(), manifest.original_game_addons.clone()),
    };
    let exes = manifest
        .executables
        .iter()
        .filter(|x| match (&x.dependency, entry) {
            (Some(dep), Some(e)) if !dep.is_empty() && !same_name(dep, ORIGINAL_GAME) => same_name(dep, &e.name),
            // no dependency (or "Original Game"): usable with any mod, like in 1.x
            _ => true,
        })
        .collect();
    (patches, addons, exes)
}

pub async fn fetch_extras(fetcher: &Fetcher, manifest: &Manifest, entry: Option<&ModEntry>) -> ExtraInfos {
    let (p, a, e) = extra_urls(manifest, entry);
    let exe_urls: Vec<String> = e.iter().map(|x| x.link.clone()).collect();
    let (patches, addons, exes) =
        futures::join!(fetcher.version_infos(&p), fetcher.version_infos(&a), fetcher.version_infos(&exe_urls));
    ExtraInfos {
        patches: patches.into_iter().flatten().collect(),
        addons: addons.into_iter().flatten().collect(),
        exes: exes.into_iter().flatten().collect(),
    }
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
                let synthetic = VersionInfo { kind: ModType::Mod, name: x.name.clone(), ..Default::default() };
                out.push(extra_item(mod_id, kind, &synthetic, &[own]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Colors;

    fn entry(name: &str) -> ModEntry {
        ModEntry { name: name.into(), link: format!("https://x/{name}.yaml"), ..Default::default() }
    }

    fn info(name: &str, version: &str) -> VersionInfo {
        VersionInfo { name: name.into(), version: Some(version.into()), ..Default::default() }
    }

    fn disk_mod(name: &str, versions: &[&str]) -> InstalledMod {
        InstalledMod { name: name.into(), versions: versions.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    /// The UI (`src/lib/types.ts` Item) reads these exact camelCase keys; renaming one here breaks it silently.
    #[test]
    fn item_json_matches_ui_contract() {
        let mut c = Catalog::default();
        c.manifest.mod_datas = vec![entry("Contra")];
        c.mods = vec![Some(info("Contra", "1.0"))];
        let items = mod_items(&c, &[]);
        let json = serde_json::to_value(&items[0]).unwrap();
        let mut keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["cover", "dependsOn", "deprecated", "hue", "id", "kind", "latest", "links", "name", "parent", "summary", "versions"]);
        assert_eq!(json["kind"], "mod");
        assert_eq!(json["versions"][0], serde_json::json!({ "v": "1.0", "installed": false }));
        let exe = extra_item("contra", Kind::Exe, &info("E", "2"), &[]);
        assert_eq!(serde_json::to_value(&exe).unwrap()["kind"], "exe");
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
        c.manifest.mod_datas = vec![entry("Contra"), entry("Shockwave")];
        c.mods = vec![
            Some(VersionInfo { colors: Some(Colors { active: Some("#00ff00".into()), ..Default::default() }), ..info("Contra", "009 Final") }),
            None, // dead link in the catalog must not drop the entry
        ];
        let disk = vec![disk_mod("contra", &["009 Beta 3"]), disk_mod("My Local Mod", &["1.0"])];
        let items = mod_items(&c, &disk);

        let contra = items.iter().find(|i| i.id == "contra").unwrap();
        assert_eq!(contra.hue, 120);
        assert_eq!(contra.latest, "009 Final");
        assert_eq!(contra.versions, [
            VersionDto { v: "009 Final".into(), installed: false },
            VersionDto { v: "009 Beta 3".into(), installed: true },
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
        let mut m = Manifest::default();
        m.executables = vec![
            ExeEntry { name: "global".into(), link: "g".into(), dependency: None },
            ExeEntry { name: "contra-only".into(), link: "c".into(), dependency: Some("Contra".into()) },
            ExeEntry { name: "other".into(), link: "o".into(), dependency: Some("Shockwave".into()) },
        ];
        let e = entry("Contra");
        let (_, _, exes) = extra_urls(&m, Some(&e));
        let names: Vec<_> = exes.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["global", "contra-only"]);
    }
}
