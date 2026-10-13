mod common;

use common::{Handler, Resp, serve};
use genlauncher2_lib::download::{Cancel, Options, percent_decode};
use genlauncher2_lib::error::Error;
use genlauncher2_lib::install::{Job, Phase, Progress, Report, Source, run};
use std::path::PathBuf;
use md5::{Digest, Md5};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn md5_hex(b: &[u8]) -> String {
    hex::encode(Md5::digest(b))
}

fn fast() -> Options {
    Options { chunk_timeout: Duration::from_millis(500), attempts: 3, backoff: Duration::from_millis(5) }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

#[derive(Clone)]
struct Store {
    bucket: &'static str,
    folder: &'static str,
    files: Vec<(String, Vec<u8>)>,
    /// first N GETs of these files return corrupted bytes
    corrupt: Arc<Mutex<BTreeMap<String, usize>>>,
    hits: Arc<Mutex<Vec<String>>>,
}

impl Store {
    fn new(files: &[(&str, &[u8])]) -> Self {
        Store {
            bucket: "bkt",
            folder: "mod-files",
            files: files.iter().map(|(n, d)| (n.to_string(), d.to_vec())).collect(),
            corrupt: Default::default(),
            hits: Default::default(),
        }
    }
    fn handler(&self) -> Handler {
        let s = self.clone();
        Arc::new(move |req| {
            let path = req.target.split('?').next().unwrap();
            let query = req.target.split_once('?').map(|(_, q)| q).unwrap_or("");
            if query.contains("list-type=2") {
                // pages of 2 objects; the token is the index of the next one
                let start: usize = query.split('&').find_map(|p| p.strip_prefix("continuation-token=")).and_then(|t| t.parse().ok()).unwrap_or(0);
                let end = (start + 2).min(s.files.len());
                let mut xml = String::from(r#"<?xml version="1.0"?><ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">"#);
                xml.push_str(&format!("<IsTruncated>{}</IsTruncated>", end < s.files.len()));
                if end < s.files.len() {
                    xml.push_str(&format!("<NextContinuationToken>{end}</NextContinuationToken>"));
                }
                xml.push_str(&format!("<Contents><Key>{}/</Key><ETag>&#34;d41d8cd98f00b204e9800998ecf8427e&#34;</ETag><Size>0</Size></Contents>", s.folder));
                for (name, data) in &s.files[start..end] {
                    xml.push_str(&format!(
                        "<Contents><Key>{}/{}</Key><ETag>&#34;{}&#34;</ETag><Size>{}</Size></Contents>",
                        s.folder,
                        esc(name),
                        md5_hex(data),
                        data.len()
                    ));
                }
                xml.push_str("</ListBucketResult>");
                return Resp::ok(xml);
            }
            let prefix = format!("/{}/{}/", s.bucket, s.folder);
            let Some(rel) = path.strip_prefix(&prefix).map(percent_decode) else { return Resp::status(404) };
            s.hits.lock().unwrap().push(rel.clone());
            match s.files.iter().find(|(n, _)| *n == rel) {
                Some((_, data)) => {
                    let mut left = s.corrupt.lock().unwrap();
                    if let Some(n) = left.get_mut(&rel) {
                        if *n > 0 {
                            *n -= 1;
                            let mut bad = data.clone();
                            bad[0] ^= 0xFF;
                            return Resp::ok(bad);
                        }
                    }
                    Resp::ok(data.clone())
                }
                None => Resp::status(404),
            }
        })
    }
    fn source(&self, addr: std::net::SocketAddr) -> Source {
        let base = format!("http://{addr}");
        Source::S3 { list_base: base.clone(), file_base: base, bucket: self.bucket.into(), folder: self.folder.into(), creds: None }
    }
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
async fn fresh_s3_install_across_pages_is_verified_and_atomic() {
    let big: Vec<u8> = (0..200_000).map(|i| (i % 253) as u8).collect();
    let store = Store::new(&[("Art.gib", &big), ("Data/INI/Object & Co.ini", b"ini data"), ("Data/x y.tga", b"tga"), ("z.big", b"zz"), ("last.txt", b"end")]);
    let addr = serve(store.handler()).await;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join(".genlauncher/mods/Mod/1.0");
    let (rec, report) = Recorder::new();
    run(&client(), &job(dest.clone(), store.source(addr), None), &Cancel::new(), report, &fast()).await.unwrap();

    assert_eq!(read(&dest.join("Art.gib")), big);
    assert_eq!(read(&dest.join("Data/INI/Object & Co.ini")), b"ini data");
    assert_eq!(read(&dest.join("Data/x y.tga")), b"tga");
    assert_eq!(read(&dest.join("last.txt")), b"end");
    assert!(!staging_of(&dest).exists(), "temp folder removed on success");

    let log = rec.0.lock().unwrap();
    let last = log.last().unwrap();
    assert_eq!(last.phase, Phase::Finishing);
    assert_eq!(last.done, last.total);
    assert_eq!(last.total, (big.len() + 8 + 3 + 2 + 3) as u64);
    assert!(log.windows(2).all(|w| w[0].done <= w[1].done), "progress never goes backwards");
}

#[tokio::test]
async fn update_reuses_unchanged_files_from_the_previous_version() {
    let a: Vec<u8> = vec![1; 50_000];
    let b: Vec<u8> = vec![2; 50_000];
    let store = Store::new(&[("a.gib", &a), ("b.gib", &b), ("new.ini", b"brand new")]);
    let addr = serve(store.handler()).await;
    let tmp = tempfile::tempdir().unwrap();
    let prev = tmp.path().join(".genlauncher/mods/Mod/1.0");
    put(&prev.join("a.gib"), &a); // identical
    put(&prev.join("b.gib"), &vec![9; 50_000]); // same size, different content -> must be downloaded
    put(&prev.join("gone.ini"), b"removed in 1.1");
    let dest = tmp.path().join(".genlauncher/mods/Mod/1.1");
    let (_rec, report) = Recorder::new();
    run(&client(), &job(dest.clone(), store.source(addr), Some(prev.clone())), &Cancel::new(), report, &fast()).await.unwrap();

    let mut hits = store.hits.lock().unwrap().clone();
    hits.sort();
    assert_eq!(hits, ["b.gib", "new.ini"], "a.gib must not be downloaded again");
    assert_eq!(read(&dest.join("a.gib")), a);
    assert_eq!(read(&dest.join("b.gib")), b);
    assert!(!dest.join("gone.ini").exists(), "files removed upstream are not carried over");
    assert_eq!(read(&prev.join("a.gib")), a, "previous version untouched");
}

#[tokio::test]
async fn corrupted_download_is_retried_and_persistent_corruption_fails_cleanly() {
    let store = Store::new(&[("a.gib", &vec![5u8; 10_000])]);
    store.corrupt.lock().unwrap().insert("a.gib".into(), 1); // bad once, then fine
    let addr = serve(store.handler()).await;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join(".genlauncher/mods/M/1");
    let (_r, report) = Recorder::new();
    run(&client(), &job(dest.clone(), store.source(addr), None), &Cancel::new(), report.clone(), &fast()).await.unwrap();
    assert_eq!(read(&dest.join("a.gib")), vec![5u8; 10_000]);

    let bad = Store::new(&[("a.gib", &vec![5u8; 10_000])]);
    bad.corrupt.lock().unwrap().insert("a.gib".into(), 99);
    let addr = serve(bad.handler()).await;
    let dest2 = tmp.path().join(".genlauncher/mods/M/2");
    let e = run(&client(), &job(dest2.clone(), bad.source(addr), None), &Cancel::new(), report, &fast()).await.unwrap_err();
    assert!(e.to_string().contains("checksum"), "{e}");
    assert!(!dest2.exists(), "a failed install must not look installed");
}

#[tokio::test]
async fn interrupted_install_resumes_without_refetching_finished_files() {
    let store = Store::new(&[("done.gib", &vec![7u8; 30_000]), ("todo.gib", &vec![8u8; 30_000])]);
    let addr = serve(store.handler()).await;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join(".genlauncher/mods/M/1");
    // what an earlier, killed attempt left behind
    put(&staging_of(&dest).join("content/done.gib"), &vec![7u8; 30_000]);
    put(&staging_of(&dest).join("content/todo.gib.part"), &vec![8u8; 10_000]);
    let (_r, report) = Recorder::new();
    run(&client(), &job(dest.clone(), store.source(addr), None), &Cancel::new(), report, &fast()).await.unwrap();
    assert_eq!(store.hits.lock().unwrap().as_slice(), ["todo.gib"]);
    assert_eq!(read(&dest.join("todo.gib")), vec![8u8; 30_000]);
}

#[tokio::test]
async fn cancel_leaves_nothing_installed_but_keeps_progress_for_later() {
    let store = Store::new(&[("a.gib", &vec![1u8; 100_000]), ("b.gib", &vec![2u8; 100_000])]);
    let cancel = Cancel::new();
    let (c2, inner) = (cancel.clone(), store.handler());
    // the user presses Cancel at the moment the first file is requested
    let addr = serve(Arc::new(move |req| {
        if !req.target.contains("list-type") {
            c2.cancel();
        }
        inner(req)
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join(".genlauncher/mods/M/1");
    let (_r, report) = Recorder::new();
    let e = run(&client(), &job(dest.clone(), store.source(addr), None), &cancel, report, &fast()).await.unwrap_err();
    assert!(matches!(e, Error::Cancelled), "{e}");
    assert!(!dest.exists());
    assert!(staging_of(&dest).exists(), "temp folder kept so the next attempt can resume");
}

#[tokio::test]
async fn failed_update_leaves_the_installed_version_alone() {
    let store = Store::new(&[("a.gib", b"x")]); // listing says a.gib exists...
    let mut broken = store.clone();
    broken.files = vec![("a.gib".into(), b"x".to_vec())];
    let h = broken.handler();
    // ...but every file request fails
    let handler: Handler = Arc::new(move |req| if req.target.contains("list-type") { h(req) } else { Resp::status(404) });
    let addr = serve(handler).await;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join(".genlauncher/mods/M/1");
    put(&dest.join("keep.gib"), b"working install");
    let (_r, report) = Recorder::new();
    let e = run(&client(), &job(dest.clone(), store.source(addr), None), &Cancel::new(), report, &fast()).await.unwrap_err();
    assert!(e.to_string().contains("404"), "{e}");
    assert_eq!(read(&dest.join("keep.gib")), b"working install");
}

#[tokio::test]
async fn reinstall_replaces_the_old_folder_only_after_success() {
    let store = Store::new(&[("new.gib", b"new content")]);
    let addr = serve(store.handler()).await;
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join(".genlauncher/mods/M/1");
    put(&dest.join("old.gib"), b"old");
    let (_r, report) = Recorder::new();
    run(&client(), &job(dest.clone(), store.source(addr), None), &Cancel::new(), report, &fast()).await.unwrap();
    assert_eq!(read(&dest.join("new.gib")), b"new content");
    assert!(!dest.join("old.gib").exists());
    let siblings: Vec<_> = fs::read_dir(dest.parent().unwrap()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(siblings, ["1"], "no leftovers: {siblings:?}");
}

#[tokio::test]
async fn empty_listing_and_hostile_names_are_rejected() {
    let empty = Store::new(&[]);
    let addr = serve(empty.handler()).await;
    let tmp = tempfile::tempdir().unwrap();
    let (_r, report) = Recorder::new();
    let e = run(&client(), &job(tmp.path().join("a"), empty.source(addr), None), &Cancel::new(), report.clone(), &fast()).await.unwrap_err();
    assert!(e.to_string().contains("no files"), "{e}");

    let evil = Store::new(&[("../../evil.txt", b"x")]);
    let addr = serve(evil.handler()).await;
    let e = run(&client(), &job(tmp.path().join("b"), evil.source(addr), None), &Cancel::new(), report, &fast()).await.unwrap_err();
    assert!(e.to_string().contains("unsafe"), "{e}");
    assert!(!tmp.path().join("evil.txt").exists());
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

#[tokio::test]
async fn private_storage_needs_the_keys_the_mod_provides() {
    use genlauncher2_lib::sigv4::Creds;
    let store = Store::new(&[("a.gib", b"private content")]);
    let inner = store.handler();
    // listing only works with a SigV4 Authorization header; file reads stay public
    let addr = serve(Arc::new(move |req| {
        let listing = req.target.contains("list-type");
        let signed = req.headers.get("authorization").is_some_and(|a| a.starts_with("AWS4-HMAC-SHA256 Credential=AKIAKEY/") && a.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"))
            && req.headers.contains_key("x-amz-date")
            && req.headers.get("x-amz-content-sha256").is_some();
        if listing && !signed { Resp::status(403) } else { inner(req) }
    }))
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let (_r, report) = Recorder::new();
    let base = format!("http://{addr}");
    let source = |creds| Source::S3 { list_base: base.clone(), file_base: base.clone(), bucket: "bkt".into(), folder: "mod-files".into(), creds };

    let e = run(&client(), &job(tmp.path().join("a"), source(None), None), &Cancel::new(), report.clone(), &fast()).await.unwrap_err();
    assert!(e.to_string().contains("403") && e.to_string().contains("no access key"), "{e}");

    let keys = Some(Creds { access_key: "AKIAKEY".into(), secret_key: "secret".into() });
    run(&client(), &job(tmp.path().join("b"), source(keys), None), &Cancel::new(), report, &fast()).await.unwrap();
    assert_eq!(read(&tmp.path().join("b/a.gib")), b"private content");
}
