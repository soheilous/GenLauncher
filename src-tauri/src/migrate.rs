//! Moving a game folder from GenLauncher 1.x's layout to this launcher's, and cleaning up what
//! the old launcher left behind.
//!
//! Rules that keep this safe:
//! * Installed mods are **moved** (a rename on the same drive), never deleted and re-downloaded.
//! * All moves happen before any deletion; if a move fails nothing has been removed.
//! * Only things positively identified as the old launcher's are deleted: the old exe (it must
//!   contain the old program's internal name), its 7-zip DLL folders, its settings and image
//!   cache, its empty Vulkan folder and 1.x's partial downloads. Anything else is left and
//!   reported.
//! * [`plan`] computes everything without touching disk, so it can be shown before [`execute`].

use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::game::find_ci;
use crate::layout::{self, legacy};
use crate::library::contains_file;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "step", rename_all = "camelCase")]
pub enum Step {
    /// Rename a folder (same drive: instant, no copying).
    Move { from: PathBuf, to: PathBuf },
    RemoveFile { path: PathBuf },
    /// Remove a folder and everything in it (only ever a folder this module identified as junk).
    RemoveDir { path: PathBuf },
    /// Remove a folder only if it is empty by now.
    RemoveDirIfEmpty { path: PathBuf },
}

