//! From the user's selection to a running game: which layers, which executable, which arguments.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::{Child, Command};

use crate::catalog::{ORIGINAL_GAME_ID, extra_id, slug, Kind};
use crate::error::{Error, Result};
use crate::game::find_ci;
use crate::library::{self, InstalledMod};
use crate::manifest::VersionInfo;
use crate::plan::{Layer, LayerKind};
use crate::store::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Game,
    WorldBuilder,
}

#[derive(Debug)]
pub struct Resolved {
    pub layers: Vec<Layer>,
    /// File name of the executable to start (found in the game folder once layers are linked).
    pub exe: String,
    /// Extra programs a mod wants started alongside the game.
    pub helpers: Vec<String>,
}

/// `exe_infos`: catalog records of the active mod's executables, keyed by item id.
pub fn resolve(
    game_dir: &Path,
    settings: &Settings,
    disk: &[InstalledMod],
    exe_infos: &HashMap<String, VersionInfo>,
    target: Target,
) -> Result<Resolved> {
    let mod_id = settings.active_mod.as_deref().filter(|m| !m.is_empty()).unwrap_or(ORIGINAL_GAME_ID);
    let is_original = mod_id == ORIGINAL_GAME_ID;
    let mods_root = library::mods_root(game_dir);

    let installed = disk.iter().find(|d| slug(&d.name) == mod_id);
    let mut layers = Vec::new();

    if !is_original {
        let m = installed
            .filter(|m| !m.versions.is_empty())
            .ok_or_else(|| Error::msg("The selected mod is not installed"))?;
        let version = settings
            .chosen_versions
            .get(mod_id)
            .and_then(|want| m.versions.iter().find(|v| v.eq_ignore_ascii_case(want)))
            .unwrap_or(&m.versions[0]); // versions are sorted newest first
        layers.push(Layer { root: mods_root.join(&m.name).join(version), kind: LayerKind::Mod });
    }

    let mut exe = match target {
        Target::Game => "generals.exe".to_string(),
        Target::WorldBuilder => "WorldBuilder.exe".to_string(),
    };
    let mut helpers = Vec::new();

    // (the original game's folder "Original Game" slugs to `original-game`, so it is found like any mod)
    // Where an enabled extra may live: the mod's own folder first, then any other folder (1.x filed an
    // addon under the patch it needs). Ids are scoped by `mod_id/`, so other mods' extras never match.
    let mut owners: Vec<&InstalledMod> = installed.into_iter().collect();
    owners.extend(disk.iter().filter(|d| installed.is_none_or(|m| !std::ptr::eq(*d, m))));
    let mut seen: HashSet<String> = HashSet::new();
    let mut replaced = false;
    for (kind, dir, layer_kind) in [
        (Kind::Patch, library::PATCHES_DIR, LayerKind::Patch),
        (Kind::Addon, library::ADDONS_DIR, LayerKind::Addon),
        (Kind::Exe, library::EXES_DIR, LayerKind::Exe),
    ] {
        let mut picked = 0;
        for m in &owners {
            let list = match kind {
                Kind::Patch => &m.patches,
                Kind::Addon => &m.addons,
                _ => &m.exes,
            };
            for e in list {
                let id = extra_id(mod_id, kind, &e.name);
                if !settings.enabled.contains(&id) || !seen.insert(id.clone()) {
                    continue;
                }
                picked += 1;
                let Some(version) = e.versions.first() else { continue };
                layers.push(Layer { root: mods_root.join(&m.name).join(dir).join(&e.name).join(version), kind: layer_kind });

                if kind == Kind::Exe {
                    if let Some(info) = exe_infos.get(&id) {
                        let file = info.executable_file_name.clone().filter(|f| !f.is_empty());
                        let replaces = match target {
                            Target::Game => info.replaces_game_exe,
                            Target::WorldBuilder => info.replaces_wb_exe,
                        };
                        match (replaces, file) {
                            (true, Some(f)) if replaced => {
                                return Err(Error::msg(format!(
                                    "More than one executable replaces the {}; turn one off ({f})",
                                    if target == Target::Game { "game" } else { "World Builder" }
                                )));
                            }
                            (true, Some(f)) => {
                                replaced = true;
                                exe = f;
                            }
                            (false, Some(f)) if target == Target::Game && !info.replaces_wb_exe => helpers.push(f),
                            _ => {}
                        }
                    }
                }
            }
        }
        if kind == Kind::Patch && picked > 1 {
            return Err(Error::msg("Only one patch can be active at a time"));
        }
    }
    Ok(Resolved { layers, exe, helpers })
}

