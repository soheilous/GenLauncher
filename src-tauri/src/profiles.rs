//! Saved games and replays per mod (and mod version).
//!
//! The game keeps every save and replay in one place (`<Documents>/Command and Conquer Generals Zero Hour Data/{Save, Replays}`),
//! whatever mod made them, and a save or replay of one mod does not work with another. With this switched on, the launcher keeps
//! one pair of folders per *profile* (a mod and its version) under `GenLauncher Profiles/<profile>/{Save,Replays}`, and before the game
//! starts it puts the profile of the mod being started where the game looks. The folders that are not in use are moved aside, never
//! copied or deleted, so nothing is lost and a switch takes a moment (it is a rename on the same drive).
//!
//! The profile whose files are in the game's own folders is written in `GenLauncher Profiles/.active`.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::error::{Error, Result};

const ROOT: &str = "GenLauncher Profiles";
const ACTIVE: &str = ".active";
const SAVE: &str = "Save";
const REPLAYS: &str = "Replays";
/// What was in the game's folders before the launcher started to keep them apart.
pub const SHARED: &str = "everything-before";

pub fn root(data: &Path) -> PathBuf {
    data.join(ROOT)
}

/// A profile name that is a plain folder name (it comes from a mod id and a version).
pub fn clean_key(key: &str) -> String {
    let k = crate::target::sanitize_component(key);
    if k == ACTIVE || k == "." || k == ".." { "profile".into() } else { k }
}

pub fn active(data: &Path) -> Option<String> {
    let k = fs::read_to_string(root(data).join(ACTIVE)).ok()?.trim().to_string();
    (!k.is_empty()).then_some(k)
}

fn set_active(data: &Path, key: &str) -> Result<()> {
    fs::create_dir_all(root(data))?;
    fs::write(root(data).join(ACTIVE), key)?;
    Ok(())
}

/// Where the Save / Replays folder of a profile is right now: the game's own folder for the active profile, else the profile's.
fn folder(data: &Path, key: &str, kind: &str) -> PathBuf {
    if active(data).as_deref() == Some(key) { data.join(kind) } else { root(data).join(key).join(kind) }
}

fn move_dir(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }
    if let Some(p) = to.parent() {
        fs::create_dir_all(p)?;
    }
    if to.exists() {
        // the target exists (an empty folder the game made, say): fold the files in, never overwriting
        for e in fs::read_dir(from)?.flatten() {
            let target = to.join(e.file_name());
            if !target.exists() {
                fs::rename(e.path(), target)?;
            }
        }
        let _ = fs::remove_dir(from);
        return Ok(());
    }
    fs::rename(from, to).map_err(|e| Error::msg(format!("could not move {}: {e}", from.display())))
}

/// Make `key` the profile whose saves and replays the game sees. What is in the game's folders goes to the profile that was active
/// (the very first time: to [`SHARED`]); the new profile's folders come back (or empty ones are left for the game to fill).
/// Returns the profile that was active before, when it changed.
pub fn activate(data: &Path, key: &str) -> Result<Option<String>> {
    let key = clean_key(key);
    let prev = active(data).unwrap_or_else(|| SHARED.to_string());
    if active(data).as_deref() == Some(&key) {
        return Ok(None);
    }
    for kind in [SAVE, REPLAYS] {
        move_dir(&data.join(kind), &root(data).join(&prev).join(kind))?;
        move_dir(&root(data).join(&key).join(kind), &data.join(kind))?;
    }
    set_active(data, &key)?;
    // an empty profile folder left behind is not worth keeping
    let _ = fs::remove_dir(root(data).join(&key));
    Ok(Some(prev))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub key: String,
    pub active: bool,
    pub saves: u32,
    pub replays: u32,
    pub bytes: u64,
    /// Newest file, seconds since 1970 (0 when empty).
    pub modified: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    /// `save` or `replay`.
    pub kind: &'static str,
    pub name: String,
    pub bytes: u64,
    pub modified: u64,
}

fn files_in(dir: &Path, kind: &'static str, ext: &str) -> Vec<FileInfo> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some(FileInfo {
                kind,
                name: e.file_name().to_string_lossy().into_owned(),
                bytes: m.len(),
                modified: m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs()),
            })
        })
        .collect()
}

/// The saves (`.sav`) and replays (`.rep`) of a profile, newest first.
pub fn files(data: &Path, key: &str) -> Vec<FileInfo> {
    let mut all = files_in(&folder(data, key, SAVE), "save", "sav");
    all.extend(files_in(&folder(data, key, REPLAYS), "replay", "rep"));
    all.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    all
}

