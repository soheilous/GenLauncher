//! The mod repository format shared with GenLauncher 1.x (YamlDotNet-written yaml on GitHub).
//! Field names must match the remote files exactly, hence the explicit renames.

use futures::{StreamExt, stream};
use serde::{Deserialize, Deserializer, Serialize};
use std::time::Duration;

use crate::error::Result;

// The YamlDotNet writer emits `Key:` with no value for null lists/strings; accept that.
fn null_vec<'de, D, T>(d: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

fn null_bool<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<bool, D::Error> {
    Ok(Option::<bool>::deserialize(d)?.unwrap_or(false))
}

/// Strings that may be written as bare numbers in yaml (`Version: 1.87`).
fn loose_string<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        S(String),
        I(i64),
        F(f64),
        B(bool),
    }
    Ok(Option::<Any>::deserialize(d)?.map(|v| match v {
        Any::S(s) => s,
        Any::I(i) => i.to_string(),
        Any::F(f) => f.to_string(),
        Any::B(b) => b.to_string(),
    }))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModType {
    Addon,
    Patch,
    Advertising,
    Executable,
    /// Also what a missing or unrecognised value becomes.
    #[default]
    #[serde(other)]
    Mod,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "LauncherVersion", default, deserialize_with = "loose_string")]
    pub launcher_version: Option<String>,
    #[serde(rename = "DownloadLink", default)]
    pub download_link: Option<String>,
    #[serde(rename = "VulkanReposData", default)]
    pub vulkan_repos_data: Option<String>,
    #[serde(rename = "modDatas", default, deserialize_with = "null_vec")]
    pub mod_datas: Vec<ModEntry>,
    #[serde(rename = "originalGameAddons", default, deserialize_with = "null_vec")]
    pub original_game_addons: Vec<String>,
    #[serde(rename = "originalGamePatches", default, deserialize_with = "null_vec")]
    pub original_game_patches: Vec<String>,
    #[serde(rename = "executables", default, deserialize_with = "null_vec")]
    pub executables: Vec<ExeEntry>,
}

/// One mod in the main manifest: where its own yaml lives and which patches/addons it has.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModEntry {
    #[serde(rename = "ModName", default)]
    pub name: String,
    #[serde(rename = "ModLink", default)]
    pub link: String,
    #[serde(rename = "ModPatches", default, deserialize_with = "null_vec")]
    pub patches: Vec<String>,
    #[serde(rename = "ModAddons", default, deserialize_with = "null_vec")]
    pub addons: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExeEntry {
    #[serde(rename = "ModName", default)]
    pub name: String,
    #[serde(rename = "ModLink", default)]
    pub link: String,
    /// Mod this executable belongs to; empty means it applies to the original game.
    #[serde(rename = "DependencyName", default)]
    pub dependency: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Colors {
    #[serde(rename = "GenLauncherActiveColor", default)]
    pub active: Option<String>,
    #[serde(rename = "GenLauncherBorderColor", default)]
    pub border: Option<String>,
    #[serde(rename = "GenLauncherBackgroundImageLink", default)]
    pub background_image_link: Option<String>,
}

/// Latest-version record for a mod, patch, addon or executable.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionInfo {
    #[serde(rename = "ModificationType", default)]
    pub kind: ModType,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Version", default, deserialize_with = "loose_string")]
    pub version: Option<String>,
    #[serde(rename = "SimpleDownloadLink", default)]
    pub download_link: Option<String>,
    #[serde(rename = "UIImageSourceLink", default)]
    pub image_link: Option<String>,
    #[serde(rename = "DiscordLink", default)]
    pub discord_link: Option<String>,
    #[serde(rename = "ModDBLink", default)]
    pub moddb_link: Option<String>,
    #[serde(rename = "NewsLink", default)]
    pub news_link: Option<String>,
    #[serde(rename = "SupportLink", default)]
    pub support_link: Option<String>,
    #[serde(rename = "NetworkInfo", default)]
    pub network_info: Option<String>,
    #[serde(rename = "DependenceName", default)]
    pub dependence_name: Option<String>,
    #[serde(rename = "S3HostLink", default)]
    pub s3_host: Option<String>,
    #[serde(rename = "S3BucketName", default)]
    pub s3_bucket: Option<String>,
    #[serde(rename = "S3FolderName", default)]
    pub s3_folder: Option<String>,
    /// Optional read key a mod author puts in their own manifest for a bucket that is not publicly listable.
    #[serde(rename = "S3HostPublicKey", default)]
    pub s3_access_key: Option<String>,
    #[serde(rename = "S3HostSecretKey", default)]
    pub s3_secret_key: Option<String>,
    #[serde(rename = "Deprecated", default, deserialize_with = "null_bool")]
    pub deprecated: bool,
    #[serde(rename = "ColorsInformation", default)]
    pub colors: Option<Colors>,
    #[serde(rename = "ExecutableFileName", default)]
    pub executable_file_name: Option<String>,
    #[serde(rename = "ReplacesOriginalGameFile", default, deserialize_with = "null_bool")]
    pub replaces_game_exe: bool,
    #[serde(rename = "ReplacesOriginalWBFile", default, deserialize_with = "null_bool")]
    pub replaces_wb_exe: bool,
}