/// Whitespace-separated arguments; double quotes group words (`-mod "C:\My Mods"`).
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(cur);
    }
    out
}

pub fn game_args(settings: &Settings) -> Vec<String> {
    let mut args = Vec::new();
    if settings.windowed {
        args.push("-win".to_string());
    }
    if settings.quick_start {
        args.push("-quickstart".to_string());
        args.push("-noshellmap".to_string());
    }
    args.extend(split_args(&settings.game_params));
    args
}

/// Start `exe` from the game folder. On Linux the game is a Windows program, so it goes through
/// Wine (`GENLAUNCHER_WINE` overrides the `wine` binary; prefix selection comes later).
pub fn spawn(game_dir: &Path, exe: &str, args: &[String]) -> Result<Child> {
    let path = find_ci(game_dir, exe).ok_or_else(|| Error::msg(format!("{exe} was not found in the game folder")))?;
    #[cfg(windows)]
    let mut cmd = Command::new(&path);
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new(std::env::var("GENLAUNCHER_WINE").unwrap_or_else(|_| "wine".into()));
        c.arg(&path);
        c
    };
    cmd.args(args).current_dir(game_dir).spawn().map_err(|e| Error::msg(format!("could not start {exe}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::ORIGINAL_GAME;
    use crate::library::Extra;
    use std::path::PathBuf;

    fn disk() -> Vec<InstalledMod> {
        vec![
            InstalledMod {
                name: "Contra".into(),
                versions: vec!["10.0".into(), "9.0".into()],
                patches: vec![
                    Extra { name: "Hotfix".into(), versions: vec!["3".into(), "2".into()] },
                    Extra { name: "Other".into(), versions: vec!["1".into()] },
                ],
                addons: vec![Extra { name: "Maps".into(), versions: vec!["1.0".into()] }],
                exes: vec![Extra { name: "Big Exe".into(), versions: vec!["2".into()] }, Extra { name: "Tool".into(), versions: vec!["1".into()] }],
            },
            InstalledMod { name: ORIGINAL_GAME.into(), addons: vec![Extra { name: "Widescreen".into(), versions: vec!["1.1".into()] }], ..Default::default() },
        ]
    }

    fn settings(active: &str, enabled: &[&str]) -> Settings {
        Settings { active_mod: Some(active.into()), enabled: enabled.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    fn roots(r: &Resolved) -> Vec<String> {
        r.layers.iter().map(|l| l.root.to_string_lossy().replace('\\', "/").rsplit(".genlauncher/mods/").next().unwrap().to_string()).collect()
    }

    #[test]
    fn layers_in_override_order_with_newest_versions() {
        let s = settings("contra", &["contra/addon/maps", "contra/patch/hotfix", "contra/exe/tool"]);
        let r = resolve(&PathBuf::from("G"), &s, &disk(), &HashMap::new(), Target::Game).unwrap();
        assert_eq!(roots(&r), ["Contra/10.0", "Contra/patches/Hotfix/3", "Contra/addons/Maps/1.0", "Contra/exes/Tool/1"]);
        assert_eq!(r.exe, "generals.exe");
    }

    #[test]
    fn chosen_version_is_used_when_installed_else_newest() {
        let mut s = settings("contra", &[]);
        s.chosen_versions.insert("contra".into(), "9.0".into());
        assert_eq!(roots(&resolve(&PathBuf::from("G"), &s, &disk(), &HashMap::new(), Target::Game).unwrap()), ["Contra/9.0"]);
        s.chosen_versions.insert("contra".into(), "8.0".into()); // not installed
        assert_eq!(roots(&resolve(&PathBuf::from("G"), &s, &disk(), &HashMap::new(), Target::Game).unwrap()), ["Contra/10.0"]);
    }

    #[test]
    fn original_game_has_extras_but_no_mod_layer() {
        let s = settings("original-game", &["original-game/addon/widescreen"]);
        let r = resolve(&PathBuf::from("G"), &s, &disk(), &HashMap::new(), Target::Game).unwrap();
        assert_eq!(roots(&r), ["Original Game/addons/Widescreen/1.1"]);
        let none = resolve(&PathBuf::from("G"), &Settings::default(), &disk(), &HashMap::new(), Target::Game).unwrap();
        assert!(none.layers.is_empty());
    }

    #[test]
    fn rejects_uninstalled_mod_and_two_patches() {
        let err = resolve(&PathBuf::from("G"), &settings("ghost", &[]), &disk(), &HashMap::new(), Target::Game).unwrap_err();
        assert!(err.to_string().contains("not installed"));
        let s = settings("contra", &["contra/patch/hotfix", "contra/patch/other"]);
        let err = resolve(&PathBuf::from("G"), &s, &disk(), &HashMap::new(), Target::Game).unwrap_err();
        assert!(err.to_string().contains("one patch"));
    }

    #[test]
    fn replacing_exe_sets_the_executable_and_helpers_start_alongside() {
        let mut infos = HashMap::new();
        infos.insert("contra/exe/big-exe".to_string(), VersionInfo { executable_file_name: Some("contra.exe".into()), replaces_game_exe: true, ..Default::default() });
        infos.insert("contra/exe/tool".to_string(), VersionInfo { executable_file_name: Some("tool.exe".into()), ..Default::default() });
        let s = settings("contra", &["contra/exe/big-exe", "contra/exe/tool"]);
        let r = resolve(&PathBuf::from("G"), &s, &disk(), &infos, Target::Game).unwrap();
        assert_eq!(r.exe, "contra.exe");
        assert_eq!(r.helpers, ["tool.exe"]);
        // World Builder ignores a game-exe replacement
        let wb = resolve(&PathBuf::from("G"), &s, &disk(), &infos, Target::WorldBuilder).unwrap();
        assert_eq!(wb.exe, "WorldBuilder.exe");
        // two replacements conflict
        infos.insert("contra/exe/tool".to_string(), VersionInfo { executable_file_name: Some("t.exe".into()), replaces_game_exe: true, ..Default::default() });
        assert!(resolve(&PathBuf::from("G"), &s, &disk(), &infos, Target::Game).unwrap_err().to_string().contains("More than one"));
    }

    #[test]
    fn argument_building() {
        assert_eq!(split_args(r#"-xres 1920 -mod "C:\My Mods" "" -x"#), ["-xres", "1920", "-mod", r"C:\My Mods", "", "-x"]);
        let mut s = Settings::default(); // windowed + quick start on
        s.game_params = "-fps 60".into();
        assert_eq!(game_args(&s), ["-win", "-quickstart", "-noshellmap", "-fps", "60"]);
        s.windowed = false;
        s.quick_start = false;
        s.game_params.clear();
        assert!(game_args(&s).is_empty());
    }
}

#[cfg(test)]
mod owner_tests {
    use super::*;
    use crate::library::Extra;
    use std::path::PathBuf;

    #[test]
    fn addon_filed_under_a_patch_folder_is_still_found() {
        // 1.x layout: .genlauncher/mods/Hotfix/addons/Maps/1.0 belongs to the addon contra/addon/maps
        let disk = vec![
            InstalledMod { name: "Contra".into(), versions: vec!["10".into()], ..Default::default() },
            InstalledMod { name: "Hotfix".into(), addons: vec![Extra { name: "Maps".into(), versions: vec!["1.0".into()] }], ..Default::default() },
        ];
        let s = Settings { active_mod: Some("contra".into()), enabled: vec!["contra/addon/maps".into()], ..Default::default() };
        let r = resolve(&PathBuf::from("G"), &s, &disk, &HashMap::new(), Target::Game).unwrap();
        let roots: Vec<String> = r.layers.iter().map(|l| l.root.to_string_lossy().replace('\\', "/")).collect();
        assert!(roots[0].ends_with(".genlauncher/mods/Contra/10"));
        assert!(roots[1].ends_with(".genlauncher/mods/Hotfix/addons/Maps/1.0"), "{roots:?}");
        // another mod's ids never pull this in
        let other = Settings { active_mod: Some("contra".into()), enabled: vec!["shockwave/addon/maps".into()], ..Default::default() };
        assert_eq!(resolve(&PathBuf::from("G"), &other, &disk, &HashMap::new(), Target::Game).unwrap().layers.len(), 1);
    }
}
