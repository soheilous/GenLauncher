//! Updating GenLauncher itself.
//!
//! A published release also publishes `latest.json` (the format Tauri's updater uses) at a fixed address, which names the
//! newest setup program and carries its minisign signature. Before anything is run the program is checked against the
//! public key in tauri.conf.json, so a changed file (or someone else's server) cannot get code onto the computer.
//! (Tauri's own updater plugin does the same; it could not be built at the time of writing because the crates it needs
//! had been withdrawn, and the pieces are small.)

use crate::error::{Error, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

const PLATFORM: &str = "windows-x86_64";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Update {
    pub version: String,
    pub notes: String,
    pub url: String,
    #[serde(skip)]
    pub signature: String,
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: String,
    platforms: std::collections::HashMap<String, Platform>,
}

#[derive(Deserialize)]
struct Platform {
    url: String,
    signature: String,
}

/// The newer version named by a manifest, if it is newer than `current`.
pub fn newer_in(manifest: &str, current: &str) -> Result<Option<Update>> {
    let m: Manifest = serde_json::from_str(manifest).map_err(|_| Error::msg("The update information was not understood"))?;
    let latest = m.version.trim_start_matches('v').to_string();
    if crate::version::compare(&latest, current.trim_start_matches('v')) != std::cmp::Ordering::Greater {
        return Ok(None);
    }
    let p = m.platforms.get(PLATFORM).ok_or_else(|| Error::msg("There is no update for this kind of computer"))?;
    Ok(Some(Update { version: latest, notes: m.notes, url: p.url.clone(), signature: p.signature.clone() }))
}

fn decode_text(b64: &str) -> Result<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| Error::msg("The update's signature is not valid"))?;
    String::from_utf8(bytes).map_err(|_| Error::msg("The update's signature is not valid"))
}

/// Check `data` against a minisign signature. Both the key and the signature are base64 of the minisign text files.
pub fn verify(pubkey_b64: &str, signature_b64: &str, data: &[u8]) -> Result<()> {
    let key = minisign_verify::PublicKey::decode(&decode_text(pubkey_b64)?).map_err(|_| Error::msg("The update key is not valid"))?;
    let sig = minisign_verify::Signature::decode(&decode_text(signature_b64)?).map_err(|_| Error::msg("The update's signature is not valid"))?;
    key.verify(data, &sig, true).map_err(|_| Error::msg("The downloaded update does not match its signature, so it was not installed"))
}

/// The update address and key the app was built with.
pub struct Settings {
    pub endpoint: String,
    pub pubkey: String,
}

pub fn settings(app: &tauri::AppHandle) -> Result<Settings> {
    let cfg = app.config().plugins.0.get("updater").ok_or_else(|| Error::msg("Updates are not set up in this build"))?;
    let endpoint = cfg["endpoints"][0].as_str().ok_or_else(|| Error::msg("Updates are not set up in this build"))?.to_string();
    let pubkey = cfg["pubkey"].as_str().ok_or_else(|| Error::msg("Updates are not set up in this build"))?.to_string();
    Ok(Settings { endpoint, pubkey })
}

pub async fn check(http: &reqwest::Client, endpoint: &str, current: &str) -> Result<Option<Update>> {
    let resp = http.get(endpoint).header("accept", "application/json").send().await?;
    if !resp.status().is_success() {
        return Err(Error::msg(format!("Could not check for updates ({})", resp.status())));
    }
    newer_in(&resp.text().await?, current)
}

/// Download the setup program, verify it and return where it was saved.
pub async fn download(http: &reqwest::Client, up: &Update, pubkey: &str) -> Result<std::path::PathBuf> {
    // only https from the project's own release page
    if !up.url.starts_with("https://github.com/soheilous/GenLauncher/releases/download/") {
        return Err(Error::msg("The update points somewhere unexpected, so it was not installed"));
    }
    let resp = http.get(&up.url).send().await?;
    if !resp.status().is_success() {
        return Err(Error::msg(format!("Could not download the update ({})", resp.status())));
    }
    let bytes = resp.bytes().await?;
    verify(pubkey, &up.signature, &bytes)?;
    let path = std::env::temp_dir().join(format!("GenLauncher-update-{}.exe", up.version));
    std::fs::write(&path, &bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> String {
        format!(r#"{{"version":"{version}","notes":"n","platforms":{{"windows-x86_64":{{"url":"https://github.com/soheilous/GenLauncher/releases/download/v{version}/x.exe","signature":"c2ln"}}}}}}"#)
    }

    #[test]
    fn only_a_newer_version_is_offered() {
        assert!(newer_in(&manifest("0.1.1"), "0.1.1").unwrap().is_none());
        assert!(newer_in(&manifest("0.1.0"), "0.1.1").unwrap().is_none());
        let up = newer_in(&manifest("0.2.0"), "0.1.1").unwrap().unwrap();
        assert_eq!(up.version, "0.2.0");
        assert!(newer_in(&manifest("v0.10.0"), "0.9.9").unwrap().is_some());
    }

    #[test]
    fn a_manifest_without_this_platform_or_garbage_is_an_error() {
        assert!(newer_in(r#"{"version":"9.0.0","platforms":{}}"#, "0.1.0").is_err());
        assert!(newer_in("not json", "0.1.0").is_err());
    }

    // signed with the project's real key (tauri signer sign) over the bytes "hello update"
    const KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEM0MDQ1REVBQjZBMjYwN0YKUldSL1lLSzI2bDBFeEo3ODBneCticHpNdWY5Qk9yUDBhVzVrUHEvNzBFTmtXaDNzYml0Wk4wQk8K";
    const SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSL1lLSzI2bDBFeFBIZ0RqNUZRZjJObXNZUHVWbk8ySlhLQ04vMVNxeXg5cDdjM0E5R0ExQ3ZqY09mdU0rME5GcDE4bDU5aXZsMHc1SXNkZWM0b2JZRDRUYlhHSTJmNlFZPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkyMTQzNjQ5CWZpbGU6c2FtcGxlLmJpbgp4MnA2L1ZkSkNZUGZBczAzLzV5aTVRbW02cEhvZXNpdTdTRWMyYXk5UVJ6UTErR2dVOW13S0ZWV25FRVdHdkNNTWhIeCt1QXlUZWdOcnVaMW9NWldDdz09Cg==";

    #[test]
    fn a_genuine_signature_passes_and_a_changed_file_does_not() {
        verify(KEY, SIG, b"hello update").unwrap();
        assert!(verify(KEY, SIG, b"hello update!").is_err());
    }

    /// With GL_TEST_SETUP set to a built setup program that has its .sig beside it (cargo test -- --ignored).
    #[test]
    #[ignore]
    fn a_built_setup_program_verifies_against_the_key_in_the_config() {
        let setup = std::env::var("GL_TEST_SETUP").expect("GL_TEST_SETUP");
        let sig = std::fs::read_to_string(format!("{setup}.sig")).unwrap();
        let conf: serde_json::Value = serde_json::from_str(&std::fs::read_to_string("tauri.conf.json").unwrap()).unwrap();
        let key = conf["plugins"]["updater"]["pubkey"].as_str().unwrap();
        verify(key, &sig, &std::fs::read(&setup).unwrap()).unwrap();
    }

    #[test]
    fn a_bad_signature_is_refused() {
        assert!(verify("bm9wZQ==", "bm9wZQ==", b"data").is_err());
    }
}
