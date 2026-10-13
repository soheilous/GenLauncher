//! Locating and recognising Generals / Zero Hour installs.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::game_files;

pub const REPLACE_SUFFIX: &str = ".GLR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    Zh,
    Generals,
}

impl Game {
    /// Archives shipped with this game (lowercase file names).
    pub fn stock_archives(self) -> &'static [&'static str] {
        match self {
            Game::Zh => game_files::ZERO_HOUR,
            Game::Generals => game_files::GENERALS,
        }
    }

    pub fn manifest_url(self) -> &'static str {
        match self {
            Game::Zh => "https://raw.githubusercontent.com/p0ls3r/GenLauncherModsData/master/ReposModificationDataZH4.yaml",
            Game::Generals => "https://raw.githubusercontent.com/p0ls3r/GenLauncherModsData/master/ReposModificationDataGenerals3.yaml",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Game::Zh => "zh",
            Game::Generals => "generals",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameInstall {
    pub dir: PathBuf,
    pub game: Game,
}

/// Case-insensitive lookup of a file directly inside `dir` (the game and its mods assume
/// a case-insensitive filesystem; ext4 is not one).
pub fn find_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.exists() {
        return Some(direct);
    }
    let wanted = name.to_lowercase();
    fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        (e.file_name().to_string_lossy().to_lowercase() == wanted).then(|| e.path())
    })
}

/// A file may be parked under `<name>.GLR` if a previous session crashed mid-launch.
fn has_file(dir: &Path, name: &str) -> bool {
    find_ci(dir, name).is_some() || find_ci(dir, &format!("{name}{REPLACE_SUFFIX}")).is_some()
}

/// Recognise a game folder. Same markers GenLauncher 1.x used: `generals.exe`,
/// `BINKW32.DLL` and a window archive (`WindowZH.big` = Zero Hour, `Window.big` = Generals).
pub fn detect(dir: &Path) -> Option<GameInstall> {
    if !dir.is_dir() || find_ci(dir, "generals.exe").is_none() || find_ci(dir, "BINKW32.DLL").is_none() {
        return None;
    }
    if has_file(dir, "WindowZH.big") {
        Some(GameInstall { dir: dir.to_path_buf(), game: Game::Zh })
    } else if has_file(dir, "Window.big") {
        Some(GameInstall { dir: dir.to_path_buf(), game: Game::Generals })
    } else {
        None
    }
}

const STEAM_FOLDERS: &[&str] = &["Command & Conquer Generals - Zero Hour", "Command and Conquer Generals"];

fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(p) = std::env::var_os(var) {
            roots.push(PathBuf::from(p).join("Steam"));
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".steam/steam"));
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    }
    roots
}

/// Extra Steam library paths from `steamapps/libraryfolders.vdf` (`"path"  "D:\\Games\\Steam"`).
fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("\"path\"")?.trim();
            let val = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(PathBuf::from(val.replace("\\\\", "\\")))
        })
        .collect()
}

fn candidate_dirs(extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = extra.to_vec();
    if let Some(p) = std::env::var_os("GENLAUNCHER_GAME_DIR") {
        out.push(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            out.push(d.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd);
    }
    let mut libraries = steam_roots();
    for root in steam_roots() {
        if let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            libraries.extend(parse_library_folders(&text));
        }
    }
    for lib in libraries {
        for name in STEAM_FOLDERS {
            out.push(lib.join("steamapps").join("common").join(name));
        }
    }
    out
}

/// Every distinct game install found, in priority order (`extra` first, e.g. the saved choice).
pub fn discover(extra: &[PathBuf]) -> Vec<GameInstall> {
    let mut found: Vec<GameInstall> = Vec::new();
    for dir in candidate_dirs(extra) {
        let Some(install) = detect(&dir) else { continue };
        let canon = fs::canonicalize(&install.dir).unwrap_or_else(|_| install.dir.clone());
        if !found.iter().any(|f| fs::canonicalize(&f.dir).unwrap_or_else(|_| f.dir.clone()) == canon) {
            found.push(install);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn touch(dir: &Path, name: &str) {
        File::create(dir.join(name)).unwrap();
    }

    #[test]
    fn detects_zero_hour_case_insensitively() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "GENERALS.EXE");
        touch(d.path(), "binkw32.dll");
        touch(d.path(), "windowzh.big");
        assert_eq!(detect(d.path()).unwrap().game, Game::Zh);
    }

    #[test]
    fn detects_generals() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "generals.exe");
        touch(d.path(), "BINKW32.DLL");
        touch(d.path(), "Window.big");
        assert_eq!(detect(d.path()).unwrap().game, Game::Generals);
    }

    #[test]
    fn recognises_folder_left_mid_launch() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "generals.exe");
        touch(d.path(), "BINKW32.DLL");
        touch(d.path(), "WindowZH.big.GLR"); // parked by a crashed launch
        assert_eq!(detect(d.path()).unwrap().game, Game::Zh);
    }

    #[test]
    fn rejects_other_folders() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "generals.exe");
        assert!(detect(d.path()).is_none());
        assert!(detect(&d.path().join("missing")).is_none());
    }

    #[test]
    fn parses_steam_library_folders() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}";
        let libs = parse_library_folders(vdf);
        assert_eq!(libs, [PathBuf::from("C:\\Program Files (x86)\\Steam"), PathBuf::from("D:\\SteamLibrary")]);
    }

    #[test]
    fn stock_lists_are_populated() {
        assert!(Game::Zh.stock_archives().contains(&"inizh.big"));
        assert!(Game::Generals.stock_archives().contains(&"ini.big"));
    }
}
