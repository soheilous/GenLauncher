//! Versioned backups of the launcher configuration, kept in the launcher's own settings folder
//! (`<config dir>/backups/config-YYYYMMDD-HHMMSS-<label>.json`). The newest ones are kept, older ones are pruned.
//! Cloud storage will hold the same files.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::ConfigFile;
use crate::error::{Error, Result};

/// How many backups are kept.
pub const KEEP: usize = 20;

pub fn dir(config_dir: &Path) -> PathBuf {
    config_dir.join("backups")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    /// File name; also the handle used to restore or delete it.
    pub name: String,
    /// Unix seconds (UTC) taken from the name.
    pub at: u64,
    /// Why it was made: `manual`, `before-import`, `before-restore`...
    pub label: String,
    pub bytes: u64,
}

/// `(year, month, day, hour, minute, second)` in UTC.
fn civil(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, (rem / 3600) as u32, (rem % 3600 / 60) as u32, (rem % 60) as u32)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn clean_label(label: &str) -> String {
    let l: String = label.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(24).collect();
    if l.is_empty() { "manual".into() } else { l.to_ascii_lowercase() }
}

pub fn file_name(at: u64, label: &str) -> String {
    let (y, mo, d, h, mi, s) = civil(at);
    format!("config-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}-{}.json", clean_label(label))
}

/// Read `config-YYYYMMDD-HHMMSS-label.json`. Anything else in the folder is not ours.
pub fn parse_name(name: &str) -> Option<(u64, String)> {
    let rest = name.strip_prefix("config-")?.strip_suffix(".json")?;
    let (date, rest) = rest.split_once('-')?;
    let (time, label) = rest.split_once('-')?;
    let num = |s: &str, n: usize| (s.len() == n && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u32>().ok()).flatten();
    let (y, mo, d) = (num(date.get(0..4)?, 4)?, num(date.get(4..6)?, 2)?, num(date.get(6..8)?, 2)?);
    if date.len() != 8 || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let (h, mi, s) = (num(time.get(0..2)?, 2)?, num(time.get(2..4)?, 2)?, num(time.get(4..6)?, 2)?);
    if time.len() != 6 || h > 23 || mi > 59 || s > 59 || label.is_empty() || !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    let days = days_from_civil(i64::from(y), mo, d);
    let at = u64::try_from(days).ok()? * 86400 + u64::from(h) * 3600 + u64::from(mi) * 60 + u64::from(s);
    Some((at, label.to_string()))
}

/// All backups, newest first.
pub fn list(dir: &Path) -> Vec<Backup> {
    let mut out: Vec<Backup> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let (at, label) = parse_name(&name)?;
                    let meta = e.metadata().ok().filter(|m| m.is_file())?;
                    Some(Backup { name, at, label, bytes: meta.len() })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.name.cmp(&a.name)));
    out
}

/// Save `cfg` as a new backup and drop the oldest ones beyond [`KEEP`].
pub fn create(dir: &Path, cfg: &ConfigFile, label: &str) -> Result<Backup> {
    fs::create_dir_all(dir)?;
    let mut at = cfg.exported_at;
    // two backups in the same second must not overwrite each other
    let name = loop {
        let n = file_name(at, label);
        if !dir.join(&n).exists() {
            break n;
        }
        at += 1;
    };
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, cfg.to_json()?)?;
    fs::rename(&tmp, dir.join(&name))?;
    for old in list(dir).into_iter().skip(KEEP) {
        let _ = fs::remove_file(dir.join(old.name));
    }
    let (at, label) = parse_name(&name).ok_or_else(|| Error::msg("could not name the backup"))?;
    let bytes = fs::metadata(dir.join(&name))?.len();
    Ok(Backup { name, at, label, bytes })
}

/// The file for `name`, refusing anything that is not one of our backup names (no paths, no other files).
pub fn path_of(dir: &Path, name: &str) -> Result<PathBuf> {
    if parse_name(name).is_none() {
        return Err(Error::msg("That is not a backup"));
    }
    let p = dir.join(name);
    if !p.is_file() {
        return Err(Error::msg("That backup no longer exists"));
    }
    Ok(p)
}

pub fn read(dir: &Path, name: &str) -> Result<ConfigFile> {
    let p = path_of(dir, name)?;
    if fs::metadata(&p)?.len() > crate::config::MAX_BYTES {
        return Err(Error::msg("That backup is too large to be a configuration"));
    }
    ConfigFile::parse(&fs::read_to_string(p)?)
}

pub fn delete(dir: &Path, name: &str) -> Result<()> {
    Ok(fs::remove_file(path_of(dir, name)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn cfg_at(at: u64) -> ConfigFile {
        ConfigFile { exported_at: at, ..ConfigFile::new(BTreeMap::new(), serde_json::json!({})) }
    }

    #[test]
    fn names_encode_the_utc_time_and_read_back() {
        assert_eq!(file_name(0, "manual"), "config-19700101-000000-manual.json");
        assert_eq!(file_name(1_000_000_000, "Before Import!"), "config-20010909-014640-beforeimport.json");
        assert_eq!(file_name(1_760_538_600, ""), "config-20251015-143000-manual.json");
        for t in [0u64, 951_782_400, 1_000_000_000, 1_760_538_600, 4_102_444_799] {
            assert_eq!(parse_name(&file_name(t, "before-import")), Some((t, "before-import".to_string())), "{t}");
        }
    }

    #[test]
    fn only_our_names_are_accepted() {
        for bad in ["", "config.json", "config-20260101-000000.json", "config-2026-01-01-manual.json", "config-20261301-000000-x.json",
            "config-20260101-246000-x.json", "../config-20260101-000000-x.json", "config-20260101-000000-x.json.tmp", "config-20260101-000000-a_b.json"] {
            assert!(parse_name(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn create_list_prune_read_delete() {
        let d = tempfile::tempdir().unwrap();
        let dir = dir(d.path());
        assert!(list(&dir).is_empty());
        for i in 0..(KEEP as u64 + 3) {
            create(&dir, &cfg_at(1_700_000_000 + i * 60), "manual").unwrap();
        }
        let all = list(&dir);
        assert_eq!(all.len(), KEEP);
        assert!(all[0].at > all[1].at, "newest first");
        assert_eq!(all[0].at, 1_700_000_000 + (KEEP as u64 + 2) * 60);
        assert!(!dir.join(file_name(1_700_000_000, "manual")).exists(), "the oldest were pruned");

        let b = create(&dir, &cfg_at(1_700_000_000 + 100_000), "before-import").unwrap();
        assert_eq!(b.label, "before-import");
        assert_eq!(read(&dir, &b.name).unwrap().app, "GenLauncher");
        delete(&dir, &b.name).unwrap();
        assert!(read(&dir, &b.name).is_err());
        assert!(!fs::read_dir(&dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn two_backups_in_one_second_both_survive() {
        let d = tempfile::tempdir().unwrap();
        let a = create(d.path(), &cfg_at(1_700_000_000), "manual").unwrap();
        let b = create(d.path(), &cfg_at(1_700_000_000), "manual").unwrap();
        assert_ne!(a.name, b.name);
        assert_eq!(list(d.path()).len(), 2);
    }

    #[test]
    fn paths_and_foreign_files_cannot_be_reached() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("secret.json"), "{}").unwrap();
        for bad in ["secret.json", "../secret.json", "..\\x.json", "/etc/passwd", "config-20260101-000000-x.json"] {
            assert!(path_of(d.path(), bad).is_err(), "{bad}");
            assert!(delete(d.path(), bad).is_err());
        }
        assert!(list(d.path()).is_empty());
    }
}
