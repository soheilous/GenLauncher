//! Where an item (identified by its UI id) lives or will live on disk, and where it comes from.
//! Folder layout is the one GenLauncher 1.x used so both launchers see the same installs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::catalog::{self, Catalog, ExtraInfos, Kind, ORIGINAL_GAME, ORIGINAL_GAME_ID, slug};
use crate::error::{Error, Result};
use crate::library::{self, InstalledMod};
use crate::manifest::VersionInfo;

#[derive(Debug, Clone)]
pub struct Located {
    pub kind: Kind,
    pub display: String,
    /// Folder that contains this item's version folders.
    pub root: PathBuf,
    /// Versions currently on disk, newest first.
    pub installed: Vec<String>,
    pub info: Option<VersionInfo>,
}

/// A folder name that is valid on Windows and Linux.
pub fn sanitize_component(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    if cleaned.is_empty() { "_".into() } else { cleaned }
}

fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

struct ModLoc<'a> {
    folder: String,
    display: String,
    info: Option<&'a VersionInfo>,
    disk: Option<&'a InstalledMod>,
}

fn locate_mod<'a>(catalog: &'a Catalog, disk: &'a [InstalledMod], id: &str) -> Result<ModLoc<'a>> {
    if id == ORIGINAL_GAME_ID {
        return Ok(ModLoc { folder: ORIGINAL_GAME.into(), display: ORIGINAL_GAME.into(), info: None, disk: disk.iter().find(|d| eq(&d.name, ORIGINAL_GAME)) });
    }
    if let Some(i) = catalog.manifest.mod_datas.iter().position(|m| slug(&m.name) == id) {
        let entry = &catalog.manifest.mod_datas[i];
        let info = catalog.mods.get(i).and_then(|m| m.as_ref());
        let display = info.map(|i| i.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(&entry.name).to_string();
        let on_disk = disk.iter().find(|d| eq(&d.name, &display) || eq(&d.name, &entry.name));
        let folder = on_disk.map(|d| d.name.clone()).unwrap_or_else(|| sanitize_component(&display));
        return Ok(ModLoc { folder, display, info, disk: on_disk });
    }
    disk.iter()
        .find(|d| slug(&d.name) == id)
        .map(|d| ModLoc { folder: d.name.clone(), display: d.name.clone(), info: None, disk: Some(d) })
        .ok_or_else(|| Error::msg("unknown item"))
}

pub fn locate(
    game_dir: &Path,
    catalog: &Catalog,
    extras: &HashMap<String, ExtraInfos>,
    disk: &[InstalledMod],
    id: &str,
) -> Result<Located> {
    let parts: Vec<&str> = id.split('/').collect();
    let glm = library::mods_root(game_dir);

    if parts.len() == 1 {
        let m = locate_mod(catalog, disk, id)?;
        return Ok(Located {
            kind: Kind::Mod,
            display: m.display,
            root: glm.join(&m.folder),
            installed: m.disk.map(|d| d.versions.clone()).unwrap_or_default(),
            info: m.info.cloned(),
        });
    }
    if parts.len() != 3 {
        return Err(Error::msg("unknown item"));
    }

    let (mod_id, kind) = (parts[0], match parts[1] {
        "patch" => Kind::Patch,
        "addon" => Kind::Addon,
        "exe" => Kind::Exe,
        _ => return Err(Error::msg("unknown item")),
    });
    let parent = locate_mod(catalog, disk, mod_id)?;
    let infos = extras.get(mod_id);
    let list = infos.map(|e| match kind {
        Kind::Patch => &e.patches,
        Kind::Addon => &e.addons,
        _ => &e.exes,
    });
    let info = list.and_then(|l| l.iter().find(|i| catalog::extra_id(mod_id, kind, &i.name) == id));

    // 1.x filed an extra under the folder named by DependenceName: a mod, or the patch an addon needs.
    let owner_name = info.and_then(|i| i.dependence_name.as_deref()).filter(|d| !d.is_empty());
    let owner_disk = owner_name.and_then(|dep| disk.iter().find(|d| eq(&d.name, dep)));
    let owner_folder = match (owner_name, owner_disk) {
        (_, Some(d)) => d.name.clone(),
        (Some(dep), None) => sanitize_component(dep),
        (None, None) => parent.folder.clone(),
    };

    let sub = match kind {
        Kind::Patch => library::PATCHES_DIR,
        Kind::Addon => library::ADDONS_DIR,
        _ => library::EXES_DIR,
    };
    let pick = |d: &InstalledMod| match kind {
        Kind::Patch => d.patches.clone(),
        Kind::Addon => d.addons.clone(),
        _ => d.exes.clone(),
    };
    // existing folder name + versions: look in the owner folder first, then the mod's own
    let existing = |name_matches: &dyn Fn(&str) -> bool| {
        [owner_disk, parent.disk].into_iter().flatten().find_map(|d| pick(d).into_iter().find(|e| name_matches(&e.name)).map(|e| (d.name.clone(), e)))
    };
    let found = match info {
        Some(i) => existing(&|n| eq(n, &i.name)),
        None => existing(&|n| catalog::extra_id(mod_id, kind, n) == id),
    };
    let (folder_owner, name, installed) = match (&found, info) {
        (Some((owner, e)), _) => (owner.clone(), e.name.clone(), e.versions.clone()),
        (None, Some(i)) => (owner_folder, sanitize_component(&i.name), Vec::new()),
        (None, None) => return Err(Error::msg("unknown item")),
    };
    Ok(Located {
        kind,
        display: info.map(|i| i.name.clone()).unwrap_or_else(|| name.clone()),
        root: glm.join(&folder_owner).join(sub).join(&name),
        installed,
        info: info.cloned(),
    })
}

/// Where a user-added item goes: its UI id and the folder that will hold its version folders.
/// Adding a version of something that already exists reuses that item's folder.
pub fn new_item(
    game_dir: &Path,
    catalog: &Catalog,
    extras: &HashMap<String, ExtraInfos>,
    disk: &[InstalledMod],
    kind: Kind,
    name: &str,
    parent_id: Option<&str>,
) -> Result<(String, PathBuf)> {
    let name = name.trim();
    let folder = sanitize_component(name);
    if name.is_empty() || slug(name).is_empty() {
        return Err(Error::msg("Give it a name with at least one letter or number"));
    }
    if kind == Kind::Mod {
        let id = slug(name);
        if id == ORIGINAL_GAME_ID {
            return Err(Error::msg("That name is reserved for the stock game. Choose another name."));
        }
        let existing = disk.iter().find(|d| slug(&d.name) == id).map(|d| d.name.clone());
        return Ok((id, library::mods_root(game_dir).join(existing.unwrap_or(folder))));
    }

    let parent_id = parent_id.filter(|p| !p.is_empty()).ok_or_else(|| Error::msg("Choose which mod this belongs to"))?;
    let parent = locate(game_dir, catalog, extras, disk, parent_id)?;
    let id = catalog::extra_id(parent_id, kind, name);
    let (sub, pick): (&str, fn(&InstalledMod) -> &Vec<library::Extra>) = match kind {
        Kind::Patch => (library::PATCHES_DIR, |d| &d.patches),
        Kind::Addon => (library::ADDONS_DIR, |d| &d.addons),
        _ => (library::EXES_DIR, |d| &d.exes),
    };
    let existing = disk
        .iter()
        .filter(|d| parent.root.file_name().is_some_and(|f| f.to_string_lossy() == d.name))
        .flat_map(|d| pick(d).iter())
        .find(|e| slug(&e.name) == slug(name))
        .map(|e| e.name.clone());
    Ok((id, parent.root.join(sub).join(existing.unwrap_or(folder))))
}

/// Folder of one installed version, only if it really is below `.genlauncher/mods/` and exists.
pub fn version_dir(game_dir: &Path, loc: &Located, version: &str) -> Result<PathBuf> {
    let v = loc.installed.iter().find(|v| eq(v, version)).ok_or_else(|| Error::msg("that version is not installed"))?;
    // the version must be exactly one plain folder name, never `..`, `a/b` or an absolute path
    let single = {
        let mut c = Path::new(v).components();
        matches!((c.next(), c.next()), (Some(std::path::Component::Normal(_)), None))
    };
    let dir = loc.root.join(v);
    let glm = library::mods_root(game_dir);
    let inside = match (dir.canonicalize(), glm.canonicalize()) {
        (Ok(d), Ok(g)) => d.starts_with(&g) && d != g,
        _ => false,
    };
    if !single || !inside || !dir.is_dir() {
        return Err(Error::msg("refusing to touch a folder outside the mod library"));
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Extra;
    use crate::manifest::{ModEntry};

    fn catalog_with(name: &str, info: VersionInfo) -> Catalog {
        let mut c = Catalog::default();
        c.manifest.mod_datas = vec![ModEntry { name: name.into(), link: "x".into(), ..Default::default() }];
        c.mods = vec![Some(info)];
        c
    }

    fn info(name: &str, version: &str) -> VersionInfo {
        VersionInfo { name: name.into(), version: Some(version.into()), ..Default::default() }
    }

    #[test]
    fn extras_follow_the_old_folder_layout() {
        let cat = catalog_with("Contra", info("Contra", "10"));
        let mut hotfix = info("Hotfix", "4");
        hotfix.download_link = Some("https://x/h.zip".into());
        let mut maps = info("Maps", "2");
        maps.download_link = Some("https://x/m.zip".into());
        maps.dependence_name = Some("Hotfix".into()); // addon that needs the patch: filed under the patch's folder
        let mut extras = HashMap::new();
        extras.insert("contra".to_string(), ExtraInfos { patches: vec![hotfix], addons: vec![maps], exes: vec![] });
        let disk = vec![InstalledMod {
            name: "Contra".into(),
            versions: vec!["10".into()],
            patches: vec![Extra { name: "Hotfix".into(), versions: vec!["3".into()] }],
            ..Default::default()
        }];

        let p = locate(Path::new("G"), &cat, &extras, &disk, "contra/patch/hotfix").unwrap();
        assert_eq!(p.installed, ["3"]);
        assert!(p.root.to_string_lossy().replace('\\', "/").ends_with(".genlauncher/mods/Contra/patches/Hotfix"), "{:?}", p.root);

        let a = locate(Path::new("G"), &cat, &extras, &disk, "contra/addon/maps").unwrap();
        assert!(a.root.to_string_lossy().replace('\\', "/").ends_with(".genlauncher/mods/Hotfix/addons/Maps"), "{:?}", a.root);
    }

    #[test]
    fn errors_are_clear() {
        let cat = catalog_with("Contra", info("Contra", "10"));
        assert!(locate(Path::new("G"), &cat, &HashMap::new(), &[], "nope").is_err());
        assert!(locate(Path::new("G"), &cat, &HashMap::new(), &[], "contra/hack/x").is_err());
    }

    #[test]
    fn new_items_land_in_the_right_folders() {
        let g = Path::new("G");
        let cat = catalog_with("Contra", info("Contra", "10"));
        let disk = vec![InstalledMod {
            name: "Contra".into(),
            versions: vec!["10".into()],
            addons: vec![library::Extra { name: "Extra Maps".into(), versions: vec!["1".into()] }],
            ..Default::default()
        }];
        let none = HashMap::new();
        let norm = |p: PathBuf| p.to_string_lossy().replace('\\', "/");

        // a brand-new mod, with an unsafe character in its name
        let (id, root) = new_item(g, &cat, &none, &disk, Kind::Mod, "  My: Mod ", None).unwrap();
        assert_eq!(id, "my-mod");
        assert!(norm(root).ends_with(".genlauncher/mods/My_ Mod"));
        // another version of an existing mod (different capitalisation) reuses its folder
        let (id, root) = new_item(g, &cat, &none, &disk, Kind::Mod, "CONTRA", None).unwrap();
        assert_eq!(id, "contra");
        assert!(norm(root).ends_with(".genlauncher/mods/Contra"));
        // an addon for Contra; an existing addon of the same name reuses its folder
        let (id, root) = new_item(g, &cat, &none, &disk, Kind::Addon, "extra maps", Some("contra")).unwrap();
        assert_eq!(id, "contra/addon/extra-maps");
        assert!(norm(root).ends_with(".genlauncher/mods/Contra/addons/Extra Maps"));
        let (_, root) = new_item(g, &cat, &none, &disk, Kind::Patch, "Fix", Some("contra")).unwrap();
        assert!(norm(root).ends_with("Contra/patches/Fix"));
        // problems are explained
        assert!(new_item(g, &cat, &none, &disk, Kind::Mod, "  ", None).is_err());
        assert!(new_item(g, &cat, &none, &disk, Kind::Mod, "***", None).unwrap_err().to_string().contains("letter"));
        // the stock game's slot cannot be taken by a mod, whatever the capitalisation or punctuation
        for reserved in ["Original Game", "original-game", " ORIGINAL   game "] {
            assert!(new_item(g, &cat, &none, &disk, Kind::Mod, reserved, None).unwrap_err().to_string().contains("reserved"), "{reserved}");
        }
        assert!(new_item(g, &cat, &none, &disk, Kind::Patch, "Fix", None).unwrap_err().to_string().contains("which mod"));
        assert!(new_item(g, &cat, &none, &disk, Kind::Patch, "Fix", Some("unknown-mod")).is_err());
    }

    #[test]
    fn uninstall_refuses_paths_outside_glm() {
        let d = tempfile::tempdir().unwrap();
        let g = d.path();
        std::fs::create_dir_all(g.join(".genlauncher/mods/Mod/1.0")).unwrap();
        let loc = Located { kind: Kind::Mod, display: "Mod".into(), root: g.join(".genlauncher/mods/Mod"), installed: vec!["1.0".into(), "..".into()], info: None };
        assert!(version_dir(g, &loc, "1.0").is_ok());
        assert!(version_dir(g, &loc, "9.9").is_err());
        let evil = Located { root: g.join("Data"), ..loc };
        assert!(version_dir(g, &evil, "1.0").is_err());
        assert!(version_dir(g, &Located { installed: vec!["..".into()], root: g.join(".genlauncher/mods/Mod"), kind: Kind::Mod, display: "x".into(), info: None }, "..").is_err());
    }
}
