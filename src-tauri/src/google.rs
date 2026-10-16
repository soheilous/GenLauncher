//! Signing in with Google and keeping configuration backups in the user's Google Drive.
//!
//! The backups go to Drive's *app data folder*: a hidden area that only this app can read and that does not show up in
//! the user's Drive. The sign-in is the standard desktop flow: the system browser (Google does not allow embedded
//! windows), a one-time loopback address on this computer to receive the answer, and PKCE.
//!
//! The OAuth client id is public. Google also hands desktop apps a "client secret" that its token endpoint insists on
//! but that is not confidential; it is not stored in this repository: it is read from `GENLAUNCHER_GOOGLE_CLIENT_SECRET`
//! (when running, or when the release is built).

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::backups;
use crate::error::{Error, Result};

pub const CLIENT_ID: &str = "1067851772029-qcki52563f45r087qmb8a37bu6b6k7oo.apps.googleusercontent.com";
/// Who is signed in (`email`) and the hidden app folder in Drive. Nothing else.
pub const SCOPES: &str = "openid email https://www.googleapis.com/auth/drive.appdata";
const SECRET_VAR: &str = "GENLAUNCHER_GOOGLE_CLIENT_SECRET";

pub fn client_secret() -> Option<String> {
    std::env::var(SECRET_VAR).ok().or_else(|| option_env!("GENLAUNCHER_GOOGLE_CLIENT_SECRET").map(str::to_string)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Where the requests go (replaced by a local server in tests).
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub auth: String,
    pub token: String,
    pub revoke: String,
    pub api: String,
    pub upload: String,
}

impl Endpoints {
    pub fn google() -> Self {
        Endpoints {
            auth: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            revoke: "https://oauth2.googleapis.com/revoke".into(),
            api: "https://www.googleapis.com/drive/v3".into(),
            upload: "https://www.googleapis.com/upload/drive/v3".into(),
        }
    }
}

/// What is kept on this computer between runs: enough to get new access tokens, and whom they belong to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Session {
    pub refresh_token: String,
    pub email: String,
}

// ---------- PKCE and the sign-in round trip ----------

pub fn random_string(len: usize) -> Result<String> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut bytes = vec![0u8; len];
    getrandom::fill(&mut bytes).map_err(|e| Error::msg(format!("no source of randomness: {e}")))?;
    Ok(bytes.iter().map(|b| CHARS[*b as usize % CHARS.len()] as char).collect())
}

/// RFC 7636: `BASE64URL(SHA256(verifier))`.
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn auth_url(ep: &Endpoints, redirect: &str, state: &str, challenge: &str) -> Result<String> {
    let url = reqwest::Url::parse_with_params(
        &ep.auth,
        [
            ("client_id", CLIENT_ID),
            ("redirect_uri", redirect),
            ("response_type", "code"),
            ("scope", SCOPES),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
            ("access_type", "offline"),
            // always ask, so Google always returns a refresh token
            ("prompt", "consent"),
        ],
    )
    .map_err(|e| Error::msg(format!("bad sign-in address: {e}")))?;
    Ok(url.into())
}

const PAGE: &str = "<!doctype html><html><head><meta charset=utf-8><title>GenLauncher</title><style>body{font-family:system-ui,sans-serif;display:grid;place-items:center;height:100vh;margin:0;background:#15131c;color:#eee}div{text-align:center;max-width:28rem;padding:2rem}</style></head><body><div><h2>{TITLE}</h2><p>{TEXT}</p></div></body></html>";

fn page(title: &str, text: &str) -> String {
    PAGE.replace("{TITLE}", title).replace("{TEXT}", text)
}

async fn reply(sock: &mut tokio::net::TcpStream, status: &str, body: &str) {
    let msg = format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = sock.write_all(msg.as_bytes()).await;
    let _ = sock.flush().await;
}

