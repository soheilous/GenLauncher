//! "Game health": looks for the things that most often keep Generals / Zero Hour (especially the Steam version on Windows 10/11)
//! from starting or make mods look wrong, and offers fixes that can be undone.
//!
//! The checks only read. A fix changes one thing, never needs administrator rights, and has a matching "undo". The answer
//! carries ids and values, not sentences: the interface words them in the chosen language.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::game::{self, Game, GameInstall};
use crate::layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// All good.
    Ok,
    /// Worth knowing.
    Info,
    /// Likely to cause trouble.
    Warn,
    /// The game will not work like this.
    Bad,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: &'static str,
    pub level: Level,
    /// Values for the sentence (file names, counts).
    pub args: BTreeMap<&'static str, String>,
    /// The fix that can be offered, if any (see [`fix`]).
    pub fix: Option<&'static str>,
}

fn finding(id: &'static str, level: Level) -> Finding {
    Finding { id, level, args: BTreeMap::new(), fix: None }
}

const DBGHELP: &str = "dbghelp.dll";
const DBGHELP_OFF: &str = "dbghelp.dll.disabled";

fn parked_dir(game_dir: &Path) -> PathBuf {
    layout::root(game_dir).join("parked-archives")
}

/// GenPatcher and GenTool name their add-ons with a three digit order number (`340_ControlBarProZH.big`, `310_ExpandedLANLobbyMenu.big`).
/// They belong to the usual setup and are never reported.
fn is_community_patch_archive(lower_name: &str) -> bool {
    let b = lower_name.as_bytes();
    b.len() > 4 && b[..3].iter().all(u8::is_ascii_digit) && b[3] == b'_'
}

/// `.big` archives in the game folder that do not ship with the game (what a mod leaves behind, or a texture pack added by
/// hand). They are what makes textures pink or a mod behave oddly when no mod is meant to be active.
pub fn foreign_archives(install: &GameInstall) -> Vec<String> {
    let stock = install.game.stock_archives();
    let mut out: Vec<String> = fs::read_dir(&install.dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let l = n.to_lowercase();
            l.ends_with(".big") && !stock.contains(&l.as_str()) && !is_community_patch_archive(&l)
        })
        .collect();
    out.sort_by_key(|n| n.to_lowercase());
    out
}

/// Stock archives the game needs that are not there (a damaged install or an unfinished download).
pub fn missing_archives(install: &GameInstall) -> Vec<String> {
    // the language archives are optional (a game has one language); these are the ones every install has
    let required: &[&str] = match install.game {
        Game::Zh => &["inizh.big", "w3dzh.big", "textureszh.big", "terrainzh.big", "windowzh.big", "shaderszh.big", "patchzh.big", "mapszh.big"],
        Game::Generals => &["ini.big", "w3d.big", "textures.big", "terrain.big", "window.big", "shaders.big", "maps.big"],
    };
    required
        .iter()
        .filter(|n| game::find_ci(&install.dir, n).is_none() && game::find_ci(&install.dir, &format!("{n}{}", game::REPLACE_SUFFIX)).is_none())
        .map(|n| n.to_string())
        .collect()
}

fn can_write(dir: &Path) -> bool {
    let probe = dir.join(".genlauncher-write-test");
    let ok = fs::write(&probe, b"x").is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

/// The game's `InstallPath` in the registry (where a Windows 10/11 compatibility fix such as GenPatcher's writes it).
#[cfg(windows)]
fn registry_install_path(game: Game) -> Option<String> {
    use std::os::windows::process::CommandExt;
    let key = match game {
        Game::Zh => r"HKLM\SOFTWARE\WOW6432Node\EA Games\Command and Conquer Generals Zero Hour",
        Game::Generals => r"HKLM\SOFTWARE\WOW6432Node\Electronic Arts\EA Games\Generals",
    };
    let out = std::process::Command::new("reg").args(["query", key, "/v", "InstallPath"]).creation_flags(0x0800_0000).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().find(|l| l.trim_start().to_lowercase().starts_with("installpath")).and_then(|l| l.split("REG_SZ").nth(1)).map(|v| v.trim().to_string())
}

#[cfg(not(windows))]
fn registry_install_path(_game: Game) -> Option<String> {
    None
}

fn same_dir(a: &str, b: &Path) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase();
    norm(a) == norm(&b.to_string_lossy())
}

