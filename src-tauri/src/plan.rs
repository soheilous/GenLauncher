//! Turns "these mod layers on this game folder" into an ordered list of [`Op`]s, without
//! touching anything. Mirrors what GenLauncher 1.x did in `GameLauncher.PrepareGameFiles`:
//!
//! * archives and loose data files that are not part of the game or the chosen layers are
//!   moved aside so the game cannot load them,
//! * every file of every layer is linked into the game folder; later layers win
//!   (mod < patch < addons < executables),
//! * `.gib` files (how archives are stored so the game never sees them) are linked as `.big`,
//! * a 24-byte archive is a placeholder meaning "hide the game's archive of that name",
//! * `.exe` / `.dll` are only linked from executable layers.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::big;
use crate::error::{Error, Result};
use crate::game::Game;
use crate::journal::{Op, norm, park_path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Mod,
    Patch,
    Addon,
    Exe,
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub root: PathBuf,
    pub kind: LayerKind,
}

/// Loose data files that override game content when found outside the mod folders.
const LOOSE_DATA_EXT: &[&str] = &["w3d", "dds", "tga", "ini", "scb", "wnd", "csf", "str"];
/// Top-level folders that belong to the launcher (or Steam's duplicate game folder), never scanned.
const LAUNCHER_DIRS: &[&str] = &[".genlauncher", "glm", ".genlauncherfolder", "vulkan", "zh_generals"];
const PLACEHOLDER_BIG_SIZE: u64 = 24;

fn ext_lower(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn exists_any(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

/// `rel` below `root`, using the on-disk spelling of every component that already exists
/// (Linux filesystems are case-sensitive; the game and its mods are not).
pub fn resolve_ci(root: &Path, rel: &Path) -> PathBuf {
    let mut cur = root.to_path_buf();
    let mut missing = false;
    for comp in rel.components() {
        let name = comp.as_os_str();
        if !missing {
            let direct = cur.join(name);
            if exists_any(&direct) {
                cur = direct;
                continue;
            }
            let wanted = name.to_string_lossy().to_lowercase();
            let found = fs::read_dir(&cur)
                .ok()
                .and_then(|rd| rd.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == wanted));
            match found {
                Some(e) => {
                    cur = e.path();
                    continue;
                }
                None => missing = true,
            }
        }
        cur.push(name);
    }
    cur
}

fn is_temp_dir(name: &str) -> bool {
    name.to_uppercase().ends_with(".GLTC")
}

fn foreign_files(game_dir: &Path, game: Game) -> Vec<PathBuf> {
    fn go(dir: &Path, top: bool, game: Game, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue; // ours or the user's; either way not content to hide
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            if ft.is_dir() {
                if (top && LAUNCHER_DIRS.contains(&lower.as_str())) || is_temp_dir(&name) {
                    continue;
                }
                go(&e.path(), false, game, out);
            } else if ft.is_file() {
                let ext = ext_lower(&e.path());
                let foreign_archive = ext == "big" && !game.stock_archives().contains(&lower.as_str());
                if foreign_archive || LOOSE_DATA_EXT.contains(&ext.as_str()) {
                    out.push(e.path());
                }
            }
        }
    }
    let mut out = Vec::new();
    go(game_dir, true, game, &mut out);
    out
}

