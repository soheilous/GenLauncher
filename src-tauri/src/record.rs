//! Where an installed version came from, saved next to it as `<version>.source.json`
//! (a file beside the version folder, so it is never linked into the game). Used to offer
//! "check for update" by reopening the same page, and to show provenance.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::Result;
use crate::store;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SourceRecord {
    /// `file` (picked on this computer), `link` (a download link) or `catalog`.
    pub origin: String,
    /// Direct download link, if any.
    pub url: Option<String>,
    /// The web page the file was taken from (e.g. the ModDB page), if known.
    pub page_url: Option<String>,
    pub archive_name: Option<String>,
    pub sha256: Option<String>,
    /// Unix seconds.
    pub added_at: u64,
}

impl SourceRecord {
    pub fn now(origin: &str) -> Self {
        SourceRecord {
            origin: origin.into(),
            added_at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            ..Default::default()
        }
    }
}

pub fn path_for(version_dir: &Path) -> PathBuf {
    let mut name = version_dir.file_name().map(|n| n.to_owned()).unwrap_or_default();
    name.push(".source.json");
    version_dir.with_file_name(name)
}

pub fn write(version_dir: &Path, rec: &SourceRecord) -> Result<()> {
    store::save(&path_for(version_dir), rec)
}

pub fn read(version_dir: &Path) -> Option<SourceRecord> {
    let text = fs::read_to_string(path_for(version_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Best effort: a stale record is harmless.
pub fn remove(version_dir: &Path) {
    let _ = fs::remove_file(path_for(version_dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_beside_the_version_folder_and_is_not_a_version() {
        let d = tempfile::tempdir().unwrap();
        let version = d.path().join("Contra/10.0");
        fs::create_dir_all(&version).unwrap();
        fs::write(version.join("a.gib"), b"x").unwrap();
        let mut rec = SourceRecord::now("link");
        rec.page_url = Some("https://www.moddb.com/mods/contra".into());
        rec.sha256 = Some("ab".repeat(32));
        write(&version, &rec).unwrap();

        assert_eq!(path_for(&version), d.path().join("Contra/10.0.source.json"));
        assert_eq!(read(&version), Some(rec));
        // the library scan only sees folders, so the sidecar is never mistaken for a version
        let lib_root = d.path().join(".genlauncher/mods");
        fs::create_dir_all(lib_root.join("M/1.0")).unwrap();
        fs::write(lib_root.join("M/1.0/f.gib"), b"x").unwrap();
        write(&lib_root.join("M/1.0"), &SourceRecord::now("file")).unwrap();
        assert_eq!(crate::library::scan(d.path())[0].versions, ["1.0"]);
        remove(&version);
        assert_eq!(read(&version), None);
    }

    #[test]
    fn unreadable_record_is_just_absent() {
        let d = tempfile::tempdir().unwrap();
        let v = d.path().join("V");
        fs::write(path_for(&v), "not json").unwrap();
        assert_eq!(read(&v), None);
    }
}
