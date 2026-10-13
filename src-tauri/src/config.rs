//! A launcher configuration that can leave the computer: exported to a file, imported from one, or (later) kept in the
//! user's cloud storage. It holds choices only (settings per game and look-and-feel), never mod files or file paths.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::store::Settings;

pub const APP: &str = "GenLauncher";
pub const FORMAT: u32 = 1;
/// A configuration file is a few kilobytes (a few megabytes with custom cover images); anything bigger is not one.
pub const MAX_BYTES: u64 = 24 * 1024 * 1024;
/// The games a configuration may carry settings for.
pub const GAMES: [&str; 2] = ["zh", "generals"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfigFile {
    pub app: String,
    pub format: u32,
    /// Unix seconds.
    pub exported_at: u64,
    /// Settings per game (`zh`, `generals`).
    pub games: BTreeMap<String, Settings>,
    /// Look and feel kept by the UI (theme, sidebar, covers...). Opaque here: the UI validates it.
    pub ui: serde_json::Value,
}

impl ConfigFile {
    pub fn new(games: BTreeMap<String, Settings>, ui: serde_json::Value) -> Self {
        let exported_at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        ConfigFile { app: APP.into(), format: FORMAT, exported_at, games, ui }
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)? + "\n")
    }

    /// Read and check a configuration. Anything that is not one of ours, or is from a newer launcher, is refused.
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() as u64 > MAX_BYTES {
            return Err(Error::msg("That file is too large to be a GenLauncher configuration"));
        }
        let mut cfg: ConfigFile =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| Error::msg("That is not a GenLauncher configuration file"))?;
        if cfg.app != APP {
            return Err(Error::msg("That is not a GenLauncher configuration file"));
        }
        if cfg.format > FORMAT {
            return Err(Error::msg("That configuration was saved by a newer GenLauncher. Update the launcher first."));
        }
        cfg.games.retain(|k, _| GAMES.contains(&k.as_str()));
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ConfigFile {
        let mut games = BTreeMap::new();
        games.insert("zh".to_string(), Settings { camera_height: 120, enabled: vec!["contra/patch/hotfix".into()], ..Default::default() });
        ConfigFile::new(games, serde_json::json!({ "theme": "light" }))
    }

    #[test]
    fn round_trips() {
        let cfg = sample();
        assert_eq!(ConfigFile::parse(&cfg.to_json().unwrap()).unwrap(), cfg);
    }

    #[test]
    fn only_our_own_files_are_accepted() {
        assert!(ConfigFile::parse("not json").is_err());
        assert!(ConfigFile::parse("{}").is_err()); // no app tag
        assert!(ConfigFile::parse(r#"{"app":"Other","format":1}"#).is_err());
        let newer = ConfigFile { format: FORMAT + 1, ..sample() };
        assert!(ConfigFile::parse(&newer.to_json().unwrap()).unwrap_err().to_string().contains("newer"));
        assert!(ConfigFile::parse(&"x".repeat(MAX_BYTES as usize + 1)).unwrap_err().to_string().contains("too large"));
    }

    #[test]
    fn unknown_games_and_fields_are_dropped_or_ignored() {
        let text = r#"{"app":"GenLauncher","format":1,"future":true,"games":{"zh":{"windowed":false,"newSetting":1},"Evil":{"windowed":true}}}"#;
        let cfg = ConfigFile::parse(text).unwrap();
        assert_eq!(cfg.games.keys().collect::<Vec<_>>(), ["zh"]);
        assert!(!cfg.games["zh"].windowed);
        assert!(cfg.games["zh"].quick_start); // missing settings keep their defaults
    }
}