fn layer_files(root: &Path) -> Vec<PathBuf> {
    fn go(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            if p.is_dir() {
                if !is_temp_dir(&e.file_name().to_string_lossy()) {
                    go(&p, out);
                }
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    go(root, &mut out);
    out
}

struct Entry {
    rel: PathBuf,
    /// `None`: hide whatever the game has at this path.
    source: Option<PathBuf>,
}

pub fn build(game_dir: &Path, game: Game, layers: &[Layer], camera_height: u32) -> Result<Vec<Op>> {
    let mut parks: Vec<Op> = Vec::new();
    let mut parked: HashSet<String> = HashSet::new();
    fn park(path: PathBuf, parks: &mut Vec<Op>, parked: &mut HashSet<String>) -> Result<()> {
        let to = park_path(&path);
        if exists_any(&to) {
            return Err(Error::msg(format!(
                "{} already exists, probably left by an older launcher session. Restore or remove it first.",
                to.display()
            )));
        }
        parked.insert(norm(&path));
        parks.push(Op::Park { from: path, to });
        Ok(())
    }

    for f in foreign_files(game_dir, game) {
        park(f, &mut parks, &mut parked)?;
    }

    // Final content of the game folder contributed by the layers: later layers override earlier ones.
    let mut map: BTreeMap<String, Entry> = BTreeMap::new();
    for layer in layers {
        for file in layer_files(&layer.root) {
            let Ok(rel) = file.strip_prefix(&layer.root) else { continue };
            let name = file.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            if name.ends_with(".glr") || name.ends_with(".gof") {
                continue;
            }
            let ext = ext_lower(&file);
            if layer.kind != LayerKind::Exe && (ext == "exe" || ext == "dll") {
                continue;
            }
            let (target_rel, source) = if ext == "big" || ext == "gib" {
                let size = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                let rel = rel.with_extension("big");
                (rel, (size != PLACEHOLDER_BIG_SIZE).then(|| file.clone()))
            } else {
                (rel.to_path_buf(), Some(file.clone()))
            };
            let key = norm(&target_rel);
            map.insert(key, Entry { rel: target_rel, source });
        }
    }

    let mut mkdirs: Vec<Op> = Vec::new();
    let mut seen_dirs: HashSet<String> = HashSet::new();
    let mut links: Vec<Op> = Vec::new();

    for entry in map.values() {
        let target = resolve_ci(game_dir, &entry.rel);

        // Folders that do not exist yet (outermost first).
        let mut missing = Vec::new();
        let mut cur = target.parent().map(Path::to_path_buf);
        while let Some(dir) = cur {
            if norm(&dir) == norm(game_dir) || exists_any(&dir) {
                break;
            }
            cur = dir.parent().map(Path::to_path_buf);
            missing.push(dir);
        }
        for dir in missing.into_iter().rev() {
            if seen_dirs.insert(norm(&dir)) {
                mkdirs.push(Op::MkDir { path: dir });
            }
        }

        match fs::symlink_metadata(&target) {
            Ok(md) if md.is_dir() => {
                return Err(Error::msg(format!("a mod file would replace the folder {}", target.display())));
            }
            Ok(_) if !parked.contains(&norm(&target)) => park(target.clone(), &mut parks, &mut parked)?,
            _ => {}
        }
        if let Some(source) = &entry.source {
            links.push(Op::Link { link: target, target: source.clone() });
        }
    }

    let mut ops = parks;
    ops.extend(mkdirs);
    ops.extend(links);

    // Camera height (Zero Hour): the first archive, by file name, that defines GameData.ini.
    if camera_height > 0 && game == Game::Zh {
        let mut archives: Vec<&Entry> = map
            .values()
            .filter(|e| e.source.is_some() && ext_lower(&e.rel) == "big")
            .collect();
        archives.sort_by_key(|e| e.rel.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        for e in archives {
            let src = e.source.as_ref().expect("filtered above");
            if let Ok(Some(spot)) = big::find_camera_height(src) {
                let new = big::fit_height(camera_height, spot.old.len())?;
                if new != spot.old {
                    ops.push(Op::Patch { file: src.clone(), offset: spot.offset, old: spot.old, new });
                }
                break;
            }
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal;
    use std::collections::BTreeMap;

    /// Everything under `dir` (except the launcher's data folder): path -> content, with link
    /// content resolved, so before/after comparisons catch any leftover change.
    fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        fn go(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for e in fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                if rel.starts_with(".genlauncher/state") {
                    continue;
                }
                if p.is_dir() {
                    out.insert(format!("{rel}/"), Vec::new());
                    go(base, &p, out);
                } else {
                    out.insert(rel, fs::read(&p).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        go(dir, dir, &mut out);
        out
    }

    fn put(base: &Path, rel: &str, data: &[u8]) -> PathBuf {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, data).unwrap();
        p
    }

    struct Fixture {
        _tmp: tempfile::TempDir,
        game: PathBuf,
    }

    fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let game = tmp.path().join("game");
        put(&game, "generals.exe", b"exe");
        put(&game, "BINKW32.DLL", b"dll");
        put(&game, "WindowZH.big", b"stock window");
        put(&game, "INIZH.big", b"stock ini");
        put(&game, "310_ExpandedLobby.big", b"foreign archive");
        put(&game, "Data/INI/Loose.ini", b"loose override");
        put(&game, ".genlauncher/mods/Mod/1.0/Data/INI/Mod.ini", b"mod ini"); // lives in GLM, must not be treated as loose
        Fixture { _tmp: tmp, game }
    }

    fn layer(game: &Path, rel: &str, kind: LayerKind) -> Layer {
        Layer { root: game.join(rel), kind }
    }

    #[test]
    fn launch_then_restore_leaves_the_folder_identical() {
        let fx = fixture();
        let g = &fx.game;
        put(g, ".genlauncher/mods/Mod/1.0/Art.gib", b"mod art archive");
        put(g, ".genlauncher/mods/Mod/1.0/INIZH.gib", b"mod ini archive replacing stock");
        put(g, ".genlauncher/mods/Mod/1.0/Data/INI/New.ini", b"new ini");
        put(g, ".genlauncher/mods/Mod/1.0/tool.dll", b"must not be linked");
        let before = snapshot(g);

        let layers = [layer(g, ".genlauncher/mods/Mod/1.0", LayerKind::Mod)];
        let ops = build(g, Game::Zh, &layers, 0).unwrap();
        journal::apply(g, ops).unwrap();

        // While "playing": mod content visible, foreign + replaced stock content hidden.
        assert_eq!(fs::read(g.join("Art.big")).unwrap(), b"mod art archive");
        assert_eq!(fs::read(g.join("INIZH.big")).unwrap(), b"mod ini archive replacing stock");
        assert_eq!(fs::read(g.join("Data/INI/New.ini")).unwrap(), b"new ini");
        assert!(!g.join("tool.dll").exists());
        assert!(!g.join("310_ExpandedLobby.big").exists());
        assert!(!g.join("Data/INI/Loose.ini").exists());
        assert!(g.join("310_ExpandedLobby.big.GLR").exists());
        assert_eq!(fs::read(g.join("WindowZH.big")).unwrap(), b"stock window"); // untouched stock file

        let rec = journal::recover(g).unwrap().unwrap();
        assert!(rec.warnings.is_empty(), "{:?}", rec.warnings);
        assert_eq!(snapshot(g), before);
        assert!(journal::recover(g).unwrap().is_none(), "journal removed");
    }

    #[test]
    fn crash_mid_session_is_undone_on_next_start() {
        let fx = fixture();
        let g = &fx.game;
        put(g, ".genlauncher/mods/Mod/1.0/Art.gib", b"a");
        let before = snapshot(g);
        let ops = build(g, Game::Zh, &[layer(g, ".genlauncher/mods/Mod/1.0", LayerKind::Mod)], 0).unwrap();
        journal::apply(g, ops).unwrap();
        // ...the launcher dies here; nothing calls recover. A new process starts:
        assert_ne!(snapshot(g), before);
        journal::recover(g).unwrap().unwrap();
        assert_eq!(snapshot(g), before);
    }

    #[test]
    fn undo_is_safe_after_a_partial_apply_and_when_repeated() {
        let fx = fixture();
        let g = &fx.game;
        put(g, ".genlauncher/mods/Mod/1.0/Data/INI/New.ini", b"x");
        let before = snapshot(g);
        let ops = build(g, Game::Zh, &[layer(g, ".genlauncher/mods/Mod/1.0", LayerKind::Mod)], 0).unwrap();
        // Apply only the first few ops by hand (simulating a crash in the middle), keep the journal.
        let n = ops.len() / 2;
        assert!(n >= 1);
        journal::apply(g, ops[..n].to_vec()).unwrap();
        // Undo twice: the second must be a harmless no-op.
        journal::recover(g).unwrap();
        journal::rollback(&ops);
        assert_eq!(snapshot(g), before);
    }

    #[test]
    fn later_layers_override_and_placeholder_hides_stock_archive() {
        let fx = fixture();
        let g = &fx.game;
        put(g, ".genlauncher/mods/Mod/1.0/Data/INI/Shared.ini", b"from mod");
        put(g, ".genlauncher/mods/Mod/patches/P/2/Data/INI/Shared.ini", b"from patch");
        put(g, ".genlauncher/mods/Mod/addons/A/1/Data/INI/Shared.ini", b"from addon");
        put(g, ".genlauncher/mods/Mod/1.0/WindowZH.gib", &[0u8; 24]); // placeholder: hide stock WindowZH.big
        let before = snapshot(g);
        let layers = [
            layer(g, ".genlauncher/mods/Mod/1.0", LayerKind::Mod),
            layer(g, ".genlauncher/mods/Mod/patches/P/2", LayerKind::Patch),
            layer(g, ".genlauncher/mods/Mod/addons/A/1", LayerKind::Addon),
        ];
        journal::apply(g, build(g, Game::Zh, &layers, 0).unwrap()).unwrap();
        assert_eq!(fs::read(g.join("Data/INI/Shared.ini")).unwrap(), b"from addon");
        assert!(!g.join("WindowZH.big").exists());
        assert!(g.join("WindowZH.big.GLR").exists());
        journal::recover(g).unwrap();
        assert_eq!(snapshot(g), before);
    }

    #[test]
    fn executable_layers_link_exe_and_dll() {
        let fx = fixture();
        let g = &fx.game;
        put(g, ".genlauncher/mods/Mod/exes/E/1/special.exe", b"exe");
        put(g, ".genlauncher/mods/Mod/exes/E/1/helper.dll", b"dll");
        let before = snapshot(g);
        journal::apply(g, build(g, Game::Zh, &[layer(g, ".genlauncher/mods/Mod/exes/E/1", LayerKind::Exe)], 0).unwrap()).unwrap();
        assert!(g.join("special.exe").exists() && g.join("helper.dll").exists());
        journal::recover(g).unwrap();
        assert_eq!(snapshot(g), before);
    }

    #[test]
    fn stale_backup_stops_the_launch_before_changing_anything() {
        let fx = fixture();
        let g = &fx.game;
        put(g, "310_ExpandedLobby.big.GLR", b"leftover from an older session");
        let before = snapshot(g);
        let err = build(g, Game::Zh, &[], 0).unwrap_err().to_string();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(snapshot(g), before);
    }

    #[test]
    fn failure_while_applying_rolls_everything_back() {
        let fx = fixture();
        let g = &fx.game;
        put(g, ".genlauncher/mods/Mod/1.0/A.gib", b"a");
        put(g, ".genlauncher/mods/Mod/1.0/Z.gib", b"z");
        let before = snapshot(g);
        let mut ops = build(g, Game::Zh, &[layer(g, ".genlauncher/mods/Mod/1.0", LayerKind::Mod)], 0).unwrap();
        // Make the last op impossible (link target folder missing -> link creation fails).
        ops.push(Op::Link { link: g.join("Nope/Deeper/file.big"), target: g.join(".genlauncher/mods/Mod/1.0/Z.gib") });
        let err = journal::apply(g, ops).unwrap_err().to_string();
        assert!(err.contains("cannot link"), "{err}");
        assert_eq!(snapshot(g), before);
        assert!(journal::recover(g).unwrap().is_none());
    }

    #[test]
    fn camera_height_is_patched_in_the_mod_archive_and_restored() {
        let fx = fixture();
        let g = &fx.game;
        let ini = b"GameData\n  MaxCameraHeight = 310.0\nEnd\n";
        let archive = big::build_big(&[("Data\\INI\\GameData.ini", ini)]);
        put(g, ".genlauncher/mods/Mod/1.0/Cam.gib", &archive);
        let before = snapshot(g);
        let ops = build(g, Game::Zh, &[layer(g, ".genlauncher/mods/Mod/1.0", LayerKind::Mod)], 450).unwrap();
        assert!(ops.iter().any(|o| matches!(o, Op::Patch { .. })));
        journal::apply(g, ops).unwrap();
        let patched = fs::read(g.join("Cam.big")).unwrap();
        assert!(String::from_utf8_lossy(&patched).contains("MaxCameraHeight = 450.0"));
        journal::recover(g).unwrap();
        assert_eq!(snapshot(g), before);
        // Generals has no camera patch.
        let ops = build(g, Game::Generals, &[layer(g, ".genlauncher/mods/Mod/1.0", LayerKind::Mod)], 450).unwrap();
        assert!(!ops.iter().any(|o| matches!(o, Op::Patch { .. })));
    }

    #[test]
    fn resolve_ci_prefers_the_existing_spelling() {
        let fx = fixture();
        let p = resolve_ci(&fx.game, Path::new("data/ini/loose.INI"));
        assert!(p.exists());
        assert_eq!(p.file_name().unwrap().to_string_lossy().to_lowercase(), "loose.ini");
        // Missing tail keeps the requested spelling under the existing (re-cased) parents.
        let q = resolve_ci(&fx.game, Path::new("DATA/Maps/New/x.map"));
        let tail = q.strip_prefix(&fx.game).unwrap().to_string_lossy().replace('\\', "/");
        assert_eq!(tail.to_lowercase(), "data/maps/new/x.map");
        if cfg!(not(windows)) {
            // case-sensitive filesystems: existing folders are re-cased, only the new tail keeps our spelling
            assert_eq!(tail, "Data/Maps/New/x.map");
        }
    }
}
