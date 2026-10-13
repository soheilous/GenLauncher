//! Unpacking downloaded archives (zip, 7z, rar) into a mod folder.
//!
//! * Entry paths are validated: an archive can never write outside the target folder.
//! * `*.big` entries are stored as `*.gib` (the game ignores them until a launch links them in).

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

pub fn is_archive_name(name: &str) -> bool {
    matches!(ext(name).as_str(), "zip" | "7z" | "rar")
}

fn ext(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Which format a file is, by magic bytes (file names from download hosts are unreliable).
pub fn sniff(path: &Path) -> Result<&'static str> {
    let mut head = [0u8; 8];
    let n = File::open(path)?.read(&mut head)?;
    let h = &head[..n];
    if h.starts_with(b"PK\x03\x04") || h.starts_with(b"PK\x05\x06") {
        Ok("zip")
    } else if h.starts_with(b"7z\xBC\xAF\x27\x1C") {
        Ok("7z")
    } else if h.starts_with(b"Rar!") {
        Ok("rar")
    } else {
        Err(Error::msg("the downloaded file is not a zip, 7z or rar archive"))
    }
}

/// Where an entry goes, or an error if it tries to escape `dest`.
pub fn safe_target(dest: &Path, entry: &str) -> Result<PathBuf> {
    let mut rel = PathBuf::new();
    for comp in Path::new(&entry.replace('\\', "/")).components() {
        match comp {
            Component::Normal(c) => rel.push(c),
            Component::CurDir => {}
            _ => return Err(Error::msg(format!("unsafe path in archive: {entry}"))),
        }
    }
    if rel.as_os_str().is_empty() {
        return Err(Error::msg(format!("empty path in archive: {entry}")));
    }
    if ext(&rel.to_string_lossy()) == "big" {
        rel.set_extension("gib");
    }
    Ok(dest.join(rel))
}

fn write_entry(target: &Path, reader: &mut (impl Read + ?Sized)) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    io::copy(reader, &mut File::create(target)?)?;
    Ok(())
}

pub fn extract(archive: &Path, dest: &Path, cancelled: &dyn Fn() -> bool) -> Result<()> {
    fs::create_dir_all(dest)?;
    match sniff(archive)? {
        "zip" => extract_zip(archive, dest, cancelled),
        "7z" => extract_7z(archive, dest, cancelled),
        _ => extract_rar(archive, dest, cancelled),
    }
}

fn extract_zip(archive: &Path, dest: &Path, cancelled: &dyn Fn() -> bool) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?).map_err(|e| Error::msg(format!("bad zip: {e}")))?;
    for i in 0..zip.len() {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let mut entry = zip.by_index(i).map_err(|e| Error::msg(format!("bad zip entry: {e}")))?;
        let name = entry.name().map_err(|e| Error::msg(format!("bad zip entry name: {e}")))?.to_string();
        if entry.is_dir() {
            fs::create_dir_all(safe_target(dest, name.trim_end_matches('/'))?)?;
            continue;
        }
        write_entry(&safe_target(dest, &name)?, &mut entry)?;
    }
    Ok(())
}

