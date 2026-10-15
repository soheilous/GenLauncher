//! The Google sign-in and Drive app-data flow against a local stand-in for Google's servers.

mod common;

use common::{Resp, serve};
use genlauncher2_lib::error::Error;
use genlauncher2_lib::google::{self, Endpoints};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn endpoints(addr: std::net::SocketAddr) -> Endpoints {
    let base = format!("http://{addr}");
    Endpoints { auth: format!("{base}/auth"), token: format!("{base}/token"), revoke: format!("{base}/revoke"), api: format!("{base}/drive/v3"), upload: format!("{base}/upload/drive/v3") }
}

/// Send one GET to `addr` and return the whole answer.
async fn get(addr: std::net::SocketAddr, target: &str) -> String {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(format!("GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    out
}

#[tokio::test]
async fn the_browser_answer_is_accepted_only_with_the_right_state() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let waiter = tokio::spawn(google::wait_for_redirect(listener, "good-state", Duration::from_secs(10)));

    assert!(get(addr, "/favicon.ico").await.starts_with("HTTP/1.1 404"));
    assert!(get(addr, "/?code=evil&state=wrong").await.starts_with("HTTP/1.1 400"), "a wrong state must be turned away");
    assert!(get(addr, "/?state=good-state").await.starts_with("HTTP/1.1 400"), "no code is not an answer");
    assert!(!waiter.is_finished(), "none of that may end the wait");
    let ok = get(addr, "/?state=good-state&code=4%2Fabc").await;
    assert!(ok.starts_with("HTTP/1.1 200") && ok.contains("signed in"));
    assert_eq!(waiter.await.unwrap().unwrap(), "4/abc");
}

#[tokio::test]
async fn a_refused_sign_in_and_a_silent_browser_are_reported() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let waiter = tokio::spawn(google::wait_for_redirect(listener, "s", Duration::from_secs(10)));
    get(addr, "/?state=s&error=access_denied").await;
    assert!(waiter.await.unwrap().unwrap_err().to_string().contains("cancelled"));

    let quiet = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let e = google::wait_for_redirect(quiet, "s", Duration::from_millis(150)).await.unwrap_err();
    assert!(e.to_string().contains("Nothing came back"), "{e}");
}

#[tokio::test]
async fn tokens_are_exchanged_and_refreshed_with_pkce_and_the_secret() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let s2 = seen.clone();
    let addr = serve(Arc::new(move |req| {
        assert_eq!(req.method, "POST");
        s2.lock().unwrap().push(req.body.clone());
        if req.body.contains("grant_type=refresh_token") && req.body.contains("refresh_token=revoked") {
            return Resp::status(400).body_text(r#"{"error":"invalid_grant"}"#);
        }
        Resp::ok(r#"{"access_token":"AT","refresh_token":"RT","expires_in":3599,"id_token":"x"}"#)
    }))
    .await;
    let ep = endpoints(addr);
    let http = reqwest::Client::new();

    let t = google::exchange_code(&http, &ep, "the-secret", "code/1", "verifier123", "http://127.0.0.1:9").await.unwrap();
    assert_eq!((t.access_token.as_str(), t.refresh_token.as_deref(), t.expires_in), ("AT", Some("RT"), 3599));
    let body = seen.lock().unwrap()[0].clone();
    for part in ["client_id=1067851772029-", "client_secret=the-secret", "code=code%2F1", "code_verifier=verifier123", "grant_type=authorization_code", "redirect_uri=http%3A%2F%2F127.0.0.1%3A9"] {
        assert!(body.contains(part), "{part} missing from {body}");
    }

    assert_eq!(google::refresh(&http, &ep, "the-secret", "RT").await.unwrap().access_token, "AT");
    assert!(matches!(google::refresh(&http, &ep, "the-secret", "revoked").await, Err(Error::SignedOut)));
}

#[tokio::test]
async fn backups_are_listed_uploaded_downloaded_and_deleted_in_the_app_folder() {
    let log = Arc::new(Mutex::new(Vec::<String>::new()));
    let l2 = log.clone();
    let addr = serve(Arc::new(move |req| {
        l2.lock().unwrap().push(format!("{} {}", req.method, req.target));
        if req.headers.get("authorization").map(String::as_str) != Some("Bearer TOKEN") {
            return Resp::status(401);
        }
        match (req.method.as_str(), req.target.as_str()) {
            ("GET", t) if t.starts_with("/drive/v3/files?") => Resp::ok(
                r#"{"files":[
                    {"id":"id-old","name":"config-20260101-000000-manual.json","size":"120"},
                    {"id":"id-new","name":"config-20261015-143000-before-import.json","size":"300"},
                    {"id":"id-other","name":"notes.txt","size":"5"}]}"#,
            ),
            ("POST", t) if t.starts_with("/upload/drive/v3/files?uploadType=multipart") => {
                assert!(req.headers["content-type"].starts_with("multipart/related; boundary="));
                assert!(req.body.contains(r#""parents":["appDataFolder"]"#) && req.body.contains(r#""name":"config-20261015-150000-manual.json""#), "{}", req.body);
                assert!(req.body.contains("{\"app\":\"GenLauncher\"}"));
                Resp::ok(r#"{"id":"id-created","name":"config-20261015-150000-manual.json","size":"22"}"#)
            }
            ("GET", "/drive/v3/files/id-new?alt=media") => Resp::ok(r#"{"app":"GenLauncher","format":1}"#),
            ("DELETE", "/drive/v3/files/id-old") => Resp::status(200),
            _ => Resp::status(404),
        }
    }))
    .await;
    let ep = endpoints(addr);
    let http = reqwest::Client::new();

    let list = google::list(&http, &ep, "TOKEN").await.unwrap();
    assert_eq!(list.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["id-new", "id-old"], "newest first, foreign files ignored");
    assert_eq!(list[0].label, "before-import");

    let made = google::upload(&http, &ep, "TOKEN", "config-20261015-150000-manual.json", "{\"app\":\"GenLauncher\"}").await.unwrap();
    assert_eq!((made.id.as_str(), made.label.as_str()), ("id-created", "manual"));

    assert!(google::download(&http, &ep, "TOKEN", "id-new").await.unwrap().contains("GenLauncher"));
    google::delete(&http, &ep, "TOKEN", "id-old").await.unwrap();
    assert!(google::download(&http, &ep, "TOKEN", "../etc").await.is_err(), "ids are checked before any request is made");

    // pruning keeps the newest one and removes the other
    google::prune(&http, &ep, "TOKEN", 1).await.unwrap();
    assert!(log.lock().unwrap().iter().any(|l| l == "DELETE /drive/v3/files/id-old"));
    assert!(!log.lock().unwrap().iter().any(|l| l == "DELETE /drive/v3/files/id-new"));

    // an expired token is reported as such, not as a generic failure
    assert!(matches!(google::list(&http, &ep, "STALE").await, Err(Error::SignedOut)));
}