/// Everything that is checked. `options_ini`: where the game keeps its settings (it only exists once the game was started).
pub fn check(install: &GameInstall, options_ini: Option<&Path>, session_unfinished: bool) -> Vec<Finding> {
    let mut out = Vec::new();
    let dir = &install.dir;

    let missing = missing_archives(install);
    if !missing.is_empty() {
        let mut f = finding("archives-missing", Level::Bad);
        f.args.insert("count", missing.len().to_string());
        f.args.insert("files", missing.join(", "));
        out.push(f);
    }

    if !can_write(dir) {
        out.push(finding("folder-read-only", Level::Bad));
    }

    if game::find_ci(dir, DBGHELP).is_some() {
        let mut f = finding("dbghelp", Level::Warn);
        f.fix = Some("disable-dbghelp");
        out.push(f);
    } else if game::find_ci(dir, DBGHELP_OFF).is_some() {
        let mut f = finding("dbghelp-disabled", Level::Info);
        f.fix = Some("restore-dbghelp");
        out.push(f);
    }

    let foreign = foreign_archives(install);
    if !foreign.is_empty() && !session_unfinished {
        let mut f = finding("foreign-archives", Level::Info); // often deliberate (map packs, add-ons): shown, never raised as a problem
        f.args.insert("count", foreign.len().to_string());
        f.args.insert("files", foreign.iter().take(6).cloned().collect::<Vec<_>>().join(", "));
        f.fix = Some("park-foreign");
        out.push(f);
    }
    if parked_dir(dir).is_dir() && fs::read_dir(parked_dir(dir)).is_ok_and(|mut d| d.next().is_some()) {
        let mut f = finding("foreign-parked", Level::Info);
        f.fix = Some("restore-foreign");
        out.push(f);
    }

    if session_unfinished {
        out.push(finding("session-unfinished", Level::Info));
    }

    if options_ini.is_some_and(|p| !p.exists()) {
        out.push(finding("never-started", Level::Info));
    }

    // Windows 10/11: the registry entry other tools (and some mods) read
    #[cfg(windows)]
    match registry_install_path(install.game) {
        None => out.push(finding("registry-missing", Level::Info)),
        Some(p) if !same_dir(&p, dir) => {
            let mut f = finding("registry-other-folder", Level::Info);
            f.args.insert("path", p);
            out.push(f);
        }
        Some(_) => {}
    }

    if game::find_ci(dir, "d3d8.dll").is_some() {
        out.push(finding("gentool", Level::Info));
    }

    if out.iter().all(|f| f.level == Level::Info) && out.iter().all(|f| f.fix.is_none()) {
        // nothing to act on: say so explicitly (the interface shows a tick)
        out.insert(0, finding("all-good", Level::Ok));
    }
    out
}

/// The `.big` files that belong to no mod are moved aside (not deleted), remembered, and can be moved back.
fn park_foreign(install: &GameInstall) -> Result<String> {
    let files = foreign_archives(install);
    if files.is_empty() {
        return Ok("nothing to move".into());
    }
    let to = parked_dir(&install.dir);
    fs::create_dir_all(&to)?;
    for name in &files {
        let target = to.join(name);
        if target.exists() {
            fs::remove_file(&target)?;
        }
        fs::rename(install.dir.join(name), target)?;
    }
    Ok(format!("{} files moved", files.len()))
}

fn restore_foreign(install: &GameInstall) -> Result<String> {
    let from = parked_dir(&install.dir);
    let mut n = 0;
    for e in fs::read_dir(&from)?.flatten() {
        let name = e.file_name();
        let back = install.dir.join(&name);
        if back.exists() {
            continue; // a file with this name is there again: keep it, leave the parked copy
        }
        fs::rename(e.path(), back)?;
        n += 1;
    }
    let _ = fs::remove_dir(&from);
    Ok(format!("{n} files moved back"))
}

