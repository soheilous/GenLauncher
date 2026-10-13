//! What is installed on disk (see [`crate::layout`] for the folder structure).
//! Disk is the source of truth for "installed"; nothing here trusts a cached flag.

use std::fs;
use std::path::{Path, PathBuf};

use crate::layout;

pub const PATCHES_DIR: &str = layout::PATCHES;
pub const ADDONS_DIR: &str = layout::ADDONS;
pub const EXES_DIR: &str = layout::EXES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extra {
    pub name: String,
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstalledMod {
    pub name: String,
    pub versions: Vec<String>,
    pub patches: Vec<Extra>,
    pub addons: Vec<Extra>,
    pub exes: Vec<Extra>,
}

fn subdirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<_> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect();
    v.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    v
}

pub(crate) fn contains_file(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(dir) else { return false };
    for e in rd.flatten() {
        match e.file_type() {
            Ok(t) if t.is_file() => return true,
            Ok(t) if t.is_dir() && contains_file(&e.path()) => return true,
            _ => {}
        }
    }
    false
}

fn installed_versions(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = subdirs(dir).into_iter().filter(|(_, p)| contains_file(p)).map(|(n, _)| n).collect();
    crate::version::sort_desc(&mut v);
    v
}

fn extras(dir: &Path) -> Vec<Extra> {
    subdirs(dir)
        .into_iter()
        .filter_map(|(name, path)| {
            let versions = installed_versions(&path);
            (!versions.is_empty()).then_some(Extra { name, versions })
        })
        .collect()
}

/// Delete one installed version folder and its source record, then the folders that become empty because of it
/// (the item's own folder, and a `patches` / `addons` / `exes` list). Never goes above `mods_root`.
pub fn remove_version(dir: &Path, mods_root: &Path) -> crate::error::Result<()> {
    fs::remove_dir_all(dir)?;
    crate::record::remove(dir);
    for parent in dir.ancestors().skip(1).take(3) {
        if parent == mods_root || fs::remove_dir(parent).is_err() {
            break; // the library root, or something else still lives there
        }
    }
    Ok(())
}

pub fn mods_root(game_dir: &Path) -> PathBuf {
    layout::mods(game_dir)
}

pub fn scan(game_dir: &Path) -> Vec<InstalledMod> {
    subdirs(&mods_root(game_dir))
        .into_iter()
        .map(|(name, path)| {
            let mut versions: Vec<String> = subdirs(&path)
                .into_iter()
                .filter(|(n, p)| ![PATCHES_DIR, ADDONS_DIR, EXES_DIR].contains(&n.as_str()) && contains_file(p))
                .map(|(n, _)| n)
                .collect();
            crate::version::sort_desc(&mut versions);
            InstalledMod {
                name,
                versions,
                patches: extras(&path.join(PATCHES_DIR)),
                addons: extras(&path.join(ADDONS_DIR)),
                exes: extras(&path.join(EXES_DIR)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    #[test]
    fn scans_the_layout() {
        let d = tempfile::tempdir().unwrap();
        let g = d.path().join(".genlauncher/mods");
        file(&g.join("Contra/009 Beta 3/a.big"));
        file(&g.join("Contra/009 Final/data/x.ini")); // nested file counts
        file(&g.join("Contra/patches/Hotfix/3/p.big"));
        file(&g.join("Contra/addons/Maps/1.0/m.map"));
        file(&g.join("Contra/exes/Modded/2.0/modded.exe"));
        let mods = scan(d.path());
        assert_eq!(mods.len(), 1);
        let m = &mods[0];
        assert_eq!(m.versions, ["009 Final", "009 Beta 3"]);
        assert_eq!(m.patches, [Extra { name: "Hotfix".into(), versions: vec!["3".into()] }]);
        assert_eq!(m.addons[0].name, "Maps");
        assert_eq!(m.exes[0].versions, ["2.0"]);
    }

    #[test]
    fn empty_version_folders_are_not_installed() {
        let d = tempfile::tempdir().unwrap();
        let g = d.path().join(".genlauncher/mods");
        fs::create_dir_all(g.join("M/2.0")).unwrap(); // empty
        file(&g.join("M/3.0/ok.big"));
        assert_eq!(scan(d.path())[0].versions, ["3.0"]);
    }

    #[test]
    fn removing_the_last_version_tidies_up_but_keeps_siblings() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join(".genlauncher/mods");
        file(&root.join("Contra/10/a.big"));
        file(&root.join("Contra/patches/Hotfix/3/p.big"));
        file(&root.join("Contra/patches/Hotfix/4/p.big"));
        file(&root.join("Contra/patches/Other/1/p.big"));
        file(&root.join("Solo/patches/Only/1/p.big"));

        // one of two versions: the item stays
        remove_version(&root.join("Contra/patches/Hotfix/3"), &root).unwrap();
        assert!(root.join("Contra/patches/Hotfix/4").exists());
        // the last version of an item: its folder goes, its siblings stay
        remove_version(&root.join("Contra/patches/Hotfix/4"), &root).unwrap();
        assert!(!root.join("Contra/patches/Hotfix").exists());
        assert!(root.join("Contra/patches/Other/1/p.big").exists() && root.join("Contra/10/a.big").exists());
        // the last patch of a mod that has nothing else: the empty patches list goes, the mod folder goes too
        remove_version(&root.join("Solo/patches/Only/1"), &root).unwrap();
        assert!(!root.join("Solo").exists());
        assert!(root.exists());
    }

    #[test]
    fn missing_library_is_empty() {
        let d = tempfile::tempdir().unwrap();
        assert!(scan(d.path()).is_empty());
    }
}