/// Wait for the browser to come back to the loopback address with the answer. Requests that are not the answer
/// (a favicon, something else on this computer poking the port, a wrong `state`) are turned away and ignored.
pub async fn wait_for_redirect(listener: TcpListener, expected_state: &str, timeout: Duration) -> Result<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let (mut sock, _) = tokio::time::timeout_at(deadline, listener.accept())
            .await
            .map_err(|_| Error::msg("Nothing came back from the browser. Try signing in again."))?
            .map_err(|e| Error::msg(format!("could not receive the sign-in answer: {e}")))?;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 2048];
        let read = tokio::time::timeout(Duration::from_secs(5), async {
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
                match sock.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
            }
        })
        .await;
        if read.is_err() {
            continue;
        }
        let text = String::from_utf8_lossy(&buf).into_owned();
        let target = text.lines().next().unwrap_or("").split(' ').nth(1).unwrap_or("/").to_string();
        let Ok(url) = reqwest::Url::parse(&format!("http://127.0.0.1{target}")) else {
            reply(&mut sock, "400 Bad Request", &page("Not understood", "")).await;
            continue;
        };
        if url.path() != "/" {
            reply(&mut sock, "404 Not Found", &page("Not found", "")).await;
            continue;
        }
        let get = |k: &str| url.query_pairs().find(|(name, _)| name == k).map(|(_, v)| v.into_owned());
        if get("state").as_deref() != Some(expected_state) {
            reply(&mut sock, "400 Bad Request", &page("Not for this sign-in", "Go back to GenLauncher and start the sign-in again.")).await;
            continue;
        }
        if let Some(err) = get("error") {
            reply(&mut sock, "200 OK", &page("Sign-in cancelled", "You can close this tab and go back to GenLauncher.")).await;
            return Err(Error::msg(if err == "access_denied" { "Google sign-in was cancelled.".to_string() } else { format!("Google refused the sign-in ({err}).") }));
        }
        let Some(code) = get("code") else {
            reply(&mut sock, "400 Bad Request", &page("No code", "")).await;
            continue;
        };
        reply(&mut sock, "200 OK", &page("You are signed in", "You can close this tab and go back to GenLauncher.")).await;
        return Ok(code);
    }
}

// ---------- tokens ----------

#[derive(Debug, Clone, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: u64,
    #[serde(default)]
    pub id_token: Option<String>,
    /// What the user actually allowed (space separated). Google lets people untick a permission on the consent screen.
    #[serde(default)]
    pub scope: Option<String>,
}

impl Tokens {
    /// False when Google says the user did not grant access to the Drive app folder.
    pub fn drive_granted(&self) -> bool {
        self.scope.as_deref().map_or(true, |s| s.split(' ').any(|p| p.ends_with("/auth/drive.appdata")))
    }
}

fn form_body(pairs: &[(&str, &str)]) -> String {
    let mut u = reqwest::Url::parse("http://x/").expect("static url");
    u.query_pairs_mut().extend_pairs(pairs);
    u.query().unwrap_or("").to_string()
}

/// The client secret, or a clear message when this build has none.
pub fn require_client_secret() -> Result<String> {
    client_secret().ok_or_else(|| Error::msg(format!("Google sign-in is not set up in this build ({SECRET_VAR} is missing).")))
}

async fn token_request(http: &reqwest::Client, ep: &Endpoints, secret: &str, pairs: &[(&str, &str)]) -> Result<Tokens> {
    let mut all = vec![("client_id", CLIENT_ID), ("client_secret", secret)];
    all.extend_from_slice(pairs);
    let resp = http
        .post(&ep.token)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form_body(&all))
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let code = v["error"].as_str().unwrap_or("");
        if code == "invalid_grant" {
            return Err(Error::SignedOut);
        }
        let why = v["error_description"].as_str().unwrap_or(code);
        return Err(Error::msg(format!("Google said no ({status}): {why}")));
    }
    serde_json::from_str(&text).map_err(|_| Error::msg("Google's answer was not understood"))
}

pub async fn exchange_code(http: &reqwest::Client, ep: &Endpoints, secret: &str, code: &str, verifier: &str, redirect: &str) -> Result<Tokens> {
    token_request(http, ep, secret, &[("code", code), ("code_verifier", verifier), ("redirect_uri", redirect), ("grant_type", "authorization_code")]).await
}

pub async fn refresh(http: &reqwest::Client, ep: &Endpoints, secret: &str, refresh_token: &str) -> Result<Tokens> {
    token_request(http, ep, secret, &[("refresh_token", refresh_token), ("grant_type", "refresh_token")]).await
}

/// Tell Google to forget the grant. Best effort: signing out locally must work offline.
pub async fn revoke(http: &reqwest::Client, ep: &Endpoints, token: &str) {
    let _ = http.post(&ep.revoke).header("content-type", "application/x-www-form-urlencoded").body(form_body(&[("token", token)])).send().await;
}

/// The address in the `id_token` Google returned over TLS (so it is not re-verified here; it is only displayed).
pub fn email_from_id_token(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let v: serde_json::Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    v["email"].as_str().map(str::to_string)
}

// ---------- Drive app data folder ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudBackup {
    /// Drive's id for the file (the handle for restore and delete).
    pub id: String,
    pub name: String,
    pub at: u64,
    pub label: String,
    pub bytes: u64,
}

async fn drive_check(resp: reqwest::Response) -> Result<String> {
    let status = resp.status();
    let text = resp.text().await?;
    if status.is_success() {
        return Ok(text);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::SignedOut);
    }
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let why = v["error"]["message"].as_str().unwrap_or("").to_string();
    if status == reqwest::StatusCode::FORBIDDEN && why.contains("not been used") {
        return Err(Error::msg("The Google Drive API is switched off for this app's Google Cloud project."));
    }
    Err(Error::msg(format!("Google Drive said no ({status}) {why}").trim().to_string()))
}

