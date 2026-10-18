//! Custom maps. The game reads them from `<Documents>/Command and Conquer Generals Zero Hour Data/Maps/<Name>/<Name>.map`
//! (a folder per map, named like the map file; the map's `.str`, `.tga`, `.ini` ... sit beside it). Map packs come as an
//! archive or a folder with the maps in all sorts of layouts; importing puts every map in its own folder in the right place.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::error::{Error, Result};
use crate::game::Game;

/// `<Documents>/Command and Conquer Generals Zero Hour Data/Maps` (or the Generals data folder).
pub fn maps_dir(documents: &Path, game: Game) -> PathBuf {
    let folder = match game {
        Game::Zh => "Command and Conquer Generals Zero Hour Data",
        Game::Generals => "Command and Conquer Generals Data",
    };
    documents.join(folder).join("Maps")
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MapInfo {
    /// The folder (and map) name.
    pub name: String,
    pub bytes: u64,
    /// Last change, seconds since 1970.
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub added: Vec<String>,
    /// Maps that were there already and were overwritten (an updated version of the same map).
    pub replaced: Vec<String>,
}

fn dir_size(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

fn has_map_file(dir: &Path, name: &str) -> bool {
    fs::read_dir(dir).into_iter().flatten().flatten().any(|e| {
        let n = e.file_name().to_string_lossy().to_lowercase();
        n == format!("{}.map", name.to_lowercase())
    })
}

/// The maps in the game's Maps folder, by name.
pub fn list(dir: &Path) -> Vec<MapInfo> {
    let mut out: Vec<MapInfo> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            has_map_file(&e.path(), &name).then(|| MapInfo {
                bytes: dir_size(&e.path()),
                modified: e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()),
                name,
            })
        })
        .collect();
    out.sort_by_key(|m| m.name.to_lowercase());
    out
}

/// The preview picture of a map: `<Name>/<Name>.tga` (the file is returned as it is; the interface decodes it).
pub fn preview(dir: &Path, name: &str) -> Result<Vec<u8>> {
    let single = {
        let mut c = Path::new(name).components();
        matches!((c.next(), c.next()), (Some(std::path::Component::Normal(_)), None))
    };
    if !single {
        return Err(Error::msg("not a map name"));
    }
    let folder = dir.join(name);
    let wanted = format!("{}.tga", name.to_lowercase());
    let file = fs::read_dir(&folder)?.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == wanted).ok_or_else(|| Error::msg("this map has no preview"))?;
    if file.metadata()?.len() > 8 * 1024 * 1024 {
        return Err(Error::msg("the preview is too large"));
    }
    Ok(fs::read(file.path())?)
}

/// Delete one map (its folder). The name must be a plain folder name inside `dir`.
pub fn remove(dir: &Path, name: &str) -> Result<()> {
    let single = {
        let mut c = Path::new(name).components();
        matches!((c.next(), c.next()), (Some(std::path::Component::Normal(_)), None))
    };
    if !single {
        return Err(Error::msg("not a map name"));
    }
    let target = dir.join(name);
    if !target.is_dir() || !has_map_file(&target, name) {
        return Err(Error::msg("that map is not there"));
    }
    fs::remove_dir_all(target)?;
    Ok(())
}

fn copy_into(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)?.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_into(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn find_map_files(root: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(root).into_iter().flatten().flatten() {
        let p = e.path();
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            find_map_files(&p, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("map")) {
            out.push(p);
        }
    }
}

/// Files beside `<stem>.map` that belong to it: `<stem>.str`, `<stem>.tga`, `<stem>_art.tga`, `<stem>.ini` ...
fn belongs_to(file_name: &str, stem: &str) -> bool {
    let (f, s) = (file_name.to_lowercase(), stem.to_lowercase());
    f.strip_prefix(&s).is_some_and(|rest| rest.starts_with('.') || rest.starts_with('_'))
}

/// Files a map keeps under the same name whatever the map is called (the real maps have a `map.str`, a `map.ini`, a `solo.ini`).
const SHARED_NAMES: &[&str] = &["map.str", "map.ini", "solo.ini"];

/// The part of an unpacked archive that holds the maps: all of it, or the `subfolder` of it. A GitHub zip has one folder around everything
/// (`Repo-main/`); it is looked through, so `release` finds `Repo-main/release`.
pub fn pack_root(unpacked: &Path, subfolder: Option<&str>) -> Result<PathBuf> {
    let Some(sub) = subfolder.map(str::trim).filter(|s| !s.is_empty()) else { return Ok(unpacked.to_path_buf()) };
    if Path::new(sub).components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err(Error::msg("not a folder inside the archive"));
    }
    let entries: Vec<_> = fs::read_dir(unpacked)?.flatten().collect();
    let base = if entries.len() == 1 && entries[0].file_type().is_ok_and(|t| t.is_dir()) { entries[0].path() } else { unpacked.to_path_buf() };
    let target = base.join(sub);
    if target.is_dir() { Ok(target) } else { Err(Error::msg(format!("The archive has no folder called {sub}"))) }
}

