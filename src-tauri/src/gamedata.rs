//! Backups of what the game keeps for the player: saved games, replays and its settings
//! (`<Documents>/Command and Conquer Generals Zero Hour Data/{Save, Replays, Options.ini, Skirmish.ini}`).
//! Maps are not included (the map manager handles them, and they can be large). Each backup is one zip in the launcher's own
//! folder; a restore first makes a backup of what is there now, so it can be undone.

use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::game::Game;

/// What is saved (relative to the data folder): two folders and two files.
const FOLDERS: &[&str] = &["Save", "Replays"];
const FILES: &[&str] = &["Options.ini", "Skirmish.ini"];
const KEEP: usize = 12;

/// `<Documents>/Command and Conquer Generals Zero Hour Data` (or the Generals one).
pub fn data_dir(documents: &Path, game: Game) -> PathBuf {
    documents.join(match game {
        Game::Zh => "Command and Conquer Generals Zero Hour Data",
        Game::Generals => "Command and Conquer Generals Data",
    })
}

/// Where the backups of a game are kept.
pub fn backups_dir(config_dir: &Path, game: Game) -> PathBuf {
    config_dir.join("game-data-backups").join(game.key())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub name: String,
    /// Seconds since 1970.
    pub at: u64,
    pub bytes: u64,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn name_for(at: u64) -> String {
    format!("backup-{at}.zip")
}

fn parse(name: &str) -> Option<u64> {
    name.strip_prefix("backup-")?.strip_suffix(".zip")?.parse().ok()
}

pub fn list(dir: &Path) -> Vec<Backup> {
    let mut out: Vec<Backup> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            Some(Backup { at: parse(&name)?, bytes: e.metadata().ok()?.len(), name })
        })
        .collect();
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out
}

fn add_file(zip: &mut zip::ZipWriter<File>, path: &Path, name: &str) -> Result<()> {
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(name, options).map_err(|e| Error::msg(format!("zip: {e}")))?;
    io::copy(&mut File::open(path)?, zip)?;
    Ok(())
}

fn add_dir(zip: &mut zip::ZipWriter<File>, root: &Path, rel: &str) -> Result<u32> {
    let mut n = 0;
    for e in fs::read_dir(root)?.flatten() {
        let name = format!("{rel}/{}", e.file_name().to_string_lossy());
        if e.file_type()?.is_dir() {
            n += add_dir(zip, &e.path(), &name)?;
        } else {
            add_file(zip, &e.path(), &name)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Make a backup of the game's data folder. Refuses when there is nothing to back up. The newest [`KEEP`] are kept.
pub fn create(data: &Path, backups: &Path) -> Result<Backup> {
    fs::create_dir_all(backups)?;
    // always newer than the newest one there (two backups in the same second must not get an old name and be pruned at once)
    let at = list(backups).first().map_or(0, |b| b.at + 1).max(now());
    let name = name_for(at);
    let path = backups.join(&name);
    let mut zip = zip::ZipWriter::new(File::create(&path)?);
    let mut files = 0;
    for f in FOLDERS {
        let dir = data.join(f);
        if dir.is_dir() {
            files += add_dir(&mut zip, &dir, f)?;
        }
    }
    for f in FILES {
        let p = data.join(f);
        if p.is_file() {
            add_file(&mut zip, &p, f)?;
            files += 1;
        }
    }
    zip.finish().map_err(|e| Error::msg(format!("zip: {e}")))?;
    if files == 0 {
        let _ = fs::remove_file(&path);
        return Err(Error::msg("There are no saved games, replays or settings to back up yet."));
    }
    for old in list(backups).into_iter().skip(KEEP) {
        let _ = fs::remove_file(backups.join(old.name));
    }
    Ok(Backup { bytes: fs::metadata(&path)?.len(), at, name })
}

fn plain_name(name: &str) -> Result<()> {
    parse(name).map(|_| ()).ok_or_else(|| Error::msg("not a backup name"))
}

/// Put a backup's files back (overwriting files of the same name). What was there before is backed up first.
pub fn restore(data: &Path, backups: &Path, name: &str) -> Result<Backup> {
    plain_name(name)?;
    let path = backups.join(name);
    if !path.is_file() {
        return Err(Error::msg("that backup is not there"));
    }
    let safety = create(data, backups).ok(); // nothing to save is fine: the folder may be empty
    let mut zip = zip::ZipArchive::new(File::open(&path)?).map_err(|e| Error::msg(format!("bad backup: {e}")))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| Error::msg(format!("bad backup entry: {e}")))?;
        let entry_name = entry.name().map_err(|e| Error::msg(e.to_string()))?.to_string();
        if entry.is_dir() {
            continue;
        }
        let target = crate::extract::safe_target(data, &entry_name)?;
        // only what a backup contains: the two folders and two files
        let top = entry_name.split('/').next().unwrap_or_default();
        if !FOLDERS.contains(&top) && !FILES.contains(&top) {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&target)?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        out.write_all(&buf)?;
    }
    Ok(safety.unwrap_or(Backup { name: String::new(), at: 0, bytes: 0 }))
}

pub fn delete(backups: &Path, name: &str) -> Result<()> {
    plain_name(name)?;
    fs::remove_file(backups.join(name))?;
    Ok(())
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
    fn backing_up_and_restoring_round_trips_and_keeps_what_was_there() {
        let d = tempfile::tempdir().unwrap();
        let (data, backups) = (d.path().join("data"), d.path().join("backups"));
        put(&data, "Save/game1.sav", "one");
        put(&data, "Replays/Last Replay.rep", "rep");
        put(&data, "Options.ini", "a=1");
        put(&data, "Maps/Big/Big.map", "not saved");
        let b = create(&data, &backups).unwrap();
        assert!(b.bytes > 0 && list(&backups).len() == 1);

        // the game changes and loses things
        put(&data, "Options.ini", "a=2");
        fs::remove_file(data.join("Save/game1.sav")).unwrap();
        put(&data, "Save/game2.sav", "two");
        restore(&data, &backups, &b.name).unwrap();
        assert_eq!(fs::read_to_string(data.join("Options.ini")).unwrap(), "a=1");
        assert_eq!(fs::read_to_string(data.join("Save/game1.sav")).unwrap(), "one");
        // what was there before the restore is in a backup of its own
        assert_eq!(list(&backups).len(), 2);
        // maps are not part of a backup
        let zip = zip::ZipArchive::new(File::open(backups.join(&b.name)).unwrap()).unwrap();
        assert!(zip.file_names().all(|n| n.is_ok_and(|n| !n.starts_with("Maps"))));
    }

    #[test]
    fn nothing_to_back_up_is_said_and_names_are_checked() {
        let d = tempfile::tempdir().unwrap();
        assert!(create(&d.path().join("empty"), &d.path().join("b")).is_err());
        assert!(list(&d.path().join("b")).is_empty());
        assert!(restore(d.path(), d.path(), "../evil.zip").is_err());
        assert!(delete(d.path(), "x/y.zip").is_err());
    }

    #[test]
    fn only_the_newest_backups_are_kept() {
        let d = tempfile::tempdir().unwrap();
        let (data, backups) = (d.path().join("data"), d.path().join("b"));
        put(&data, "Options.ini", "x");
        for _ in 0..(KEEP + 3) {
            create(&data, &backups).unwrap();
        }
        assert_eq!(list(&backups).len(), KEEP);
    }
}
