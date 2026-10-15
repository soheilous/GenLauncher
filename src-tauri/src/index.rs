//! The mod catalog: a list of mods and where to find them, kept in its own repository
//! (github.com/soheilous/generals-mod-catalog, `catalog/*.json`).
//!
//! It holds pointers only: names, the latest known version, the page a mod is published on, and some styling. It never
//! contains a download link or a file: the launcher opens the page in its built-in browser and the user downloads the
//! file from the people who published it. Anyone can improve the catalog with a pull request.
//!
//! The first version of the index was converted from the mod list of GenLauncher 1.x by p0ls3r
//! (github.com/p0ls3r/GenLauncherModsData); see the credits in the README.

use serde::{Deserialize, Serialize};

pub const FORMAT: u32 = 1;

fn is_false(b: &bool) -> bool {
    !*b
}

/// One mod, patch, addon or executable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub name: String,
    /// Latest version the index knows of (free text, e.g. `1.87` or `009 Final`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The page to get it from (ModDB, Nexus Mods, the author's site...), opened in the built-in browser.
    #[serde(rename = "page", skip_serializing_if = "Option::is_none")]
    pub moddb_link: Option<String>,
    /// Cover image.
    #[serde(rename = "image", skip_serializing_if = "Option::is_none")]
    pub image_link: Option<String>,
    #[serde(rename = "discord", skip_serializing_if = "Option::is_none")]
    pub discord_link: Option<String>,
    #[serde(rename = "news", skip_serializing_if = "Option::is_none")]
    pub news_link: Option<String>,
    #[serde(rename = "support", skip_serializing_if = "Option::is_none")]
    pub support_link: Option<String>,
    #[serde(rename = "network", skip_serializing_if = "Option::is_none")]
    pub network_info: Option<String>,
    /// For an addon: the patch (or mod) it needs.
    #[serde(rename = "dependsOn", skip_serializing_if = "Option::is_none")]
    pub dependence_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Colors>,
    #[serde(skip_serializing_if = "is_false")]
    pub deprecated: bool,
    /// For an executable: the file it provides.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_file_name: Option<String>,
    #[serde(rename = "replacesGameExe", skip_serializing_if = "is_false")]
    pub replaces_game_exe: bool,
    #[serde(rename = "replacesWorldBuilderExe", skip_serializing_if = "is_false")]
    pub replaces_wb_exe: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Colors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    #[serde(rename = "backgroundImage", skip_serializing_if = "Option::is_none")]
    pub background_image_link: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModEntry {
    #[serde(flatten)]
    pub entry: Entry,
    pub patches: Vec<Entry>,
    pub addons: Vec<Entry>,
}

/// An executable (e.g. a community-patched game exe), for one mod or for any.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExeEntry {
    #[serde(flatten)]
    pub entry: Entry,
    /// The mod it belongs to; empty means it can be used with any mod.
    #[serde(rename = "for", skip_serializing_if = "Option::is_none")]
    pub for_mod: Option<String>,
}

/// Patches and addons for the stock game.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Extras {
    pub patches: Vec<Entry>,
    pub addons: Vec<Entry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Index {
    pub format: u32,
    /// `zh` or `generals`.
    pub game: String,
    /// When the list was last edited (`YYYY-MM-DD`).
    pub updated: String,
    pub mods: Vec<ModEntry>,
    pub original_game: Extras,
    pub executables: Vec<ExeEntry>,
}

/// A message from the project, shown in the launcher's Notifications page (`catalog/announcements.json` in the catalog repository).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Announcement {
    /// Stable id: a message is shown once per person.
    pub id: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub title: String,
    pub text: String,
    /// Optional page to open from the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AnnouncementFile {
    format: u32,
    items: Vec<Announcement>,
}

pub const ANNOUNCEMENTS_URL: &str = "https://raw.githubusercontent.com/soheilous/generals-mod-catalog/main/catalog/announcements.json";

pub fn parse_announcements(text: &str) -> crate::error::Result<Vec<Announcement>> {
    let f: AnnouncementFile = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    if f.format > FORMAT {
        return Err(crate::error::Error::msg("These announcements need a newer GenLauncher"));
    }
    Ok(f.items.into_iter().filter(|a| !a.id.trim().is_empty() && !a.text.trim().is_empty()).collect())
}

pub fn parse(text: &str) -> crate::error::Result<Index> {
    let idx: Index = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    if idx.format > FORMAT {
        return Err(crate::error::Error::msg("This mod index needs a newer GenLauncher"));
    }
    Ok(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_documented_shape_and_ignores_unknown_fields() {
        let json = r##"{
            "format": 1, "game": "zh", "updated": "2026-10-14", "someFutureField": true,
            "mods": [{
                "name": "Contra", "version": "10", "page": "https://www.moddb.com/mods/contra",
                "colors": { "active": "#ff0000", "backgroundImage": "https://x/bg.png" },
                "patches": [{ "name": "Hotfix", "version": "4" }],
                "addons": [{ "name": "Maps", "dependsOn": "Hotfix" }]
            }],
            "originalGame": { "patches": [{ "name": "Fix" }] },
            "executables": [{ "name": "modded.exe", "for": "Contra", "replacesGameExe": true, "executableFileName": "modded.exe" }]
        }"##;
        let idx = parse(json).unwrap();
        let m = &idx.mods[0];
        assert_eq!(m.entry.moddb_link.as_deref(), Some("https://www.moddb.com/mods/contra"));
        assert_eq!(m.entry.colors.as_ref().unwrap().background_image_link.as_deref(), Some("https://x/bg.png"));
        assert_eq!(m.patches[0].name, "Hotfix");
        assert_eq!(m.addons[0].dependence_name.as_deref(), Some("Hotfix"));
        assert_eq!(idx.original_game.patches[0].name, "Fix");
        assert!(idx.executables[0].entry.replaces_game_exe);
        assert_eq!(idx.executables[0].for_mod.as_deref(), Some("Contra"));
    }

    #[test]
    fn round_trips_and_leaves_out_empty_fields() {
        let idx = Index {
            format: FORMAT,
            game: "zh".into(),
            updated: "2026-10-14".into(),
            mods: vec![ModEntry { entry: Entry { name: "A".into(), version: Some("1".into()), ..Default::default() }, ..Default::default() }],
            ..Default::default()
        };
        let text = serde_json::to_string(&idx).unwrap();
        assert!(!text.contains("deprecated") && !text.contains("discord") && !text.contains("download"), "{text}");
        assert_eq!(parse(&text).unwrap(), idx);
    }

    #[test]
    fn announcements_skip_incomplete_entries() {
        let a = parse_announcements(
            r#"{"format":1,"items":[{"id":"a","date":"2026-10-14","title":"T","text":"Hello","link":"https://x"},{"id":"","text":"no id"},{"id":"b","text":""}]}"#,
        )
        .unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].link.as_deref(), Some("https://x"));
        assert!(parse_announcements(r#"{"format":9}"#).is_err());
    }

    #[test]
    fn a_newer_format_is_refused() {
        assert!(parse(r#"{"format": 99}"#).unwrap_err().to_string().contains("newer"));
    }
}
