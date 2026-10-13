//! Installing one mod/patch/addon/executable version.
//!
//! Everything is built in a staging folder (`.genlauncher/tmp/…/content`) and only renamed to
//! `<version>` when it is complete and verified, so an interrupted or failed install can never
//! look installed (and an existing version is only replaced once the new one is ready). The
//! staging folder is kept on failure so the next attempt resumes instead of starting over.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::download::{self, Cancel, Options};
use crate::error::{Error, Result};
use crate::extract;
use crate::links;

#[derive(Clone, Debug)]
pub enum Source {
    /// One archive (zip / 7z / rar) or file behind a link.
    Archive { url: String },
    /// A file or folder the user picked on their own computer (nothing is downloaded).
    Local { path: PathBuf },
}

#[derive(Clone, Debug)]
pub struct Job {
    /// Final folder of this version (`.genlauncher/mods/<Mod>/<Version>`).
    pub dest: PathBuf,
    /// Work folder for this download (`.genlauncher/tmp/…`). Kept after a failure so it can resume.
    pub staging: PathBuf,
    pub source: Source,
    /// Newest installed version of the same item; unchanged files are reused from it.
    pub previous: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Preparing,
    Downloading,
    Verifying,
    Extracting,
    Finishing,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: Phase,
    pub done: u64,
    /// 0 when unknown (shown as an indeterminate bar).
    pub total: u64,
}

pub type Report = Arc<dyn Fn(Progress) + Send + Sync>;

/// Byte counter that reports at most every ~120 ms.
struct Meter {
    report: Report,
    done: AtomicI64,
    total: AtomicU64,
    phase: Mutex<Phase>,
    last_ms: AtomicU64,
    started: Instant,
}

impl Meter {
    fn new(report: Report) -> Arc<Self> {
        Arc::new(Self {
            report,
            done: AtomicI64::new(0),
            total: AtomicU64::new(0),
            phase: Mutex::new(Phase::Preparing),
            last_ms: AtomicU64::new(0),
            started: Instant::now(),
        })
    }
    fn emit(&self, force: bool) {
        let now = self.started.elapsed().as_millis() as u64;
        if force || now.saturating_sub(self.last_ms.load(Ordering::Relaxed)) >= 120 {
            self.last_ms.store(now, Ordering::Relaxed);
            (self.report)(Progress {
                phase: *self.phase.lock().unwrap(),
                done: self.done.load(Ordering::Relaxed).max(0) as u64,
                total: self.total.load(Ordering::Relaxed),
            });
        }
    }
    fn set_phase(&self, p: Phase) {
        *self.phase.lock().unwrap() = p;
        self.emit(true);
    }
    fn set_total(&self, t: u64) {
        self.total.store(t, Ordering::Relaxed);
    }
    fn add(&self, d: i64) {
        self.done.fetch_add(d, Ordering::Relaxed);
        self.emit(false);
    }
}

fn dir_has_files(dir: &Path) -> bool {
    fs::read_dir(dir).map(|rd| rd.flatten().any(|e| e.path().is_file() || (e.path().is_dir() && dir_has_files(&e.path())))).unwrap_or(false)
}

fn link_or_copy(from: &Path, to: &Path) -> std::io::Result<()> {
    if let Some(p) = to.parent() {
        fs::create_dir_all(p)?;
    }
    fs::hard_link(from, to).or_else(|_| fs::copy(from, to).map(|_| ()))
}

/// What the installed content came from, for the record kept next to it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// File name of the archive that was installed (none for folders).
    pub archive_name: Option<String>,
    /// SHA-256 of that archive.
    pub sha256: Option<String>,
}

pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(hex::encode(h.finalize()));
        }
        h.update(&buf[..n]);
    }
}

pub async fn run(client: &reqwest::Client, job: &Job, cancel: &Cancel, report: Report, opts: &Options) -> Result<Outcome> {
    let meter = Meter::new(report);
    meter.set_phase(Phase::Preparing);
    let temp = job.staging.clone();
    let content = temp.join("content");
    fs::create_dir_all(&temp)?;

    let outcome = match &job.source {
        Source::Archive { url } => archive(client, url, &temp, &content, cancel, &meter, opts).await?,
        Source::Local { path } => {
            let _ = fs::remove_dir_all(&content);
            meter.set_phase(Phase::Extracting);
            unpack(path.clone(), content.clone(), cancel.clone()).await?
        }
    };
    cancel.check()?;
    if !dir_has_files(&content) {
        return Err(Error::msg("there were no files to install"));
    }
    meter.set_phase(Phase::Finishing);
    finalize(&temp, &content, &job.dest)?;
    meter.emit(true);
    Ok(outcome)
}

