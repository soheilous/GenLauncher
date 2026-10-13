//! Rules for the built-in browser window used to take downloads from web pages (ModDB and
//! others) without scraping them: the user browses and clicks, the launcher only takes the file.
//!
//! The window is created in `lib.rs`; the decisions live here so they can be tested.

use std::fs;
use std::path::{Path, PathBuf};

use crate::download::sanitize_file_name;
use crate::layout;

/// Where files downloaded by the browser window land until they are installed.
pub fn dir(game_dir: &Path) -> PathBuf {
    layout::tmp(game_dir).join("browser")
}

/// Remove files left by an earlier attempt, so a new session starts clean.
pub fn clear(dir: &Path) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let _ = if e.path().is_dir() { fs::remove_dir_all(e.path()) } else { fs::remove_file(e.path()) };
        }
    }
}

/// Only normal web pages may be browsed: no `file:`, `javascript:`, app or custom schemes.
pub fn allowed_navigation(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "http" | "https") || url.as_str() == "about:blank"
}

/// The path the browser must save a download to: our folder, a safe file name.
pub fn destination(dir: &Path, suggested: &Path) -> PathBuf {
    let name = suggested.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    dir.join(sanitize_file_name(&name))
}

/// A link that is clearly a file, so no browsing is needed: it can be downloaded directly.
/// Everything else is opened in the browser window (which also copes with pages whose
/// download button starts the file through a redirect).
pub fn looks_like_direct_file(url: &str) -> bool {
    let Ok(u) = tauri::Url::parse(url.trim()) else { return false };
    if !matches!(u.scheme(), "http" | "https") {
        return false;
    }
    let host = u.host_str().unwrap_or("").to_lowercase();
    let path = u.path().to_lowercase();
    if host.ends_with("moddb.com") && !path.ends_with(".zip") && !path.ends_with(".7z") && !path.ends_with(".rar") {
        return false; // ModDB links are pages or session-bound mirror redirects: browse them
    }
    let archive = [".zip", ".7z", ".rar", ".big", ".gib"].iter().any(|e| path.ends_with(e));
    let share = (host == "onedrive.live.com" && path.starts_with("/download"))
        || (host.ends_with("dropbox.com") && u.query().is_some_and(|q| q.contains("dl=")))
        || (host.ends_with("github.com") && path.contains("/releases/download/"))
        || host == "objects.githubusercontent.com";
    archive || share
}

/// Downloads that cannot be mods: installers and scripts the launcher will never run.
pub fn is_program_installer(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    [".exe", ".msi", ".bat", ".cmd", ".ps1", ".sh", ".appimage", ".dmg"].iter().any(|e| lower.ends_with(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> tauri::Url {
        tauri::Url::parse(s).unwrap()
    }

    #[test]
    fn only_web_pages_can_be_browsed() {
        assert!(allowed_navigation(&url("https://www.moddb.com/mods/contra")));
        assert!(allowed_navigation(&url("http://example.org/")));
        assert!(allowed_navigation(&url("about:blank")));
        for bad in ["file:///C:/Windows/System32/cmd.exe", "javascript:alert(1)", "ftp://x/y", "tauri://localhost/", "data:text/html,hi", "ms-msdt:/id"] {
            assert!(!allowed_navigation(&url(bad)), "{bad}");
        }
    }

    #[test]
    fn downloads_are_confined_to_our_folder() {
        let d = Path::new("/g/.genlauncher/tmp/browser");
        assert_eq!(destination(d, Path::new("C:/Users/me/Downloads/Mod v1.zip")), d.join("Mod v1.zip"));
        assert_eq!(destination(d, Path::new("../../evil.dll")), d.join("evil.dll"));
        assert_eq!(destination(d, Path::new("")), d.join("download"));
        // odd characters never survive, wherever the platform splits the name
        let odd = destination(d, Path::new("a:b?*.zip"));
        assert_eq!(odd.parent(), Some(d));
        let n = odd.file_name().unwrap().to_string_lossy().into_owned();
        assert!(n.ends_with(".zip") && !n.contains([':', '?', '*']), "{n}");
    }

    #[test]
    fn direct_links_versus_pages() {
        for direct in [
            "https://example.org/files/Mod_1.2.zip",
            "https://example.org/Mod.7Z?token=1",
            "https://onedrive.live.com/download?cid=1&resid=2&authkey=3",
            "https://www.dropbox.com/s/abc/ROTR.7z?dl=1",
            "https://github.com/me/mod/releases/download/v1/mod.zip",
        ] {
            assert!(looks_like_direct_file(direct), "{direct} should be direct");
        }
        for page in [
            "https://www.moddb.com/mods/zero-hour-reborn-omega",
            "https://www.moddb.com/mods/contra/downloads/contra-009",
            "https://www.moddb.com/downloads/mirror/237715/128/4a7abb/?referer=x",
            "https://example.org/download?id=5",
            "https://github.com/me/mod/releases",
            "not a link",
            "ftp://x/y.zip",
        ] {
            assert!(!looks_like_direct_file(page), "{page} should open the browser");
        }
    }

    #[test]
    fn installers_are_recognised() {
        for n in ["setup.exe", "MOD.MSI", "run.bat", "x.sh", "x.AppImage"] {
            assert!(is_program_installer(n), "{n}");
        }
        for n in ["mod.zip", "a.7z", "Art.big", "readme"] {
            assert!(!is_program_installer(n), "{n}");
        }
    }

    #[test]
    fn clear_removes_leftovers_only_inside_the_folder() {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().join("browser");
        fs::create_dir_all(d.join("sub")).unwrap();
        fs::write(d.join("old.zip"), b"x").unwrap();
        fs::write(d.join("sub/f"), b"x").unwrap();
        fs::write(t.path().join("keep.txt"), b"x").unwrap();
        clear(&d);
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
        assert!(t.path().join("keep.txt").exists());
        clear(&t.path().join("does-not-exist")); // no panic
    }
}
