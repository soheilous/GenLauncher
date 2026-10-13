//! Version ordering for mod versions like `1.87`, `009 Final`, `1.0 beta 2`.
//!
//! GenLauncher 1.x concatenated all digits ("1.10" -> 110, "1.9" -> 19 -> padded 190),
//! which ranks 1.9 above 1.10. This compares numeric segments numerically instead.

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Num(u64),
    Text(String),
}

fn tokenize(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            let mut n = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    n.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(Tok::Num(n.parse().unwrap_or(u64::MAX)));
        } else if c.is_alphabetic() {
            let mut t = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_alphabetic() {
                    t.extend(d.to_lowercase());
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(Tok::Text(t));
        } else {
            chars.next();
        }
    }
    out
}

fn is_prerelease(word: &str) -> bool {
    matches!(
        word,
        "alpha" | "beta" | "rc" | "pre" | "preview" | "test" | "dev" | "wip" | "snapshot" | "a" | "b"
    )
}

pub fn compare(a: &str, b: &str) -> Ordering {
    let (ta, tb) = (tokenize(a), tokenize(b));
    for (x, y) in ta.iter().zip(tb.iter()) {
        let ord = match (x, y) {
            (Tok::Num(p), Tok::Num(q)) => p.cmp(q),
            (Tok::Text(p), Tok::Text(q)) => match (is_prerelease(p), is_prerelease(q)) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => p.cmp(q),
            },
            // number against a word at the same position: the number is the plain release
            (Tok::Num(_), Tok::Text(w)) => if is_prerelease(w) { Ordering::Greater } else { Ordering::Less },
            (Tok::Text(w), Tok::Num(_)) => if is_prerelease(w) { Ordering::Less } else { Ordering::Greater },
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    // Common prefix equal: the longer one wins unless its extra part is a pre-release tag.
    match ta.len().cmp(&tb.len()) {
        Ordering::Equal => Ordering::Equal,
        Ordering::Greater => match &ta[tb.len()] {
            Tok::Text(w) if is_prerelease(w) => Ordering::Less,
            _ => Ordering::Greater,
        },
        Ordering::Less => match &tb[ta.len()] {
            Tok::Text(w) if is_prerelease(w) => Ordering::Greater,
            _ => Ordering::Less,
        },
    }
}

/// Sort newest first.
pub fn sort_desc(versions: &mut [String]) {
    versions.sort_by(|a, b| compare(b, a));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::*;

    #[test]
    fn numeric_segments_not_concatenated() {
        assert_eq!(compare("1.10", "1.9"), Greater); // old launcher got this wrong
        assert_eq!(compare("1.9", "1.10"), Less);
        assert_eq!(compare("1.87", "1.86"), Greater);
        assert_eq!(compare("2.0", "2.0"), Equal);
    }

    #[test]
    fn missing_segments_and_prerelease() {
        assert_eq!(compare("1.0.1", "1.0"), Greater);
        assert_eq!(compare("1.0 beta", "1.0"), Less);
        assert_eq!(compare("1.0", "1.0 beta 2"), Greater);
        assert_eq!(compare("1.0 beta 3", "1.0 beta 2"), Greater);
    }

    #[test]
    fn contra_style_names() {
        assert_eq!(compare("009 Final", "009 Beta 3"), Greater);
        assert_eq!(compare("0.97.5", "0.97.10"), Less);
    }

    #[test]
    fn sorts_newest_first() {
        let mut v = vec!["1.9".to_string(), "1.10".into(), "1.2".into()];
        sort_desc(&mut v);
        assert_eq!(v, ["1.10", "1.9", "1.2"]);
    }
}
