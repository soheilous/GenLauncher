mod common;

use common::{Resp, range_start, serve};
use genlauncher2_lib::download::{Cancel, Options, fetch, md5_file};
use genlauncher2_lib::error::Error;
use md5::{Digest, Md5};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::time::Duration;

fn payload(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 % 251) as u8).collect()
}

fn md5_hex(b: &[u8]) -> String {
    hex::encode(Md5::digest(b))
}

fn fast() -> Options {
    Options { chunk_timeout: Duration::from_millis(500), attempts: 4, backoff: Duration::from_millis(10) }
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

#[tokio::test]
async fn downloads_whole_file_with_name_and_md5() {
    let data = payload(300_000);
    let body = data.clone();
    let addr = serve(Arc::new(move |_| Resp::ok(body.clone()).header("Content-Disposition", r#"attachment; filename="My Mod.7z""#))).await;
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("x.part");
    let seen = Arc::new(AtomicI64::new(0));
    let s2 = seen.clone();
    let got = fetch(&client(), &format!("http://{addr}/file"), &part, &Cancel::new(), &move |d| { s2.fetch_add(d, Ordering::SeqCst); }, &|_| {}, true, &fast())
        .await
        .unwrap();
    assert_eq!(got.file_name, "My Mod.7z");
    assert_eq!(got.size, 300_000);
    assert_eq!(got.md5.as_deref(), Some(md5_hex(&data).as_str()));
    assert_eq!(std::fs::read(&part).unwrap(), data);
    assert_eq!(seen.load(Ordering::SeqCst), 300_000);
}

#[tokio::test]
async fn resumes_after_the_connection_drops() {
    let data = payload(500_000);
    let body = data.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let ranges = Arc::new(std::sync::Mutex::new(Vec::<Option<usize>>::new()));
    let r2 = ranges.clone();
    let addr = serve(Arc::new(move |req| {
        let n = c.fetch_add(1, Ordering::SeqCst);
        let start = range_start(req);
        r2.lock().unwrap().push(start);
        let resp = match start {
            Some(s) => Resp { status: 206, ..Resp::ok(body[s..].to_vec()) }
                .header("Content-Range", format!("bytes {s}-{}/{}", body.len() - 1, body.len())),
            None => Resp::ok(body.clone()),
        };
        if n == 0 { resp.cut(180_000) } else { resp } // first try dies part-way
    }))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("a.part");
    let seen = Arc::new(AtomicI64::new(0));
    let s2 = seen.clone();
    let got = fetch(&client(), &format!("http://{addr}/f.zip"), &part, &Cancel::new(), &move |d| { s2.fetch_add(d, Ordering::SeqCst); }, &|_| {}, true, &fast())
        .await
        .unwrap();
    assert_eq!(got.size, 500_000);
    assert_eq!(got.md5.as_deref(), Some(md5_hex(&data).as_str()), "hash must cover the resumed part too");
    assert_eq!(std::fs::read(&part).unwrap(), data);
    assert_eq!(seen.load(Ordering::SeqCst), 500_000, "progress must not double count the resumed bytes");
    let r = ranges.lock().unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0], None);
    assert!(r[1].unwrap() > 0, "second request resumed with a Range header");
}

#[tokio::test]
async fn a_leftover_part_file_from_an_earlier_run_is_continued() {
    let data = payload(100_000);
    let body = data.clone();
    let addr = serve(Arc::new(move |req| match range_start(req) {
        Some(s) => Resp { status: 206, ..Resp::ok(body[s..].to_vec()) }.header("Content-Range", format!("bytes {s}-{}/{}", body.len() - 1, body.len())),
        None => Resp::ok(body.clone()),
    }))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("b.part");
    std::fs::write(&part, &data[..40_000]).unwrap();
    let got = fetch(&client(), &format!("http://{addr}/f"), &part, &Cancel::new(), &|_| {}, &|_| {}, true, &fast()).await.unwrap();
    assert_eq!(got.md5.as_deref(), Some(md5_hex(&data).as_str()));
    assert_eq!(md5_file(&part).unwrap(), md5_hex(&data));
}

#[tokio::test]
async fn server_that_ignores_range_restarts_cleanly() {
    let data = payload(80_000);
    let body = data.clone();
    let addr = serve(Arc::new(move |_| Resp::ok(body.clone()))).await; // always 200, full body
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("c.part");
    std::fs::write(&part, b"stale garbage that must not survive").unwrap();
    let seen = Arc::new(AtomicI64::new(0));
    let s2 = seen.clone();
    let got = fetch(&client(), &format!("http://{addr}/f"), &part, &Cancel::new(), &move |d| { s2.fetch_add(d, Ordering::SeqCst); }, &|_| {}, true, &fast())
        .await
        .unwrap();
    assert_eq!(std::fs::read(&part).unwrap(), data);
    assert_eq!(got.md5.as_deref(), Some(md5_hex(&data).as_str()));
    assert_eq!(seen.load(Ordering::SeqCst), 80_000);
}

#[tokio::test]
async fn html_page_and_http_errors_are_not_retried() {
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    let addr = serve(Arc::new(move |req| {
        h.fetch_add(1, Ordering::SeqCst);
        if req.target == "/page" { Resp::ok("<html>log in</html>").header("Content-Type", "text/html; charset=utf-8") } else { Resp::status(404) }
    }))
    .await;
    let dir = tempfile::tempdir().unwrap();
    let e = fetch(&client(), &format!("http://{addr}/page"), &dir.path().join("p.part"), &Cancel::new(), &|_| {}, &|_| {}, false, &fast()).await.unwrap_err();
    assert!(e.to_string().contains("web page"), "{e}");
    let e = fetch(&client(), &format!("http://{addr}/missing"), &dir.path().join("q.part"), &Cancel::new(), &|_| {}, &|_| {}, false, &fast()).await.unwrap_err();
    assert!(e.to_string().contains("404"), "{e}");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "no retries for permanent errors");
}

#[tokio::test]
async fn server_errors_are_retried_then_succeed() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let addr = serve(Arc::new(move |_| if c.fetch_add(1, Ordering::SeqCst) < 2 { Resp::status(500) } else { Resp::ok("fine") })).await;
    let dir = tempfile::tempdir().unwrap();
    let got = fetch(&client(), &format!("http://{addr}/f"), &dir.path().join("r.part"), &Cancel::new(), &|_| {}, &|_| {}, false, &fast()).await.unwrap();
    assert_eq!(got.size, 4);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn cancel_stops_the_download() {
    let addr = serve(Arc::new(|_| Resp::ok(payload(200_000)))).await;
    let dir = tempfile::tempdir().unwrap();
    let cancel = Cancel::new();
    let c2 = cancel.clone();
    let e = fetch(&client(), &format!("http://{addr}/f"), &dir.path().join("s.part"), &cancel, &move |_| c2.cancel(), &|_| {}, false, &fast()).await.unwrap_err();
    assert!(matches!(e, Error::Cancelled), "{e}");
}