/// Copy a folder the user picked into `dst` (never hard-links: their original stays theirs).
fn copy_folder(src: &Path, dst: &Path, rel: &Path, cancel: &Cancel) -> Result<()> {
    for e in fs::read_dir(src)? {
        cancel.check()?;
        let e = e?;
        let path = e.path();
        let rel = rel.join(e.file_name());
        let ft = e.file_type()?;
        if ft.is_dir() {
            copy_folder(&path, dst, &rel, cancel)?;
        } else if ft.is_file() {
            // safe_target also rejects odd names and stores archives as .gib
            let target = extract::safe_target(dst, &rel.to_string_lossy())?;
            if let Some(p) = target.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

/// Turn a downloaded or user-chosen file (archive, single file) or folder into installed content.
async fn unpack(src: PathBuf, dst: PathBuf, cancel: Cancel) -> Result<Outcome> {
    tokio::task::spawn_blocking(move || -> Result<Outcome> {
        if src.is_dir() {
            fs::create_dir_all(&dst)?;
            copy_folder(&src, &dst, Path::new(""), &cancel)?;
            return Ok(Outcome::default());
        }
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if crate::browser::is_program_installer(&name) {
            return Err(Error::msg(format!(
                "{name} is a program installer, not a mod archive. GenLauncher only installs zip, 7z and rar files (or a single .big file); download a different file from the page."
            )));
        }
        let outcome = Outcome { archive_name: Some(name.clone()), sha256: Some(sha256_file(&src)?) };
        if extract::is_archive_name(&name) || extract::sniff(&src).is_ok() {
            if let Err(e) = extract::extract(&src, &dst, &|| cancel.is_cancelled()) {
                let _ = fs::remove_dir_all(&dst); // never leave half an extraction behind
                return Err(e);
            }
        } else {
            // a plain file (e.g. a single .big): keep it, archives are stored as .gib
            let target = extract::safe_target(&dst, &name)?;
            link_or_copy(&src, &target)?;
        }
        Ok(outcome)
    })
    .await
    .map_err(|e| Error::msg(e.to_string()))?
}

/// Swap the finished content in; the previous folder (reinstall) is only removed afterwards.
fn finalize(temp: &Path, content: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let old = {
        let mut n = temp.file_name().map(|n| n.to_owned()).unwrap_or_default();
        n.push(".replaced");
        temp.with_file_name(n)
    };
    let had_old = dest.exists();
    if had_old {
        let _ = fs::remove_dir_all(&old);
        fs::rename(dest, &old)?;
    }
    if let Err(e) = fs::rename(content, dest) {
        if had_old {
            let _ = fs::rename(&old, dest); // put the previous version back
        }
        return Err(e.into());
    }
    let _ = fs::remove_dir_all(&old);
    let _ = fs::remove_dir_all(temp);
    Ok(())
}

async fn archive(client: &reqwest::Client, url: &str, temp: &Path, content: &Path, cancel: &Cancel, meter: &Arc<Meter>, opts: &Options) -> Result<Outcome> {
    meter.set_phase(Phase::Downloading);
    let marker = temp.join(".archive-name");
    let existing = fs::read_to_string(&marker).ok().map(|n| temp.join(n.trim())).filter(|p| p.is_file());
    let file = match existing {
        Some(p) => p, // an earlier attempt downloaded it completely
        None => {
            let part = temp.join("download.part");
            let url = links::direct_download_link(url);
            let m = meter.clone();
            let m2 = meter.clone();
            let got = download::fetch(client, &url, &part, cancel, &move |d| m.add(d), &move |t| m2.set_total(t), false, opts).await?;
            let path = temp.join(&got.file_name);
            let _ = fs::remove_file(&path);
            fs::rename(&part, &path)?;
            fs::write(&marker, &got.file_name)?;
            path
        }
    };

    let _ = fs::remove_dir_all(content);
    meter.set_total(0);
    meter.set_phase(Phase::Extracting);
    unpack(file, content.to_path_buf(), cancel.clone()).await
}