/// Do one fix by id. Every fix is reversible.
pub fn fix(install: &GameInstall, id: &str) -> Result<String> {
    let dir = &install.dir;
    match id {
        "disable-dbghelp" => {
            let from = game::find_ci(dir, DBGHELP).ok_or_else(|| Error::msg("dbghelp.dll is not there"))?;
            fs::rename(&from, dir.join(DBGHELP_OFF))?;
            Ok("dbghelp.dll renamed".into())
        }
        "restore-dbghelp" => {
            let from = game::find_ci(dir, DBGHELP_OFF).ok_or_else(|| Error::msg("nothing to restore"))?;
            if game::find_ci(dir, DBGHELP).is_some() {
                return Err(Error::msg("There is already a dbghelp.dll"));
            }
            fs::rename(&from, dir.join(DBGHELP))?;
            Ok("dbghelp.dll restored".into())
        }
        "park-foreign" => park_foreign(install),
        "restore-foreign" => restore_foreign(install),
        _ => Err(Error::msg("unknown fix")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install(dir: &Path) -> GameInstall {
        GameInstall { dir: dir.to_path_buf(), game: Game::Zh }
    }

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"x").unwrap();
    }

    fn stock_zh(dir: &Path) {
        for n in ["INIZH.big", "W3DZH.big", "TexturesZH.big", "TerrainZH.big", "WindowZH.big", "ShadersZH.big", "PatchZH.big", "MapsZH.big"] {
            touch(dir, n);
        }
    }

    fn ids(fs: &[Finding]) -> Vec<&'static str> {
        fs.iter().map(|f| f.id).collect()
    }

    #[test]
    fn a_clean_install_has_no_problems() {
        let d = tempfile::tempdir().unwrap();
        stock_zh(d.path());
        let r = check(&install(d.path()), None, false);
        assert!(r.iter().all(|f| f.level <= Level::Info), "{:?}", ids(&r));
        assert!(!ids(&r).contains(&"archives-missing") && !ids(&r).contains(&"dbghelp"));
    }

    #[test]
    fn missing_archives_are_reported_unless_parked_by_a_session() {
        let d = tempfile::tempdir().unwrap();
        stock_zh(d.path());
        fs::remove_file(d.path().join("INIZH.big")).unwrap();
        let r = check(&install(d.path()), None, false);
        let f = r.iter().find(|f| f.id == "archives-missing").unwrap();
        assert_eq!(f.level, Level::Bad);
        assert_eq!(f.args["files"], "inizh.big");
        // parked as INIZH.big.GLR during a launch: not missing
        touch(d.path(), "INIZH.big.GLR");
        assert!(!ids(&check(&install(d.path()), None, true)).contains(&"archives-missing"));
    }

    #[test]
    fn dbghelp_can_be_disabled_and_restored() {
        let d = tempfile::tempdir().unwrap();
        stock_zh(d.path());
        touch(d.path(), "dbghelp.dll");
        let i = install(d.path());
        let f = check(&i, None, false).into_iter().find(|f| f.id == "dbghelp").unwrap();
        assert_eq!(f.fix, Some("disable-dbghelp"));
        fix(&i, "disable-dbghelp").unwrap();
        assert!(!d.path().join("dbghelp.dll").exists() && d.path().join("dbghelp.dll.disabled").exists());
        assert!(ids(&check(&i, None, false)).contains(&"dbghelp-disabled"));
        fix(&i, "restore-dbghelp").unwrap();
        assert!(d.path().join("dbghelp.dll").exists());
    }

    #[test]
    fn foreign_archives_are_moved_aside_and_can_come_back() {
        let d = tempfile::tempdir().unwrap();
        stock_zh(d.path());
        touch(d.path(), "ModTextures.big");
        let i = install(d.path());
        assert_eq!(foreign_archives(&i), ["ModTextures.big"]);
        assert!(ids(&check(&i, None, false)).contains(&"foreign-archives"));
        assert!(ids(&check(&i, None, true)).iter().all(|id| *id != "foreign-archives"), "not while a session is unfinished");
        fix(&i, "park-foreign").unwrap();
        assert!(!d.path().join("ModTextures.big").exists());
        assert!(ids(&check(&i, None, false)).contains(&"foreign-parked"));
        fix(&i, "restore-foreign").unwrap();
        assert!(d.path().join("ModTextures.big").exists());
        assert!(!parked_dir(d.path()).exists());
    }

    #[test]
    fn genpatcher_archives_are_not_foreign() {
        let d = tempfile::tempdir().unwrap();
        stock_zh(d.path());
        touch(d.path(), "340_ControlBarProZH.big");
        touch(d.path(), "310_ExpandedLANLobbyMenu.big");
        touch(d.path(), "AwesomeMod.big");
        assert_eq!(foreign_archives(&install(d.path())), ["AwesomeMod.big"]);
    }

    #[test]
    fn a_game_that_was_never_started_is_noted() {
        let d = tempfile::tempdir().unwrap();
        stock_zh(d.path());
        let missing = d.path().join("nope/Options.ini");
        assert!(ids(&check(&install(d.path()), Some(&missing), false)).contains(&"never-started"));
    }

    #[test]
    fn unknown_fixes_are_refused() {
        let d = tempfile::tempdir().unwrap();
        assert!(fix(&install(d.path()), "format-c").is_err());
    }
}
