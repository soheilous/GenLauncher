//! Persistent JSON state with atomic writes. A corrupt file is set aside (never silently
//! deleted like GenLauncher 1.x did with its config) and defaults are used instead.

use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;


pub fn data_dir(game_dir: &Path) -> PathBuf {
    crate::layout::state(game_dir)
}

pub fn load<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(text) = fs::read_to_string(path) else { return T::default() };
    match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let aside = path.with_extension(format!("json.bad-{ts}"));
            log::warn!("{} is corrupt ({e}); moved to {}", path.display(), aside.display());
            let _ = fs::rename(path, aside);
            T::default()
        }
    }
}

/// Write to a temp file in the same folder, then rename over the target.
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// A program the user wants started together with the game (an edge scroller for windowed mode, an overlay, a controller mapper...).
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct ExtraProgram {
    pub path: String,
    /// Start it only when the game runs in a window.
    pub only_windowed: bool,
}

/// User choices that belong to one game folder.
#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub windowed: bool,
    pub quick_start: bool,
    pub camera_height: u32,
    pub game_params: String,
    pub check_mod_files: bool,
    pub ask_before_check: bool,
    pub hide_launcher_while_playing: bool,
    pub gentool_auto_update: bool,
    pub auto_delete_old_versions: bool,
    pub modded_exe: bool,
    pub use_vulkan: bool,
    /// Programs started together with the game.
    pub extra_programs: Vec<ExtraProgram>,
    /// Keep each mod version's saved games and replays apart (see `profiles`).
    pub saves_by_mod: bool,
    pub active_mod: Option<String>,
    /// Ids of enabled patches / addons / executables (see `catalog::item_id`).
    pub enabled: Vec<String>,
    /// Version picked in the UI per mod id; absent = newest installed.
    pub chosen_versions: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            windowed: true,
            quick_start: true,
            camera_height: 0,
            game_params: String::new(),
            check_mod_files: true,
            ask_before_check: true,
            hide_launcher_while_playing: false,
            gentool_auto_update: true,
            auto_delete_old_versions: false,
            modded_exe: true,
            use_vulkan: false,
            extra_programs: Vec::new(),
            saves_by_mod: false,
            active_mod: None,
            enabled: Vec::new(),
            chosen_versions: HashMap::new(),
        }
    }
}

/// Launcher-wide (not per game folder): which install was used last.
#[derive(Debug, Clone, Default, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub last_game_dir: Option<PathBuf>,
    pub extra_game_dirs: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_defaults() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub/settings.json");
        assert_eq!(load::<Settings>(&p), Settings::default());
        let s = Settings { camera_height: 300, game_params: "-xres 1920".into(), ..Default::default() };
        save(&p, &s).unwrap();
        assert_eq!(load::<Settings>(&p), s);
        assert!(!p.with_extension("json.tmp").exists());
    }

    #[test]
    fn corrupt_file_is_set_aside_not_deleted() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("settings.json");
        fs::write(&p, "{ not json").unwrap();
        assert_eq!(load::<Settings>(&p), Settings::default());
        assert!(!p.exists());
        let bad: Vec<_> = fs::read_dir(d.path()).unwrap().flatten().collect();
        assert_eq!(bad.len(), 1);
        assert!(bad[0].file_name().to_string_lossy().contains(".bad-"));
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.json");
        fs::write(&p, r#"{"windowed": false, "futureField": 1}"#).unwrap();
        let s: Settings = load(&p);
        assert!(!s.windowed);
        assert!(s.quick_start);
    }
}