/// Every profile that has files (plus the active one, even when empty).
pub fn list(data: &Path) -> Vec<Profile> {
    let act = active(data);
    let mut keys: Vec<String> = fs::read_dir(root(data))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    if let Some(a) = &act {
        if !keys.contains(a) {
            keys.push(a.clone());
        }
    }
    if act.is_none() {
        // nothing is managed yet: the game's own folders are the one shared profile
        keys.push(SHARED.to_string());
    }
    let mut out: Vec<Profile> = keys
        .into_iter()
        .map(|key| {
            let fs_ = if act.is_none() && key == SHARED { let mut v = files_in(&data.join(SAVE), "save", "sav"); v.extend(files_in(&data.join(REPLAYS), "replay", "rep")); v } else { files(data, &key) };
            Profile {
                active: act.as_deref() == Some(key.as_str()),
                saves: fs_.iter().filter(|f| f.kind == "save").count() as u32,
                replays: fs_.iter().filter(|f| f.kind == "replay").count() as u32,
                bytes: fs_.iter().map(|f| f.bytes).sum(),
                modified: fs_.iter().map(|f| f.modified).max().unwrap_or(0),
                key,
            }
        })
        .filter(|p| p.active || p.saves + p.replays > 0)
        .collect();
    out.sort_by(|a, b| b.active.cmp(&a.active).then(b.modified.cmp(&a.modified)).then(a.key.cmp(&b.key)));
    out
}

fn plain(name: &str) -> Result<()> {
    let mut c = Path::new(name).components();
    if matches!((c.next(), c.next()), (Some(std::path::Component::Normal(_)), None)) { Ok(()) } else { Err(Error::msg("not a file name")) }
}

fn kind_dir(kind: &str) -> Result<&'static str> {
    match kind {
        "save" => Ok(SAVE),
        "replay" => Ok(REPLAYS),
        _ => Err(Error::msg("not a kind of file")),
    }
}

fn profile_folder(data: &Path, key: &str, kind: &str) -> Result<PathBuf> {
    let dir = kind_dir(kind)?;
    let key = clean_key(key);
    // while nothing is managed, the shared profile is the game's own folder
    if active(data).is_none() && key == SHARED { Ok(data.join(dir)) } else { Ok(folder(data, &key, dir)) }
}

/// Move one save or replay from one profile to another (it is not overwritten when the other has a file of that name).
pub fn move_file(data: &Path, from: &str, kind: &str, name: &str, to: &str) -> Result<()> {
    plain(name)?;
    let src = profile_folder(data, from, kind)?.join(name);
    let dst_dir = profile_folder(data, to, kind)?;
    if !src.is_file() {
        return Err(Error::msg("that file is not there"));
    }
    fs::create_dir_all(&dst_dir)?;
    let dst = dst_dir.join(name);
    if dst.exists() {
        return Err(Error::msg(format!("{name} already exists there")));
    }
    fs::rename(&src, &dst)?;
    Ok(())
}

/// Delete one save or replay.
pub fn delete_file(data: &Path, key: &str, kind: &str, name: &str) -> Result<()> {
    plain(name)?;
    let path = profile_folder(data, key, kind)?.join(name);
    if !path.is_file() {
        return Err(Error::msg("that file is not there"));
    }
    fs::remove_file(path)?;
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct MergeReport {
    pub moved: u32,
    /// Files that met a file of the same name and were kept under another name.
    pub renamed: u32,
}

/// A name for `name` that is free in `dir`: `name`, else `stem (2).ext`, `stem (3).ext`...
fn free_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2..).map(|n| format!("{stem} ({n}){ext}")).find(|c| !dir.join(c).exists()).unwrap_or_else(|| name.to_string())
}

/// The mod a set belongs to: the key without its version ("contra@009 Beta 3" -> "contra").
pub fn mod_of(key: &str) -> &str {
    key.split('@').next().unwrap_or(key)
}

/// Bring the other versions of the same mod together into `into`, which becomes the set in the game's folders (merging into the
/// unsorted pile takes every set). A save of one mod does not work in another, so other mods' sets are left alone. Nothing is
/// deleted; a file whose name is already taken keeps both (the incoming one gets ` (2)`). The emptied set folders are removed.
pub fn merge_into(data: &Path, into: &str) -> Result<MergeReport> {
    let into = clean_key(into);
    activate(data, &into)?; // the game's folders are now `into`'s
    let mut report = MergeReport::default();
    let others: Vec<PathBuf> = fs::read_dir(root(data))?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| into == SHARED || mod_of(&p.file_name().unwrap_or_default().to_string_lossy()) == mod_of(&into))
        .collect();
    for set in others {
        for kind in [SAVE, REPLAYS] {
            let from = set.join(kind);
            if !from.is_dir() {
                continue;
            }
            let to = data.join(kind);
            fs::create_dir_all(&to)?;
            for e in fs::read_dir(&from)?.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let target = free_name(&to, &name);
                fs::rename(e.path(), to.join(&target))?;
                report.moved += 1;
                if target != name {
                    report.renamed += 1;
                }
            }
            let _ = fs::remove_dir(&from);
        }
        let _ = fs::remove_dir(&set);
    }
    Ok(report)
}