/// Our backups in the app folder, newest first.
pub async fn list(http: &reqwest::Client, ep: &Endpoints, token: &str) -> Result<Vec<CloudBackup>> {
    let url = reqwest::Url::parse_with_params(
        &format!("{}/files", ep.api),
        [("spaces", "appDataFolder"), ("pageSize", "1000"), ("fields", "files(id,name,size)")],
    )
    .map_err(|e| Error::msg(e.to_string()))?;
    let v: serde_json::Value = serde_json::from_str(&drive_check(http.get(url).bearer_auth(token).send().await?).await?).unwrap_or_default();
    let mut out: Vec<CloudBackup> = v["files"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    let name = f["name"].as_str()?.to_string();
                    let (at, label) = backups::parse_name(&name)?;
                    Some(CloudBackup { id: f["id"].as_str()?.to_string(), name, at, label, bytes: f["size"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0) })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.name.cmp(&a.name)));
    Ok(out)
}

pub async fn upload(http: &reqwest::Client, ep: &Endpoints, token: &str, name: &str, content: &str) -> Result<CloudBackup> {
    let boundary = format!("genlauncher{}", random_string(16)?);
    let meta = serde_json::json!({ "name": name, "parents": ["appDataFolder"], "mimeType": "application/json" });
    let body = format!(
        "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n--{boundary}\r\nContent-Type: application/json\r\n\r\n{content}\r\n--{boundary}--"
    );
    let url = format!("{}/files?uploadType=multipart&fields=id,name,size", ep.upload);
    let resp = http
        .post(url)
        .bearer_auth(token)
        .header("content-type", format!("multipart/related; boundary={boundary}"))
        .body(body)
        .send()
        .await?;
    let v: serde_json::Value = serde_json::from_str(&drive_check(resp).await?).unwrap_or_default();
    let (at, label) = backups::parse_name(name).ok_or_else(|| Error::msg("bad backup name"))?;
    Ok(CloudBackup {
        id: v["id"].as_str().ok_or_else(|| Error::msg("Google Drive did not say where it saved the backup"))?.to_string(),
        name: name.to_string(),
        at,
        label,
        bytes: v["size"].as_str().and_then(|s| s.parse().ok()).unwrap_or(content.len() as u64),
    })
}

fn safe_id(id: &str) -> Result<&str> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err(Error::msg("That is not a backup"));
    }
    Ok(id)
}

pub async fn download(http: &reqwest::Client, ep: &Endpoints, token: &str, id: &str) -> Result<String> {
    let url = format!("{}/files/{}?alt=media", ep.api, safe_id(id)?);
    drive_check(http.get(url).bearer_auth(token).send().await?).await
}

pub async fn delete(http: &reqwest::Client, ep: &Endpoints, token: &str, id: &str) -> Result<()> {
    let url = format!("{}/files/{}", ep.api, safe_id(id)?);
    drive_check(http.delete(url).bearer_auth(token).send().await?).await.map(|_| ())
}

/// Delete the oldest backups beyond `keep`.
pub async fn prune(http: &reqwest::Client, ep: &Endpoints, token: &str, keep: usize) -> Result<()> {
    for old in list(http, ep, token).await?.into_iter().skip(keep) {
        delete(http, ep, token, &old.id).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_the_rfc_7636_example() {
        assert_eq!(pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn random_strings_use_the_allowed_characters_and_differ() {
        let a = random_string(64).unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b)));
        assert_ne!(a, random_string(64).unwrap());
    }

    #[test]
    fn the_sign_in_address_asks_for_the_least() {
        let u = reqwest::Url::parse(&auth_url(&Endpoints::google(), "http://127.0.0.1:5555", "st", "ch").unwrap()).unwrap();
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], CLIENT_ID);
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:5555");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["scope"], "openid email https://www.googleapis.com/auth/drive.appdata");
        assert!(u.as_str().starts_with("https://accounts.google.com/"));
    }

    #[test]
    fn email_is_read_from_the_id_token() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"email":"a@b.c","sub":"1"}"#);
        assert_eq!(email_from_id_token(&format!("h.{payload}.s")).as_deref(), Some("a@b.c"));
        assert_eq!(email_from_id_token("garbage"), None);
    }

    #[test]
    fn form_bodies_are_escaped() {
        assert_eq!(form_body(&[("a", "b c"), ("d", "e&f=g")]), "a=b+c&d=e%26f%3Dg");
    }

    #[test]
    fn drive_ids_cannot_smuggle_paths() {
        assert!(safe_id("1AbC-d_9").is_ok());
        for bad in ["", "../x", "a/b", "a?b", "a b", "a%2Fb"] {
            assert!(safe_id(bad).is_err(), "{bad}");
        }
    }
}