fn extract_7z(archive: &Path, dest: &Path, cancelled: &dyn Fn() -> bool) -> Result<()> {
    let mut reader = sevenz_rust2::ArchiveReader::new(File::open(archive)?, sevenz_rust2::Password::empty())
        .map_err(|e| Error::msg(format!("bad 7z: {e}")))?;
    let mut failure: Option<Error> = None;
    reader
        .for_each_entries(|entry, data| {
            if cancelled() {
                failure = Some(Error::Cancelled);
                return Ok(false);
            }
            if entry.is_directory() {
                match safe_target(dest, entry.name().trim_end_matches('/')) {
                    Ok(p) => fs::create_dir_all(p)?,
                    Err(e) => {
                        failure = Some(e);
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            match safe_target(dest, entry.name()).and_then(|p| write_entry(&p, data)) {
                Ok(()) => Ok(true),
                Err(e) => {
                    failure = Some(e);
                    Ok(false)
                }
            }
        })
        .map_err(|e| Error::msg(format!("7z extraction failed: {e}")))?;
    failure.map_or(Ok(()), Err)
}

fn extract_rar(archive: &Path, dest: &Path, cancelled: &dyn Fn() -> bool) -> Result<()> {
    let bad = |e: unrar::error::UnrarError| Error::msg(format!("rar extraction failed: {e}"));
    let mut open = unrar::Archive::new(archive).open_for_processing().map_err(bad)?;
    while let Some(header) = open.read_header().map_err(bad)? {
        if cancelled() {
            return Err(Error::Cancelled);
        }
        let entry = header.entry();
        let name = entry.filename.to_string_lossy().into_owned();
        open = if entry.is_file() {
            let target = safe_target(dest, &name)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            header.extract_to(&target).map_err(bad)?
        } else {
            fs::create_dir_all(safe_target(dest, name.trim_end_matches(['/', '\\']))?)?;
            header.skip().map_err(bad)?
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            if name.ends_with('/') {
                z.add_directory(*name, opts).unwrap();
            } else {
                z.start_file(*name, opts).unwrap();
                z.write_all(data).unwrap();
            }
        }
        z.finish().unwrap();
    }

    #[test]
    fn extracts_zip_and_renames_big_to_gib() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("m.zip");
        make_zip(&a, &[("Data/", b""), ("Art.BIG", b"archive"), ("Data/INI/x.ini", b"ini"), ("sub/Inner.big", b"i2")]);
        let out = d.path().join("out");
        extract(&a, &out, &|| false).unwrap();
        assert_eq!(fs::read(out.join("Art.gib")).unwrap(), b"archive");
        assert_eq!(fs::read(out.join("Data/INI/x.ini")).unwrap(), b"ini");
        assert_eq!(fs::read(out.join("sub/Inner.gib")).unwrap(), b"i2");
        assert!(!out.join("Art.BIG").exists() && !out.join("Art.big").exists());
    }

    #[test]
    fn extracts_7z() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        fs::create_dir_all(src.join("Data")).unwrap();
        fs::write(src.join("Mod.big"), b"seven").unwrap();
        fs::write(src.join("Data/a.ini"), b"a").unwrap();
        let a = d.path().join("m.7z");
        sevenz_rust2::compress_to_path(&src, &a).unwrap();
        let out = d.path().join("out");
        extract(&a, &out, &|| false).unwrap();
        // compress_to_path stores paths relative to the parent: find the files wherever they landed
        let found: Vec<String> = walk(&out);
        assert!(found.iter().any(|f| f.ends_with("Mod.gib")), "{found:?}");
        assert!(found.iter().any(|f| f.ends_with("Data/a.ini")), "{found:?}");
    }

    fn walk(dir: &Path) -> Vec<String> {
        let mut v = Vec::new();
        for e in fs::read_dir(dir).unwrap().flatten() {
            if e.path().is_dir() {
                v.extend(walk(&e.path()));
            } else {
                v.push(e.path().to_string_lossy().replace('\\', "/"));
            }
        }
        v
    }

    #[test]
    fn path_traversal_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        for bad in ["../evil.txt", "a/../../evil.txt", "/abs/evil.txt", "C:/evil.txt", r"..\evil.txt"] {
            assert!(safe_target(d.path(), bad).is_err(), "{bad} must be rejected");
        }
        assert!(safe_target(d.path(), "ok/./fine.txt").is_ok());

        // and through a real archive
        let a = d.path().join("evil.zip");
        make_zip(&a, &[("../escape.txt", b"x")]);
        let out = d.path().join("out");
        // The zip writer may normalise names; if the entry survives, extraction must fail.
        let r = extract(&a, &out, &|| false);
        assert!(!d.path().join("escape.txt").exists());
        let _ = r;
    }

    #[test]
    fn rejects_non_archives_and_honours_cancel() {
        let d = tempfile::tempdir().unwrap();
        let junk = d.path().join("page.html");
        fs::write(&junk, b"<html>not a file</html>").unwrap();
        assert!(extract(&junk, &d.path().join("o"), &|| false).unwrap_err().to_string().contains("not a zip"));
        let a = d.path().join("m.zip");
        make_zip(&a, &[("a.txt", b"1")]);
        assert!(matches!(extract(&a, &d.path().join("o2"), &|| true), Err(Error::Cancelled)));
    }

    #[test]
    fn archive_name_detection() {
        assert!(is_archive_name("ROTR.7z") && is_archive_name("a.ZIP") && is_archive_name("x.rar"));
        assert!(!is_archive_name("mod.big") && !is_archive_name("readme"));
    }
}