/// The folder to show the user for a profile.
pub fn open_path(data: &Path, key: &str) -> PathBuf {
    let key = clean_key(key);
    if active(data).as_deref() == Some(key.as_str()) || (active(data).is_none() && key == SHARED) { data.to_path_buf() } else { root(data).join(key) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, data: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }

    #[test]
    fn switching_profiles_moves_saves_and_replays_aside_and_back() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path();
        put(data, "Save/old.sav", "before");
        put(data, "Replays/old.rep", "before");
        put(data, "Options.ini", "x=1");

        // first start of a managed mod: what was there is kept as the shared profile
        assert_eq!(activate(data, "contra@009").unwrap().as_deref(), Some(SHARED));
        assert!(!data.join("Save/old.sav").exists());
        assert!(root(data).join(SHARED).join("Save/old.sav").is_file());
        assert_eq!(active(data).as_deref(), Some("contra@009"));

        // the game makes a save for Contra
        put(data, "Save/c1.sav", "contra");
        put(data, "Replays/c1.rep", "contra");
        assert_eq!(activate(data, "rotr@1.87").unwrap().as_deref(), Some("contra@009"));
        assert!(!data.join("Save/c1.sav").exists());
        assert!(root(data).join("contra@009/Save/c1.sav").is_file());

        // back to Contra: its files return
        activate(data, "contra@009").unwrap();
        assert_eq!(fs::read_to_string(data.join("Save/c1.sav")).unwrap(), "contra");
        assert!(data.join("Replays/c1.rep").is_file());
        // starting the same profile again changes nothing
        assert_eq!(activate(data, "contra@009").unwrap(), None);
        // Options.ini and everything else were never touched
        assert_eq!(fs::read_to_string(data.join("Options.ini")).unwrap(), "x=1");
    }

    #[test]
    fn profiles_and_their_files_are_listed_and_can_be_moved_or_deleted() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path();
        put(data, "Save/a.sav", "aaa");
        put(data, "Replays/r.rep", "r");
        // nothing managed yet: one shared profile made of the game's own folders
        let l = list(data);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].key.as_str(), l[0].saves, l[0].replays, l[0].active), (SHARED, 1, 1, false));

        activate(data, "contra@009").unwrap();
        put(data, "Save/c.sav", "c");
        let l = list(data);
        assert_eq!(l.iter().map(|p| (p.key.as_str(), p.active, p.saves)).collect::<Vec<_>>(), [("contra@009", true, 1), (SHARED, false, 1)]);

        // move a save from the shared profile to Contra's (the active one: it lands in the game's folder)
        move_file(data, SHARED, "save", "a.sav", "contra@009").unwrap();
        assert!(data.join("Save/a.sav").is_file());
        assert!(move_file(data, SHARED, "save", "a.sav", "contra@009").is_err(), "it is not there any more");
        assert_eq!(files(data, "contra@009").len(), 2);
        delete_file(data, "contra@009", "save", "c.sav").unwrap();
        assert_eq!(files(data, "contra@009").len(), 1);
        assert!(delete_file(data, "contra@009", "save", "../x").is_err());
        assert!(move_file(data, SHARED, "other", "a", "b").is_err());
    }

    #[test]
    fn merging_brings_the_versions_of_one_mod_together_and_keeps_both_files_of_the_same_name() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path();
        put(data, "Save/a.sav", "shared a");
        put(data, "Replays/Last Replay.rep", "shared replay");
        activate(data, "contra@009").unwrap();
        put(data, "Save/c.sav", "contra");
        put(data, "Replays/Last Replay.rep", "contra replay");
        activate(data, "contra@010").unwrap();
        put(data, "Save/c2.sav", "contra 10");
        put(data, "Replays/Last Replay.rep", "contra 10 replay");
        activate(data, "rotr@1.87").unwrap();
        put(data, "Save/r.sav", "rotr");

        let r = merge_into(data, "contra@009").unwrap();
        assert_eq!(active(data).as_deref(), Some("contra@009"));
        // both versions of the mod are in the game's folders; the two replays with the same name are both there
        let names: Vec<String> = files(data, "contra@009").into_iter().map(|f| f.name).collect();
        for want in ["c.sav", "c2.sav", "Last Replay.rep"] {
            assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
        }
        assert_eq!(names.iter().filter(|n| n.starts_with("Last Replay")).count(), 2);
        assert!(!names.iter().any(|n| n == "r.sav" || n == "a.sav"), "other mods stay out: {names:?}");
        assert!(r.moved == 2 && r.renamed == 1, "{r:?}");
        // the other mod and the unsorted pile keep their own sets
        let left: Vec<String> = list(data).into_iter().map(|p| p.key).collect();
        assert!(left.contains(&"rotr@1.87".to_string()) && left.contains(&SHARED.to_string()) && !left.contains(&"contra@010".to_string()), "{left:?}");

        // merging into the unsorted pile takes everything
        merge_into(data, SHARED).unwrap();
        assert_eq!(list(data).len(), 1);
    }

    #[test]
    fn profile_names_are_safe_folder_names() {
        assert_eq!(clean_key("contra@009 Beta: 3"), "contra@009 Beta_ 3");
        assert_eq!(clean_key(".active"), "active");
        assert_eq!(clean_key(".."), "_");
    }
}