impl Step {
    fn is_deletion(&self) -> bool {
        !matches!(self, Step::Move { .. })
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub game_dir: PathBuf,
    pub steps: Vec<Step>,
    /// Things that look like leftovers but are not safe to delete automatically.
    pub notes: Vec<String>,
    pub mods_to_move: usize,
    pub bytes_to_move: u64,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub steps_done: usize,
    pub error: Option<String>,
}

fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = fs::read_dir(p) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

fn child_dirs(p: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<_> = fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .collect();
    v.sort();
    v
}

/// Does the file contain the old launcher's internal assembly name? (An exe called GenLauncher.exe
/// could just as well be this launcher or something else; only the old one has this marker.)
fn is_old_launcher_exe(path: &Path) -> bool {
    const MARKER: &[u8] = b"GenLauncherNet";
    let Ok(mut f) = fs::File::open(path) else { return false };
    let mut buf = vec![0u8; 1 << 20];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let Ok(n) = f.read(&mut buf) else { return false };
        if n == 0 {
            return false;
        }
        carry.extend_from_slice(&buf[..n]);
        if carry.windows(MARKER.len()).any(|w| w == MARKER) {
            return true;
        }
        let keep = carry.len().saturating_sub(MARKER.len() - 1);
        carry.drain(..keep);
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// DLL names the old launcher's optional Vulkan feature stored in `Vulkan/`.
const VULKAN_FILES: &[&str] = &["d3d8x.dll", "d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll", "dxvk.conf", "d3d8.dll"];

pub fn plan(game_dir: &Path) -> Plan {
    let mut plan = Plan { game_dir: game_dir.to_path_buf(), ..Default::default() };
    let mods_new = layout::mods(game_dir);
    let state_new = layout::state(game_dir);

    // ---- 1. installed mods: GLM -> .genlauncher/mods -------------------------------------
    let mut moved: Vec<(PathBuf /*old*/, PathBuf /*new location*/)> = Vec::new();
    for legacy_name in [legacy::MODS, legacy::MODS_OLD] {
        let Some(old) = find_ci(game_dir, legacy_name).filter(|p| p.is_dir()) else { continue };
        if !mods_new.exists() {
            plan.steps.push(Step::Move { from: old.clone(), to: mods_new.clone() });
            moved.extend(child_dirs(&old).into_iter().map(|(n, p)| (p, mods_new.join(n))));
        } else {
            // a library already exists: merge folder by folder, never overwrite
            for (name, p) in child_dirs(&old) {
                let target = mods_new.join(&name);
                if target.exists() {
                    plan.notes.push(format!("{} exists in both the old and the new library; the old copy was left in {}", name, old.display()));
                } else {
                    plan.steps.push(Step::Move { from: p.clone(), to: target.clone() });
                    moved.push((p, target));
                }
            }
            plan.steps.push(Step::RemoveDirIfEmpty { path: old });
        }
    }

    let mut staging = Vec::new();
    let mut renames = Vec::new();
    let mut empties = Vec::new();
    for (old_child, new_child) in &moved {
        plan.mods_to_move += 1;
        plan.bytes_to_move += dir_size(old_child);
        if !contains_file(old_child) {
            empties.push(Step::RemoveDir { path: new_child.clone() });
            continue;
        }
        // partial downloads of 1.x (`<version>.GLTC`), at most mod/Patches/Name/Version deep
        fn find_staging(old: &Path, new: &Path, depth: u32, out: &mut Vec<Step>) {
            for (name, p) in child_dirs(old) {
                if name.to_uppercase().ends_with(legacy::TEMP_SUFFIX) {
                    out.push(Step::RemoveDir { path: new.join(&name) });
                } else if depth < 3 {
                    find_staging(&p, &new.join(&name), depth + 1, out);
                }
            }
        }
        find_staging(old_child, new_child, 0, &mut staging);

        for (sub, _) in child_dirs(old_child) {
            let target = match sub.to_lowercase().as_str() {
                "patches" => layout::PATCHES,
                "addons" => layout::ADDONS,
                "executables" => layout::EXES,
                _ => continue,
            };
            if sub != target {
                renames.push(Step::Move { from: new_child.join(&sub), to: new_child.join(target) });
            }
        }
    }
    // staging folders go first: they sit inside folders that the renames below will move
    plan.steps.extend(staging);
    plan.steps.extend(renames);
    plan.steps.extend(empties);

    // ---- 2. launcher state: .GenLauncherFolder ------------------------------------------
    if let Some(data) = find_ci(game_dir, legacy::DATA).filter(|p| p.is_dir()) {
        for e in fs::read_dir(&data).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(false);
            let path = e.path();
            if is_file && (lower == "settings.json" || lower == "launch-journal.json" || (lower.starts_with("catalog-") && lower.ends_with(".json"))) {
                // written by this launcher while it still used the old folder name
                if !state_new.join(&name).exists() {
                    plan.steps.push(Step::Move { from: path, to: state_new.join(&name) });
                }
            } else if is_file && matches!(lower.as_str(), "gen launchercfg.yaml" | "genlaunchercfg.yaml" | "uam.jpg" | "uam.jpgbw" | "eng") {
                plan.steps.push(Step::RemoveFile { path });
            } else if !is_file && lower == "launcherimages" {
                plan.steps.push(Step::RemoveDir { path });
            } else {
                plan.notes.push(format!("{} in {} was not recognised and was left alone", name, legacy::DATA));
            }
        }
        plan.steps.push(Step::RemoveDirIfEmpty { path: data });
    }

    // ---- 3. the old program and its helper files ----------------------------------------
    if let Some(exe) = find_ci(game_dir, "GenLauncher.exe").filter(|p| p.is_file()) {
        let running_one = std::env::current_exe().map(|me| same_file(&me, &exe)).unwrap_or(false);
        if running_one {
            plan.notes.push("GenLauncher.exe is this launcher, so it was kept".into());
        } else if is_old_launcher_exe(&exe) {
            plan.steps.push(Step::RemoveFile { path: exe });
        } else {
            plan.notes.push("GenLauncher.exe is not the old launcher (or could not be read), so it was kept".into());
        }
    }
    for dir in ["x64", "x86"] {
        // the old launcher unpacked its 7-zip library here; skip any folder with something else in it
        let Some(p) = find_ci(game_dir, dir).filter(|p| p.is_dir()) else { continue };
        let names: Vec<String> = fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_lowercase()).collect();
        if names.iter().all(|n| n == "7z.dll") && !names.is_empty() {
            plan.steps.push(Step::RemoveFile { path: p.join("7z.dll") });
            plan.steps.push(Step::RemoveDirIfEmpty { path: p });
        } else if !names.is_empty() {
            plan.notes.push(format!("{dir}/ contains files other than 7z.dll and was left alone"));
        }
    }
    if let Some(p) = find_ci(game_dir, "Vulkan").filter(|p| p.is_dir()) {
        let files: Vec<(String, PathBuf)> = fs::read_dir(&p)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| (e.file_name().to_string_lossy().to_lowercase(), e.path()))
            .collect();
        if files.iter().all(|(n, path)| path.is_file() && VULKAN_FILES.contains(&n.as_str())) {
            plan.steps.extend(files.into_iter().map(|(_, path)| Step::RemoveFile { path }));
            plan.steps.push(Step::RemoveDirIfEmpty { path: p });
        } else {
            plan.notes.push("Vulkan/ contains files the old launcher does not create and was left alone".into());
        }
    }

    // ---- 4. things we must not guess about ------------------------------------------------
    for name in ["GlBg.png", "Colors.yaml"] {
        if find_ci(game_dir, name).is_some() {
            plan.notes.push(format!("{name} is a custom theme file for the old launcher; delete it if you no longer need it"));
        }
    }
    let parked = count_parked(game_dir, 0);
    if parked > 0 {
        plan.notes.push(format!("{parked} files named *.GLR/*.GOF were left by an unfinished old-launcher session; they are your original game files and were not touched"));
    }
    plan
}

