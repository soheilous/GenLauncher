mod common;

use common::{Resp, serve};
use genlauncher2_lib::download::{Cancel, Options};
use genlauncher2_lib::install::{Job, Phase, Progress, Report, Source, run};
use std::path::PathBuf;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;


fn fast() -> Options {
    Options { chunk_timeout: Duration::from_millis(500), attempts: 3, backoff: Duration::from_millis(5) }
}


struct Recorder(Arc<Mutex<Vec<Progress>>>);
impl Recorder {
    fn new() -> (Self, Report) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let l2 = log.clone();
        (Recorder(log), Arc::new(move |p| l2.lock().unwrap().push(p)))
    }
}

/// Staging sits next to the destination in tests (the app puts it in .genlauncher/tmp).
fn staging_of(dest: &Path) -> PathBuf {
    dest.with_file_name(format!("{}.stage", dest.file_name().unwrap().to_string_lossy()))
}

fn job(dest: PathBuf, source: Source, previous: Option<PathBuf>) -> Job {
    Job { staging: staging_of(&dest), dest, source, previous }
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

fn read(p: &Path) -> Vec<u8> {
    fs::read(p).unwrap()
}

fn put(p: &Path, d: &[u8]) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, d).unwrap();
}

#[tokio::test]
async fn archive_install_extracts_renames_big_and_cleans_up() {
    let mut zipbuf = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut zipbuf));
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("Art.big", o).unwrap();
        z.write_all(b"archive").unwrap();
        z.start_file("Data/INI/a.ini", o).unwrap();
        z.write_all(b"ini").unwrap();
        z.finish().unwrap();
    }
    let body = zipbuf.clone();
    let addr = serve(Arc::new(move |_| Resp::ok(body.clone()).header("Content-Disposition", r#"attachment; filename="My Mod 1.0.zip""#))).await;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join(".genlauncher/mods/Mod/1.0");
    let (rec, report) = Recorder::new();
    run(&client(), &job(dest.clone(), Source::Archive { url: format!("http://{addr}/dl") }, None), &Cancel::new(), report, &fast())
        .await
        .unwrap();
    assert_eq!(read(&dest.join("Art.gib")), b"archive");
    assert_eq!(read(&dest.join("Data/INI/a.ini")), b"ini");
    assert!(!staging_of(&dest).exists());
    let phases: Vec<Phase> = rec.0.lock().unwrap().iter().map(|p| p.phase).collect();
    assert!(phases.contains(&Phase::Downloading) && phases.contains(&Phase::Extracting) && phases.last() == Some(&Phase::Finishing), "{phases:?}");
}

#[tokio::test]
async fn single_big_file_link_is_stored_as_gib_and_html_pages_fail() {
    let addr = serve(Arc::new(|req| {
        if req.target == "/page" { Resp::ok("<html>sign in</html>").header("Content-Type", "text/html") } else { Resp::ok(b"just an archive".to_vec()).header("Content-Disposition", "attachment; filename=Solo.big") }
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let (_r, report) = Recorder::new();
    let dest = tmp.path().join(".genlauncher/mods/Solo/1");
    run(&client(), &job(dest.clone(), Source::Archive { url: format!("http://{addr}/f") }, None), &Cancel::new(), report.clone(), &fast()).await.unwrap();
    assert_eq!(read(&dest.join("Solo.gib")), b"just an archive");

    let e = run(&client(), &job(tmp.path().join(".genlauncher/mods/Bad/1"), Source::Archive { url: format!("http://{addr}/page") }, None), &Cancel::new(), report, &fast()).await.unwrap_err();
    assert!(e.to_string().contains("web page"), "{e}");
    assert!(!tmp.path().join(".genlauncher/mods/Bad/1").exists());
}

fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut z = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let o = zip::write::SimpleFileOptions::default();
    for (name, data) in entries {
        z.start_file(*name, o).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap();
}

#[tokio::test]
async fn local_archive_is_installed_and_the_original_is_left_alone() {
    use sha2::{Digest, Sha256};
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("My Mod v1.2.zip");
    make_zip(&archive, &[("Art.big", b"archive"), ("Data/INI/a.ini", b"ini")]);
    let dest = tmp.path().join("GLM/Mine/1.2");
    let (rec, report) = Recorder::new();
    let outcome = run(&client(), &job(dest.clone(), Source::Local { path: archive.clone() }, None), &Cancel::new(), report, &fast()).await.unwrap();

    assert_eq!(read(&dest.join("Art.gib")), b"archive");
    assert_eq!(read(&dest.join("Data/INI/a.ini")), b"ini");
    assert!(archive.is_file(), "the user's own file must stay where it was");
    assert_eq!(outcome.archive_name.as_deref(), Some("My Mod v1.2.zip"));
    assert_eq!(outcome.sha256, Some(hex::encode(Sha256::digest(fs::read(&archive).unwrap()))));
    assert!(!staging_of(&dest).exists());
    let phases: Vec<Phase> = rec.0.lock().unwrap().iter().map(|p| p.phase).collect();
    assert!(!phases.contains(&Phase::Downloading) && phases.contains(&Phase::Extracting), "{phases:?}");
}

#[tokio::test]
async fn local_folder_is_copied_with_archives_stored_as_gib() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("my files");
    put(&src.join("Cool.BIG"), b"big");
    put(&src.join("Data/Maps/m.map"), b"map");
    let dest = tmp.path().join("GLM/Mine/1");
    let (_r, report) = Recorder::new();
    let outcome = run(&client(), &job(dest.clone(), Source::Local { path: src.clone() }, None), &Cancel::new(), report, &fast()).await.unwrap();
    assert_eq!(read(&dest.join("Cool.gib")), b"big");
    assert_eq!(read(&dest.join("Data/Maps/m.map")), b"map");
    assert_eq!(read(&src.join("Cool.BIG")), b"big", "the original folder is untouched (copied, not moved or linked)");
    assert_eq!(outcome, genlauncher2_lib::install::Outcome::default());
}

#[tokio::test]
async fn local_single_big_file_and_bad_input() {
    let tmp = tempfile::tempdir().unwrap();
    let (_r, report) = Recorder::new();
    let single = tmp.path().join("Solo.big");
    fs::write(&single, b"one archive").unwrap();
    let dest = tmp.path().join("GLM/Solo/1");
    run(&client(), &job(dest.clone(), Source::Local { path: single }, None), &Cancel::new(), report.clone(), &fast()).await.unwrap();
    assert_eq!(read(&dest.join("Solo.gib")), b"one archive");

    let empty = tmp.path().join("empty folder");
    fs::create_dir_all(&empty).unwrap();
    let e = run(&client(), &job(tmp.path().join("GLM/E/1"), Source::Local { path: empty }, None), &Cancel::new(), report.clone(), &fast()).await.unwrap_err();
    assert!(e.to_string().contains("no files"), "{e}");

    let notarchive = tmp.path().join("notes.zip");
    fs::write(&notarchive, b"this is not really a zip").unwrap();
    let e = run(&client(), &job(tmp.path().join("GLM/N/1"), Source::Local { path: notarchive }, None), &Cancel::new(), report, &fast()).await.unwrap_err();
    assert!(e.to_string().contains("not a zip"), "{e}");
    assert!(!tmp.path().join("GLM/N/1").exists());
}

#[tokio::test]
async fn program_installers_are_refused_with_a_helpful_message() {
    let tmp = tempfile::tempdir().unwrap();
    let exe = tmp.path().join("SetupMod.exe");
    fs::write(&exe, b"MZ not a mod").unwrap();
    let (_r, report) = Recorder::new();
    let e = run(&client(), &job(tmp.path().join("GLM/X/1"), Source::Local { path: exe.clone() }, None), &Cancel::new(), report, &fast()).await.unwrap_err();
    assert!(e.to_string().contains("program installer") && e.to_string().contains("zip, 7z"), "{e}");
    assert!(exe.is_file(), "never delete what we refuse");
    assert!(!tmp.path().join("GLM/X/1").exists());
}
