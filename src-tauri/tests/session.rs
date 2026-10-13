//! A whole launch session in a throwaway game folder, with a harmless system program standing
//! in for generals.exe. Never touches a real install.

#![cfg(windows)]

use genlauncher2_lib::{game::Game, journal, launch, library, plan, store::Settings};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

fn put(base: &Path, rel: &str, data: &[u8]) {
    let p = base.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, data).unwrap();
}

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

#[test]
fn full_session_runs_a_process_and_restores_everything() {
    let tmp = tempfile::tempdir().unwrap();
    let g = tmp.path();

    // A game folder: stock files + the stand-in executable.
    fs::copy(r"C:\Windows\System32\hostname.exe", g.join("generals.exe")).unwrap();
    put(g, "BINKW32.DLL", b"dll");
    put(g, "WindowZH.big", b"stock");
    put(g, "foreign.big", b"someone else's archive");
    // An installed mod with a patch and an addon.
    put(g, ".genlauncher/mods/Contra/10.0/Art.gib", b"art");
    put(g, ".genlauncher/mods/Contra/10.0/Data/INI/Mod.ini", b"mod");
    put(g, ".genlauncher/mods/Contra/patches/Hotfix/3/Data/INI/Mod.ini", b"patched");
    put(g, ".genlauncher/mods/Contra/addons/Maps/1/Maps/m.map", b"map");
    let before = snapshot(g);

    let settings = Settings {
        active_mod: Some("contra".into()),
        enabled: vec!["contra/patch/hotfix".into(), "contra/addon/maps".into()],
        ..Default::default()
    };
    let disk = library::scan(g);
    let resolved = launch::resolve(g, &settings, &disk, &HashMap::new(), launch::Target::Game).unwrap();
    assert_eq!(resolved.layers.len(), 3);
    assert_eq!(resolved.exe, "generals.exe");

    journal::apply(g, plan::build(g, Game::Zh, &resolved.layers, 0).unwrap()).unwrap();
    assert!(journal::journal_path(g).exists());
    assert_eq!(fs::read(g.join("Data/INI/Mod.ini")).unwrap(), b"patched"); // patch beats mod
    assert_eq!(fs::read(g.join("Art.big")).unwrap(), b"art");
    assert_eq!(fs::read(g.join("Maps/m.map")).unwrap(), b"map");
    assert!(!g.join("foreign.big").exists());

    // The game "runs" (hostname.exe ignores the -win/-quickstart style flags and exits).
    let mut child = launch::spawn(g, &resolved.exe, &launch::game_args(&settings)).unwrap();
    let _ = child.wait().unwrap();

    let rec = journal::recover(g).unwrap().expect("session was journaled");
    assert!(rec.warnings.is_empty(), "{:?}", rec.warnings);
    assert_eq!(snapshot(g), before, "game folder must be byte-identical after the session");
    assert!(!journal::journal_path(g).exists());
}

#[test]
fn a_dead_launcher_is_cleaned_up_by_the_next_one() {
    let tmp = tempfile::tempdir().unwrap();
    let g = tmp.path();
    put(g, "generals.exe", b"x");
    put(g, "foreign.big", b"f");
    put(g, ".genlauncher/mods/M/1/A.gib", b"a");
    let before = snapshot(g);
    let settings = Settings { active_mod: Some("m".into()), ..Default::default() };
    let resolved = launch::resolve(g, &settings, &library::scan(g), &HashMap::new(), launch::Target::Game).unwrap();
    journal::apply(g, plan::build(g, Game::Zh, &resolved.layers, 0).unwrap()).unwrap();
    // process "dies" here: no recover call; a fresh start finds the journal
    assert_ne!(snapshot(g), before);
    assert!(journal::recover(g).unwrap().is_some());
    assert_eq!(snapshot(g), before);
}