/// Count `*.GLR` / `*.GOF` files (outside the library folders).
fn count_parked(dir: &Path, depth: u32) -> usize {
    if depth > 6 {
        return 0;
    }
    let mut n = 0;
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let Ok(t) = e.file_type() else { continue };
        let name = e.file_name().to_string_lossy().to_uppercase();
        if t.is_dir() {
            if matches!(name.as_str(), "GLM" | ".GENLAUNCHER" | "GENLAUNCHERMODIFICATIONS") {
                continue;
            }
            n += count_parked(&e.path(), depth + 1);
        } else if name.ends_with(".GLR") || name.ends_with(".GOF") {
            n += 1;
        }
    }
    n
}

pub fn execute(plan: &Plan) -> Report {
    let game = &plan.game_dir;
    let mut report = Report::default();
    // deletions only after every move has worked
    let ordered = plan.steps.iter().filter(|s| !s.is_deletion()).chain(plan.steps.iter().filter(|s| s.is_deletion()));
    for step in ordered {
        if let Err(e) = run_step(game, step) {
            report.error = Some(format!("{step:?}: {e}"));
            return report;
        }
        report.steps_done += 1;
    }
    report
}

fn inside(game: &Path, p: &Path) -> bool {
    p.starts_with(game) && p != game
}