/// Put every map found in `source` (an unpacked pack: a folder with maps in any layout) into `dest` as `<Name>/<Name>.map` with its
/// companion files. Existing maps of the same name are overwritten.
pub fn import_tree(source: &Path, dest: &Path) -> Result<ImportReport> {
    let mut maps = Vec::new();
    find_map_files(source, &mut maps);
    if maps.is_empty() {
        return Err(Error::msg("There is no map (.map file) in it."));
    }
    fs::create_dir_all(dest)?;
    let mut report = ImportReport::default();
    for map in maps {
        let stem = map.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let name = crate::target::sanitize_component(&stem);
        let parent = map.parent().ok_or_else(|| Error::msg("odd path"))?;
        let target = dest.join(&name);
        let existed = target.exists();
        let in_own_folder = parent.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(&stem));
        if in_own_folder {
            copy_into(parent, &target)?;
        } else {
            fs::create_dir_all(&target)?;
            // with a single map in the folder, the files every map has (map.str ...) are its own
            let alone = fs::read_dir(parent)?.flatten().filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("map"))).count() == 1;
            for e in fs::read_dir(parent)?.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if e.file_type()?.is_file() && (belongs_to(&n, &stem) || (alone && SHARED_NAMES.contains(&n.to_lowercase().as_str()))) {
                    fs::copy(e.path(), target.join(e.file_name()))?;
                }
            }
        }
        if existed { report.replaced.push(name) } else { report.added.push(name) }
    }
    report.added.sort();
    report.replaced.sort();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"data").unwrap();
    }

    #[test]
    fn maps_in_their_own_folders_are_taken_as_they_are() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        put(src.path(), "Pack/Maps/Alpha Valley/Alpha Valley.map");
        put(src.path(), "Pack/Maps/Alpha Valley/Alpha Valley.str");
        put(src.path(), "Pack/Maps/Alpha Valley/Alpha Valley_art.tga");
        put(src.path(), "Pack/Maps/Beta/Beta.map");
        let r = import_tree(src.path(), dst.path()).unwrap();
        assert_eq!(r.added, ["Alpha Valley", "Beta"]);
        assert!(dst.path().join("Alpha Valley/Alpha Valley_art.tga").is_file());
        assert_eq!(list(dst.path()).iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Alpha Valley", "Beta"]);
    }

    #[test]
    fn loose_maps_get_a_folder_with_their_own_files_only() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        put(src.path(), "Gamma.map");
        put(src.path(), "Gamma.str");
        put(src.path(), "Gamma_art.tga");
        put(src.path(), "Gammatron.map");
        put(src.path(), "readme.txt");
        put(src.path(), "map.str");
        let r = import_tree(src.path(), dst.path()).unwrap();
        assert_eq!(r.added, ["Gamma", "Gammatron"]);
        assert!(dst.path().join("Gamma/Gamma.str").is_file() && dst.path().join("Gamma/Gamma_art.tga").is_file());
        // "Gammatron" is another map, not a companion file of "Gamma"
        assert!(!dst.path().join("Gamma/Gammatron.map").exists());
        assert!(!dst.path().join("Gamma/readme.txt").exists());
        // two maps share this folder, so map.str cannot be told apart: it is left out
        assert!(!dst.path().join("Gamma/map.str").exists());
    }

    #[test]
    fn a_single_loose_map_takes_the_files_every_map_has() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        put(src.path(), "Zeta.map");
        put(src.path(), "Zeta.tga");
        put(src.path(), "map.str");
        put(src.path(), "solo.ini");
        put(src.path(), "license.txt");
        import_tree(src.path(), dst.path()).unwrap();
        assert!(dst.path().join("Zeta/map.str").is_file() && dst.path().join("Zeta/solo.ini").is_file());
        assert!(!dst.path().join("Zeta/license.txt").exists());
    }

    #[test]
    fn importing_again_replaces_and_reports_it() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        put(src.path(), "Delta/Delta.map");
        assert_eq!(import_tree(src.path(), dst.path()).unwrap().added, ["Delta"]);
        let again = import_tree(src.path(), dst.path()).unwrap();
        assert!(again.added.is_empty() && again.replaced == ["Delta"]);
    }

    #[test]
    fn a_preview_is_read_from_the_maps_own_folder() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "Eta/Eta.map");
        put(d.path(), "Eta/ETA.TGA");
        put(d.path(), "Theta/Theta.map");
        assert_eq!(preview(d.path(), "Eta").unwrap(), b"data");
        assert!(preview(d.path(), "Theta").is_err(), "no picture");
        assert!(preview(d.path(), "../Eta").is_err());
    }

    #[test]
    fn only_the_subfolder_of_a_pack_is_imported() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        put(src.path(), "Repo-main/release/Good/Good.map");
        put(src.path(), "Repo-main/backlog/Unfinished/Unfinished.map");
        put(src.path(), "Repo-main/README.md");
        let root = pack_root(src.path(), Some("release")).unwrap();
        assert_eq!(import_tree(&root, dst.path()).unwrap().added, ["Good"]);
        assert!(pack_root(src.path(), Some("nope")).is_err());
        assert!(pack_root(src.path(), Some("../x")).is_err());
        // without a subfolder everything is taken
        assert_eq!(import_tree(&pack_root(src.path(), None).unwrap(), tempfile::tempdir().unwrap().path()).unwrap().added.len(), 2);
    }

    #[test]
    fn a_pack_without_maps_is_refused_and_removal_is_safe() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        put(src.path(), "notes.txt");
        assert!(import_tree(src.path(), dst.path()).is_err());
        put(dst.path(), "Eps/Eps.map");
        assert!(remove(dst.path(), "..").is_err());
        assert!(remove(dst.path(), "a/b").is_err());
        assert!(remove(dst.path(), "Nope").is_err());
        remove(dst.path(), "Eps").unwrap();
        assert!(list(dst.path()).is_empty());
    }
}