pub fn parse<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T> {
    Ok(serde_yaml_ng::from_str(text.trim_start_matches('\u{feff}'))?)
}

pub struct Fetcher {
    http: reqwest::Client,
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("GenLauncher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { http })
    }

    pub async fn text(&self, url: &str) -> Result<String> {
        let resp = self.http.get(url).send().await?.error_for_status()?;
        Ok(resp.text().await?)
    }

    pub async fn manifest(&self, url: &str) -> Result<Manifest> {
        parse(&self.text(url).await?)
    }

    pub async fn version_info(&self, url: &str) -> Result<VersionInfo> {
        parse(&self.text(url).await?)
    }

    /// Fetch many version records concurrently. Entries that fail are `None`
    /// (a dead link in the catalog must not hide the other mods).
    pub async fn version_infos(&self, urls: &[String]) -> Vec<Option<VersionInfo>> {
        stream::iter(urls.iter().cloned())
            .map(|u| async move {
                match self.version_info(&u).await {
                    Ok(v) => Some(v),
                    Err(e) => {
                        log::warn!("skipping {u}: {e}");
                        None
                    }
                }
            })
            .buffered(8) // preserves input order
            .collect()
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_main_manifest_with_nulls() {
        let y = "\u{feff}LauncherVersion: 1.0.1.1\nDownloadLink: https://x/y.exe\nmodDatas:\n- ModName: Contra\n  ModLink: https://x/c.yaml\n  ModPatches:\n  - https://x/p.yaml\n  ModAddons:\noriginalGamePatches:\nexecutables:\n- ModName: Exe\n  ModLink: https://x/e.yaml\n  DependencyName:\n";
        let m: Manifest = parse(y).unwrap();
        assert_eq!(m.launcher_version.as_deref(), Some("1.0.1.1"));
        assert_eq!(m.mod_datas[0].name, "Contra");
        assert_eq!(m.mod_datas[0].patches.len(), 1);
        assert!(m.mod_datas[0].addons.is_empty());
        assert!(m.original_game_patches.is_empty());
        assert!(m.executables[0].dependency.is_none());
    }

    #[test]
    fn parses_version_record_with_numeric_version() {
        let y = "ModificationType: Mod\nName: Rise of the Reds\nVersion: 1.87\nDeprecated:\nColorsInformation:\n  GenLauncherActiveColor: '#ff0000'\n  GenLauncherBackgroundImageLink: https://x/bg.png\nUnknownFutureField: 1\n";
        let v: VersionInfo = parse(y).unwrap();
        assert_eq!(v.kind, ModType::Mod);
        assert_eq!(v.version.as_deref(), Some("1.87"));
        assert!(!v.deprecated);
        assert_eq!(v.colors.unwrap().active.as_deref(), Some("#ff0000"));
    }

    #[test]
    fn unknown_type_defaults_to_mod_and_exe_flags_parse() {
        let y = "ModificationType: Executable\nName: E\nVersion: '2'\nReplacesOriginalGameFile: true\nExecutableFileName: modded.exe\n";
        let v: VersionInfo = parse(y).unwrap();
        assert_eq!(v.kind, ModType::Executable);
        assert!(v.replaces_game_exe && !v.replaces_wb_exe);
        let w: VersionInfo = parse("ModificationType: Banana\nName: X\n").unwrap();
        assert_eq!(w.kind, ModType::Mod);
    }
}
