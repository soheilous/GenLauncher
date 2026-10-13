//! Crash-safe changes to the game folder.
//!
//! GenLauncher 1.x rewrote the game folder in place and relied on a full-tree scan at the next
//! start to clean up. Here every launch is a list of [`Op`]s that is written to disk **before**
//! anything is touched. Undoing is just replaying that list backwards, and it tolerates any
//! prefix of it having been applied (a crash can stop us after any step) and being run twice.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::store;

pub const JOURNAL_FILE: &str = "launch-journal.json";
/// Suffix of a file moved out of the way for the duration of a launch (same as GenLauncher 1.x).
pub const PARK_SUFFIX: &str = ".GLR";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Op {
    /// Move `from` to `to` (a file that must not be seen by the game this session).
    Park { from: PathBuf, to: PathBuf },
    /// A folder that did not exist and was created for a mod's files.
    MkDir { path: PathBuf },
    /// Make `link` show the same content as `target`.
    Link { link: PathBuf, target: PathBuf },
    /// Overwrite bytes in place (`old` is kept so the file can be restored exactly).
    Patch { file: PathBuf, offset: u64, old: Vec<u8>, new: Vec<u8> },
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Journal {
    started_at: u64,
    ops: Vec<Op>,
}

pub fn journal_path(game_dir: &Path) -> PathBuf {
    store::data_dir(game_dir).join(JOURNAL_FILE)
}

pub fn park_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(PARK_SUFFIX);
    PathBuf::from(s)
}

/// Normalise for comparisons: no `\\?\` prefix, forward slashes, lowercase.
pub(crate) fn norm(p: &Path) -> String {
    p.to_string_lossy().trim_start_matches(r"\\?\").replace('\\', "/").to_lowercase()
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let mut f = File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?; // the plan must be on disk before the first change is made
    drop(f);
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Hard link on Windows (needs no admin rights or Developer Mode, and mods live on the same
/// drive as the game), falling back to a symlink; symlink elsewhere.
pub fn make_link(link: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        fs::hard_link(target, link).or_else(|_| std::os::windows::fs::symlink_file(target, link))
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link)
    }
}

fn is_our_link(link: &Path, target: &Path) -> bool {
    match fs::symlink_metadata(link) {
        Ok(md) if md.file_type().is_symlink() => {
            fs::read_link(link).map(|t| norm(&t) == norm(target)).unwrap_or(false)
        }
        Ok(_) => same_file::is_same_file(link, target).unwrap_or(false), // hard link
        Err(_) => false,
    }
}

fn exists_any(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn apply_op(op: &Op) -> Result<()> {
    match op {
        Op::Park { from, to } => {
            if exists_any(to) {
                return Err(Error::msg(format!("{} already exists (leftover from an earlier session?)", to.display())));
            }
            fs::rename(from, to)?;
        }
        Op::MkDir { path } => {
            if !path.is_dir() {
                fs::create_dir(path)?;
            }
        }
        Op::Link { link, target } => {
            if exists_any(link) {
                return Err(Error::msg(format!("cannot link {}: something is already there", link.display())));
            }
            make_link(link, target).map_err(|e| Error::msg(format!("cannot link {}: {e}", link.display())))?;
        }
        Op::Patch { file, offset, new, .. } => {
            let mut f = OpenOptions::new().write(true).open(file)?;
            f.seek(SeekFrom::Start(*offset))?;
            f.write_all(new)?;
        }
    }
    Ok(())
}

/// Undo one op. Anything unexpected is reported, never "fixed" by deleting user data.
fn undo_op(op: &Op, warnings: &mut Vec<String>) {
    match op {
        Op::Patch { file, offset, old, new } => {
            let Ok(mut f) = OpenOptions::new().read(true).write(true).open(file) else { return };
            let mut cur = vec![0u8; new.len()];
            if f.seek(SeekFrom::Start(*offset)).is_ok() && f.read_exact(&mut cur).is_ok() {
                if &cur == new {
                    let _ = f.seek(SeekFrom::Start(*offset)).and_then(|_| f.write_all(old));
                } else if &cur != old {
                    warnings.push(format!("{} changed while patched; left as is", file.display()));
                }
            }
        }
        Op::Link { link, target } => {
            if !exists_any(link) {
                return;
            }
            if is_our_link(link, target) {
                if let Err(e) = fs::remove_file(link) {
                    warnings.push(format!("could not remove {}: {e}", link.display()));
                }
            } else if !target.exists() {
                warnings.push(format!("{} may still be a leftover link (its target is gone)", link.display()));
            } else {
                warnings.push(format!("{} is not the link we made; left as is", link.display()));
            }
        }
        Op::MkDir { path } => {
            let _ = fs::remove_dir(path); // only succeeds when empty
        }
        Op::Park { from, to } => {
            if exists_any(to) && !exists_any(from) {
                if let Err(e) = fs::rename(to, from) {
                    warnings.push(format!("could not restore {}: {e}", from.display()));
                }
            } else if exists_any(to) {
                warnings.push(format!("{} exists again; kept the parked copy at {}", from.display(), to.display()));
            }
        }
    }
}

pub fn rollback(ops: &[Op]) -> Vec<String> {
    let mut warnings = Vec::new();
    for op in ops.iter().rev() {
        undo_op(op, &mut warnings);
    }
    warnings
}

/// Write the journal, then apply every op. On failure everything done so far is undone.
pub fn apply(game_dir: &Path, ops: Vec<Op>) -> Result<()> {
    let started_at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let journal = Journal { started_at, ops };
    let path = journal_path(game_dir);
    write_synced(&path, &serde_json::to_vec_pretty(&journal)?)?;

    for op in &journal.ops {
        if let Err(e) = apply_op(op) {
            let warnings = rollback(&journal.ops);
            let _ = fs::remove_file(&path);
            for w in warnings {
                log::warn!("rollback: {w}");
            }
            return Err(e);
        }
    }
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Recovery {
    pub ops_undone: usize,
    pub warnings: Vec<String>,
}

/// Undo a session that was never finished (crash, power loss, killed launcher) or end the
/// current one. Returns `None` when there is nothing to undo.
pub fn recover(game_dir: &Path) -> Result<Option<Recovery>> {
    let path = journal_path(game_dir);
    let Ok(text) = fs::read_to_string(&path) else { return Ok(None) };
    let journal: Journal = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            // Keep it for inspection; never guess at what to undo.
            let aside = path.with_extension("json.unreadable");
            let _ = fs::rename(&path, &aside);
            return Err(Error::msg(format!("launch journal unreadable ({e}); kept at {}", aside.display())));
        }
    };
    let warnings = rollback(&journal.ops);
    fs::remove_file(&path)?;
    Ok(Some(Recovery { ops_undone: journal.ops.len(), warnings }))
}
