//! Minimal reader for EA `.big` archives, enough to find `MaxCameraHeight` in a mod's
//! `Data\INI\GameData.ini` so the launcher can change it.
//!
//! Layout (big-endian except the size field): `"BIGF"`, u32 LE archive size, u32 BE entry
//! count, u32 BE header size, then per entry: u32 BE offset, u32 BE size, NUL-terminated name.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, Result};

pub const GAME_DATA_INI: &str = r"data\ini\gamedata.ini";

fn be32(r: &mut impl Read) -> Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_be_bytes(b))
}

/// `(offset, size)` of the entry named `wanted` (case-insensitive, `/` and `\` equal).
pub fn find_entry(path: &Path, wanted: &str) -> Result<Option<(u64, u64)>> {
    let mut r = BufReader::new(File::open(path)?);
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != b"BIGF" && &magic != b"BIG4" {
        return Ok(None); // not an archive (e.g. a 24-byte placeholder)
    }
    let mut skip = [0u8; 4];
    r.read_exact(&mut skip)?;
    let count = be32(&mut r)?;
    let _header = be32(&mut r)?;
    let wanted = wanted.replace('/', "\\").to_lowercase();
    for _ in 0..count {
        let offset = be32(&mut r)? as u64;
        let size = be32(&mut r)? as u64;
        let mut name = Vec::new();
        loop {
            let mut b = [0u8; 1];
            r.read_exact(&mut b)?;
            if b[0] == 0 {
                break;
            }
            name.push(b[0]);
            if name.len() > 1024 {
                return Err(Error::msg("corrupt .big entry name"));
            }
        }
        if String::from_utf8_lossy(&name).replace('/', "\\").to_lowercase() == wanted {
            return Ok(Some((offset, size)));
        }
    }
    Ok(None)
}

/// A number in the archive that can be overwritten in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot {
    pub offset: u64,
    pub old: Vec<u8>,
}

/// Locate the value of `MaxCameraHeight` inside GameData.ini (None if the archive lacks it).
pub fn find_camera_height(path: &Path) -> Result<Option<Spot>> {
    let Some((offset, size)) = find_entry(path, GAME_DATA_INI)? else { return Ok(None) };
    if size > 8 * 1024 * 1024 {
        return Ok(None);
    }
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut data = vec![0u8; size as usize];
    f.read_exact(&mut data)?;

    let lower: Vec<u8> = data.iter().map(|b| b.to_ascii_lowercase()).collect();
    let key = b"maxcameraheight";
    let mut from = 0;
    while let Some(rel) = lower[from..].windows(key.len()).position(|w| w == key) {
        let at = from + rel;
        from = at + key.len();
        // skip occurrences inside `;` comments
        let line_start = lower[..at].iter().rposition(|&b| b == b'\n').map_or(0, |p| p + 1);
        if lower[line_start..at].contains(&b';') {
            continue;
        }
        let mut i = at + key.len();
        while i < data.len() && (data[i] == b' ' || data[i] == b'\t') {
            i += 1;
        }
        if data.get(i) != Some(&b'=') {
            continue;
        }
        i += 1;
        while i < data.len() && (data[i] == b' ' || data[i] == b'\t') {
            i += 1;
        }
        let start = i;
        while i < data.len() && (data[i].is_ascii_digit() || data[i] == b'.') {
            i += 1;
        }
        if i > start {
            return Ok(Some(Spot { offset: offset + start as u64, old: data[start..i].to_vec() }));
        }
    }
    Ok(None)
}

/// The same number of bytes as `len` holding `height`, e.g. height 400, len 5 -> `400.0`.
/// (Entries cannot grow without rewriting the archive, so the width must be preserved.)
pub fn fit_height(height: u32, len: usize) -> Result<Vec<u8>> {
    let digits = height.to_string();
    if digits.len() > len {
        return Err(Error::msg(format!(
            "camera height {height} does not fit the {len}-character value in this mod's GameData.ini; use a smaller value"
        )));
    }
    let mut s = digits;
    if s.len() < len {
        s.push('.');
        while s.len() < len {
            s.push('0');
        }
    }
    Ok(s.into_bytes())
}

#[cfg(test)]
pub(crate) fn build_big(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let table: usize = entries.iter().map(|(n, _)| 8 + n.len() + 1).sum();
    let header = 16 + table;
    let mut out = Vec::new();
    out.extend_from_slice(b"BIGF");
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    out.extend_from_slice(&(header as u32).to_be_bytes());
    let mut offset = header;
    for (name, data) in entries {
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
        out.push(0);
        offset += data.len();
    }
    for (_, data) in entries {
        out.extend_from_slice(data);
    }
    let total = out.len() as u32;
    out[4..8].copy_from_slice(&total.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn finds_camera_height_value() {
        let d = tempfile::tempdir().unwrap();
        let ini = b"GameData\n  ; MaxCameraHeight = 999.0\n  MaxCameraHeight = 310.0\nEnd\n";
        let p = write(d.path(), "x.big", &build_big(&[("Data\\INI\\Other.ini", b"x"), ("Data\\INI\\GameData.ini", ini)]));
        let spot = find_camera_height(&p).unwrap().unwrap();
        assert_eq!(spot.old, b"310.0"); // not the commented one
        let bytes = std::fs::read(&p).unwrap();
        assert_eq!(&bytes[spot.offset as usize..spot.offset as usize + 5], b"310.0");
    }

    #[test]
    fn absent_or_not_an_archive() {
        let d = tempfile::tempdir().unwrap();
        let p = write(d.path(), "a.big", &build_big(&[("Data\\Other.txt", b"hi")]));
        assert!(find_camera_height(&p).unwrap().is_none());
        let q = write(d.path(), "b.big", b"??"); // placeholder
        assert!(find_entry(&q, GAME_DATA_INI).is_err() || find_entry(&q, GAME_DATA_INI).unwrap().is_none());
        let empty = write(d.path(), "c.big", &[0u8; 24]);
        assert!(find_camera_height(&empty).unwrap().is_none());
    }

    #[test]
    fn height_keeps_the_original_width() {
        assert_eq!(fit_height(400, 5).unwrap(), b"400.0");
        assert_eq!(fit_height(400, 7).unwrap(), b"400.000");
        assert_eq!(fit_height(1000, 5).unwrap(), b"1000.");
        assert_eq!(fit_height(400, 3).unwrap(), b"400");
        assert!(fit_height(1000, 3).is_err());
    }
}
