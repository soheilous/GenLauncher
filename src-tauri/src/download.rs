//! Resumable HTTP file download.
//!
//! * Data goes to a `.part` file; a retry (or a restart of the launcher) continues it with an
//!   HTTP `Range` request instead of starting over.
//! * A stalled connection (no data for `chunk_timeout`) counts as a failure and is retried with
//!   a growing delay.
//! * MD5 is computed while streaming, so verifying costs no second pass.

use futures::StreamExt;
use md5::{Digest, Md5};
use reqwest::{StatusCode, header};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::error::{Error, Result};

#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() { Err(Error::Cancelled) } else { Ok(()) }
    }
}

#[derive(Clone)]
pub struct Options {
    /// No bytes for this long = stalled.
    pub chunk_timeout: Duration,
    pub attempts: u32,
    /// Delay before retry n is `backoff * n`.
    pub backoff: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self { chunk_timeout: Duration::from_secs(30), attempts: 5, backoff: Duration::from_secs(2) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// Name suggested by the server (`Content-Disposition`) or the final URL.
    pub file_name: String,
    pub size: u64,
    pub md5: Option<String>,
}

/// Progress callback: change in downloaded bytes (negative when a restart discards a partial file).
pub type OnBytes<'a> = &'a (dyn Fn(i64) + Send + Sync);
/// Called when the full size of the file is known (before bytes flow).
pub type OnTotal<'a> = &'a (dyn Fn(u64) + Send + Sync);

fn classify(e: reqwest::Error) -> Error {
    match e.status() {
        Some(s) if s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS || s == StatusCode::REQUEST_TIMEOUT => {
            Error::Transient(e.to_string())
        }
        Some(_) => Error::Net(e),
        None => Error::Transient(e.to_string()),
    }
}

fn is_retryable(e: &Error) -> bool {
    matches!(e, Error::Transient(_) | Error::Io(_))
}

pub fn file_name_from_disposition(value: &str) -> Option<String> {
    // RFC 5987 form first: filename*=UTF-8''My%20Mod.zip
    for part in value.split(';').map(str::trim) {
        if let Some(rest) = part.strip_prefix("filename*=") {
            let enc = rest.rsplit("''").next().unwrap_or(rest);
            return Some(percent_decode(enc.trim_matches('"')));
        }
    }
    for part in value.split(';').map(str::trim) {
        if let Some(rest) = part.strip_prefix("filename=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(Ok(v)) = s.get(i + 1..i + 3).map(|h| u8::from_str_radix(h, 16)) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Keep only a plain file name (no folders, no characters Windows rejects).
pub fn sanitize_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().map(|c| if matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || c.is_control() { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() { "download".into() } else { cleaned }
}

fn md5_of_prefix(path: &Path, hasher: &mut Md5) -> Result<()> {
    let mut f = fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        hasher.update(&buf[..n]);
    }
}

pub fn md5_file(path: &Path) -> Result<String> {
    let mut h = Md5::new();
    md5_of_prefix(path, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

fn content_range_total(v: &str) -> Option<u64> {
    v.rsplit('/').next()?.trim().parse().ok()
}

/// Download `url` into `part` (resuming it if it exists). The caller renames `part` afterwards.
pub async fn fetch(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    cancel: &Cancel,
    on_bytes: OnBytes<'_>,
    on_total: OnTotal<'_>,
    want_md5: bool,
    opts: &Options,
) -> Result<Fetched> {
    let mut reported: i64 = 0;
    let mut attempt = 0;
    loop {
        attempt += 1;
        cancel.check()?;
        match attempt_once(client, url, part, cancel, on_bytes, on_total, want_md5, opts, &mut reported).await {
            Ok(f) => return Ok(f),
            Err(e) if is_retryable(&e) && attempt < opts.attempts => {
                log::warn!("download of {url} interrupted ({e}); retry {attempt}/{}", opts.attempts);
                tokio::time::sleep(opts.backoff * attempt).await;
            }
            Err(e) => return Err(e),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn attempt_once(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    cancel: &Cancel,
    on_bytes: OnBytes<'_>,
    on_total: OnTotal<'_>,
    want_md5: bool,
    opts: &Options,
    reported: &mut i64,
) -> Result<Fetched> {
    if let Some(parent) = part.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let mut req = client.get(url);
    if existing > 0 {
        req = req.header(header::RANGE, format!("bytes={existing}-"));
    }
    let resp = req.send().await.map_err(classify)?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let final_url = resp.url().clone();

    let name = headers
        .get(header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(file_name_from_disposition)
        .or_else(|| final_url.path_segments().and_then(|s| s.last()).filter(|s| !s.is_empty()).map(percent_decode))
        .map(|n| sanitize_file_name(&n))
        .unwrap_or_else(|| "download".into());

    // 416: our partial file does not fit what the server has. Complete already, or stale.
    if status == StatusCode::RANGE_NOT_SATISFIABLE {
        let total = headers.get(header::CONTENT_RANGE).and_then(|v| v.to_str().ok()).and_then(content_range_total);
        if total == Some(existing) {
            let md5 = if want_md5 { Some(md5_file(part)?) } else { None };
            return Ok(Fetched { file_name: name, size: existing, md5 });
        }
        fs::remove_file(part).ok();
        on_bytes(-*reported);
        *reported = 0;
        return Err(Error::Transient("partial file did not match the server; starting over".into()));
    }
    if !status.is_success() {
        return Err(if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
            Error::Transient(format!("server answered {status}"))
        } else {
            Error::msg(format!("the download server answered {status}"))
        });
    }

    let is_html = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.to_ascii_lowercase().starts_with("text/html"));
    if is_html && !headers.contains_key(header::CONTENT_DISPOSITION) {
        return Err(Error::msg("the link returned a web page instead of a file; the mod author needs to fix the download link"));
    }

    // 206 continues the part file; a plain 200 (server ignores Range) restarts it.
    let resume = status == StatusCode::PARTIAL_CONTENT && existing > 0;
    let mut hasher = want_md5.then(Md5::new);
    let mut file;
    let mut offset: u64;
    if resume {
        offset = existing;
        if let Some(h) = hasher.as_mut() {
            md5_of_prefix(part, h)?;
        }
        file = OpenOptions::new().append(true).open(part)?;
    } else {
        offset = 0;
        file = OpenOptions::new().create(true).write(true).truncate(true).open(part)?;
    }
    // keep the caller's progress equal to what is on disk
    let delta = offset as i64 - *reported;
    if delta != 0 {
        on_bytes(delta);
        *reported = offset as i64;
    }

    let total = if resume {
        headers.get(header::CONTENT_RANGE).and_then(|v| v.to_str().ok()).and_then(content_range_total)
    } else {
        resp.content_length()
    };
    if let Some(t) = total {
        on_total(t);
    }

    let mut stream = resp.bytes_stream();
    loop {
        cancel.check()?;
        let next = tokio::time::timeout(opts.chunk_timeout, stream.next())
            .await
            .map_err(|_| Error::Transient(format!("no data for {} s", opts.chunk_timeout.as_secs())))?;
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(classify)?;
        file.write_all(&chunk)?;
        if let Some(h) = hasher.as_mut() {
            h.update(&chunk);
        }
        offset += chunk.len() as u64;
        *reported += chunk.len() as i64;
        on_bytes(chunk.len() as i64);
    }
    file.flush()?;
    drop(file);

    if let Some(t) = total {
        if offset != t {
            return Err(Error::Transient(format!("connection closed after {offset} of {t} bytes")));
        }
    }
    Ok(Fetched { file_name: name, size: offset, md5: hasher.map(|h| hex::encode(h.finalize())) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_disposition_variants() {
        assert_eq!(file_name_from_disposition(r#"attachment; filename="ROTR 1.87.7z""#).as_deref(), Some("ROTR 1.87.7z"));
        assert_eq!(file_name_from_disposition("attachment; filename*=UTF-8''My%20Mod%C3%A9.zip").as_deref(), Some("My Modé.zip"));
        assert_eq!(file_name_from_disposition("inline"), None);
    }

    #[test]
    fn names_are_made_safe() {
        assert_eq!(sanitize_file_name("../../evil.zip"), "evil.zip");
        assert_eq!(sanitize_file_name(r"C:\x\y:z?.7z"), "y_z_.7z");
        assert_eq!(sanitize_file_name("  ..  "), "download");
    }

    #[test]
    fn percent_decoding_handles_edges() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("x%2"), "x%2");
    }
}