fn run_step(game: &Path, step: &Step) -> Result<()> {
    let guard = |p: &Path| if inside(game, p) { Ok(()) } else { Err(Error::msg(format!("refusing to touch {}", p.display()))) };
    match step {
        Step::Move { from, to } => {
            guard(from)?;
            guard(to)?;
            if !fs::symlink_metadata(from).is_ok() {
                return Ok(()); // already moved by an earlier (interrupted) run
            }
            // the same-name-different-case rename (Patches -> patches) must not be refused as "exists"
            let case_only = from.parent() == to.parent()
                && from.file_name().map(|n| n.to_string_lossy().to_lowercase()) == to.file_name().map(|n| n.to_string_lossy().to_lowercase());
            if to.exists() && !case_only {
                return Err(Error::msg(format!("{} already exists", to.display())));
            }
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(from, to)?;
        }
        Step::RemoveFile { path } => {
            guard(path)?;
            if fs::symlink_metadata(path).is_ok() {
                fs::remove_file(path)?;
            }
        }
        Step::RemoveDir { path } => {
            guard(path)?;
            if path.is_dir() {
                fs::remove_dir_all(path)?;
            }
        }
        Step::RemoveDirIfEmpty { path } => {
            guard(path)?;
            if path.is_dir() && fs::read_dir(path)?.next().is_none() {
                fs::remove_dir(path)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn put(base: &Path, rel: &str, data: &[u8]) {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }

    /// All files below `dir` -> content (paths with `/`), plus folders marked with a trailing `/`.
    fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        fn go(base: &Path, d: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for e in fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                if p.is_dir() {
                    out.insert(format!("{rel}/"), vec![]);
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

    /// A game folder as GenLauncher 1.x left it (modelled on a real install).
    fn old_install() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let g = d.path();
        put(g, "generals.exe", b"game");
        put(g, "d3d8.dll", b"GenTool, not ours");
        put(g, "launcher/Generals.exe", b"steam stub");
        put(g, "GenLauncher.exe", b"MZ....GenLauncherNet....");
        put(g, "x64/7z.dll", b"7z64");
        put(g, "x86/7z.dll", b"7z86");
        fs::create_dir_all(g.join("Vulkan")).unwrap();
        put(g, "GLM/Contra/10.0/Art.gib", b"contra art");
        put(g, "GLM/Contra/10.0/Data/x.ini", b"x");
        put(g, "GLM/Contra/Patches/Hotfix/3/p.gib", b"patch");
        put(g, "GLM/Contra/Addons/Maps/1/m.map", b"map");
        put(g, "GLM/Contra/Executables/E/2/e.exe", b"exe");
        put(g, "GLM/Contra/11.0.GLTC/partial.7z", b"half a download");
        put(g, "GLM/Hotfix/Addons/NeedsPatch/1/a.gib", b"addon filed under a patch folder");
        fs::create_dir_all(g.join("GLM/Empty Mod/1.0")).unwrap();
        put(g, ".GenLauncherFolder/GenLauncherCfg.yaml", b"old config");
        put(g, ".GenLauncherFolder/uam.jpg", b"img");
        put(g, ".GenLauncherFolder/uam.jpgBW", b"img");
        put(g, ".GenLauncherFolder/LauncherImages/Contra/1", b"cover");
        put(g, ".GenLauncherFolder/settings.json", b"{\"windowed\":false}");
        put(g, ".GenLauncherFolder/catalog-zh.json", b"{}");
        d
    }

    #[test]
    fn migrates_mods_and_removes_only_old_launcher_files() {
        let d = old_install();
        let g = d.path();
        let p = plan(g);
        assert_eq!(p.mods_to_move, 3);
        let r = execute(&p);
        assert_eq!(r.error, None);

        let t = tree(g);
        // mods moved intact, extras folders renamed to the new names
        assert_eq!(t[".genlauncher/mods/Contra/10.0/Art.gib"], b"contra art");
        assert_eq!(t[".genlauncher/mods/Contra/10.0/Data/x.ini"], b"x");
        assert_eq!(t[".genlauncher/mods/Contra/patches/Hotfix/3/p.gib"], b"patch");
        assert_eq!(t[".genlauncher/mods/Contra/addons/Maps/1/m.map"], b"map");
        assert_eq!(t[".genlauncher/mods/Contra/exes/E/2/e.exe"], b"exe");
        assert_eq!(t[".genlauncher/mods/Hotfix/addons/NeedsPatch/1/a.gib"], b"addon filed under a patch folder");
        // settings kept (moved), everything else of ours removed
        assert_eq!(t[".genlauncher/state/settings.json"], b"{\"windowed\":false}");
        assert!(t.contains_key(".genlauncher/state/catalog-zh.json"));
        // junk gone
        for gone in ["GLM/", ".GenLauncherFolder/", "GenLauncher.exe", "x64/", "x86/", "Vulkan/", ".genlauncher/mods/Contra/11.0.GLTC/", ".genlauncher/mods/Empty Mod/"] {
            assert!(!t.contains_key(gone), "{gone} should be gone");
        }
        // not ours: untouched
        assert_eq!(t["d3d8.dll"], b"GenTool, not ours");
        assert_eq!(t["launcher/Generals.exe"], b"steam stub");
        assert_eq!(t["generals.exe"], b"game");
        // and the library reads it
        let lib = crate::library::scan(g);
        let contra = lib.iter().find(|m| m.name == "Contra").unwrap();
        assert_eq!(contra.versions, ["10.0"]);
        assert_eq!(contra.patches[0].name, "Hotfix");
        assert_eq!(contra.addons[0].name, "Maps");
        assert_eq!(contra.exes[0].name, "E");
        assert_eq!(lib.iter().find(|m| m.name == "Hotfix").unwrap().addons[0].name, "NeedsPatch");
    }

    #[test]
    fn is_idempotent_and_a_clean_folder_has_nothing_to_do() {
        let d = old_install();
        execute(&plan(d.path()));
        let after = tree(d.path());
        let again = plan(d.path());
        assert!(again.is_empty(), "{:?}", again.steps);
        assert_eq!(execute(&again).steps_done, 0);
        assert_eq!(tree(d.path()), after);
        let clean = tempfile::tempdir().unwrap();
        put(clean.path(), "generals.exe", b"x");
        assert!(plan(clean.path()).is_empty());
    }

    #[test]
    fn unrecognised_things_are_kept_and_reported() {
        let d = old_install();
        let g = d.path();
        put(g, "GenLauncher.exe", b"some other program"); // no old-launcher marker
        put(g, "x64/other.dll", b"not the 7z library");
        put(g, "Vulkan/mystery.bin", b"?");
        put(g, ".GenLauncherFolder/notes.txt", b"mine");
        put(g, "GlBg.png", b"theme");
        put(g, "Data/INI/Weapon.ini.GLR", b"parked original");
        let p = plan(g);
        let r = execute(&p);
        assert_eq!(r.error, None);
        let t = tree(g);
        for kept in ["GenLauncher.exe", "x64/other.dll", "Vulkan/mystery.bin", ".GenLauncherFolder/notes.txt", "GlBg.png", "Data/INI/Weapon.ini.GLR"] {
            assert!(t.contains_key(kept), "{kept} must be kept");
        }
        assert!(t.contains_key("x64/7z.dll"), "a folder with anything else in it is left completely alone");
        assert!(!t.contains_key("x86/7z.dll"), "while the folder that holds only the old 7z library is cleaned");
        let notes = p.notes.join("\n");
        for needle in ["not the old launcher", "x64/", "Vulkan/", "notes.txt", "GlBg.png", "GLR"] {
            assert!(notes.contains(needle), "missing note about {needle}: {notes}");
        }
    }

    #[test]
    fn existing_new_library_is_merged_without_overwriting() {
        let d = old_install();
        let g = d.path();
        put(g, ".genlauncher/mods/Contra/10.0/Art.gib", b"already in the new library");
        let p = plan(g);
        assert!(p.notes.iter().any(|n| n.contains("Contra") && n.contains("both")));
        assert_eq!(execute(&p).error, None);
        let t = tree(g);
        assert_eq!(t[".genlauncher/mods/Contra/10.0/Art.gib"], b"already in the new library");
        assert!(t.contains_key("GLM/Contra/10.0/Art.gib"), "the old copy stays until the user decides");
        assert!(t.contains_key(".genlauncher/mods/Hotfix/addons/NeedsPatch/1/a.gib"), "other mods still moved");
    }

    #[test]
    fn a_failed_move_deletes_nothing() {
        let d = old_install();
        let g = d.path();
        let mut p = plan(g);
        // sabotage: a move whose source is outside the game folder
        p.steps.insert(0, Step::Move { from: g.parent().unwrap().join("elsewhere"), to: g.join("x") });
        let r = execute(&p);
        assert!(r.error.is_some());
        let t = tree(g);
        assert!(t.contains_key("GenLauncher.exe") && t.contains_key("GLM/Contra/10.0/Art.gib"), "nothing removed after a failure");
    }

    #[test]
    fn never_touches_paths_outside_the_game_folder() {
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        put(outside.path(), "precious.txt", b"keep");
        let g = d.path();
        for step in [
            Step::RemoveFile { path: outside.path().join("precious.txt") },
            Step::RemoveDir { path: outside.path().to_path_buf() },
            Step::RemoveDir { path: g.to_path_buf() },
        ] {
            assert!(run_step(g, &step).is_err(), "{step:?}");
        }
        assert!(outside.path().join("precious.txt").exists());
    }

    #[test]
    fn marker_search_works_across_read_boundaries() {
        let d = tempfile::tempdir().unwrap();
        let mut data = vec![b'a'; (1 << 20) - 5];
        data.extend_from_slice(b"GenLauncherNet"); // straddles the 1 MiB read size
        data.extend_from_slice(&[b'b'; 100]);
        put(d.path(), "x.exe", &data);
        assert!(is_old_launcher_exe(&d.path().join("x.exe")));
        put(d.path(), "y.exe", &vec![b'a'; 3 << 20]);
        assert!(!is_old_launcher_exe(&d.path().join("y.exe")));
    }
}
