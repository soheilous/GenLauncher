//! The mod storage the catalog points to is a public S3 (MinIO) bucket. Listing works
//! anonymously, so no credentials are needed (GenLauncher 1.x shipped a key pair in its source).

use reqwest::Url;

use crate::error::{Error, Result};
use crate::sigv4::{self, Creds};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// Path below the mod's folder, always with `/` separators.
    pub rel: String,
    /// Unquoted ETag. For ordinary uploads this is the file's MD5.
    pub etag: String,
    pub size: u64,
}

impl Object {
    /// The MD5 to verify against, unless the object was uploaded in parts (`<md5>-<n>`).
    pub fn md5(&self) -> Option<&str> {
        (self.etag.len() == 32 && self.etag.bytes().all(|b| b.is_ascii_hexdigit())).then_some(self.etag.as_str())
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else { break };
        let ent = &rest[1..end];
        let rep = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match rep {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = s.find(&open)? + open.len();
    let end = s[start..].find(&format!("</{name}>"))? + start;
    Some(&s[start..end])
}

pub struct Page {
    pub objects: Vec<Object>,
    pub next_token: Option<String>,
}

/// Parse one ListObjectsV2 response; keys outside `folder/` and directory markers are dropped.
pub fn parse_page(xml: &str, folder: &str) -> Result<Page> {
    if !xml.contains("<ListBucketResult") {
        return Err(Error::msg("the mod storage did not return a file list"));
    }
    let prefix = format!("{}/", folder.trim_matches('/'));
    let mut objects = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<Contents>") {
        let block_end = rest[i..].find("</Contents>").map(|e| e + i).ok_or_else(|| Error::msg("truncated file list"))?;
        let block = &rest[i..block_end];
        rest = &rest[block_end..];
        let (Some(key), Some(etag), Some(size)) = (tag(block, "Key"), tag(block, "ETag"), tag(block, "Size")) else { continue };
        let key = unescape(key);
        let Some(rel) = key.strip_prefix(&prefix) else { continue };
        if rel.is_empty() || rel.ends_with('/') {
            continue;
        }
        objects.push(Object {
            rel: rel.to_string(),
            etag: unescape(etag).trim_matches('"').to_string(),
            size: size.trim().parse().map_err(|_| Error::msg("bad size in file list"))?,
        });
    }
    let truncated = tag(xml, "IsTruncated").is_some_and(|t| t.trim() == "true");
    let next_token = if truncated { tag(xml, "NextContinuationToken").map(unescape) } else { None };
    Ok(Page { objects, next_token })
}

fn with_scheme(host: &str, default: &str) -> String {
    if host.starts_with("http://") || host.starts_with("https://") { host.trim_end_matches('/').to_string() } else { format!("{default}://{host}") }
}

/// `host` as written in the catalog (`gen.insave.ovh:9000`): the API endpoint used for listing.
pub fn list_base(host: &str) -> String {
    with_scheme(host, "http")
}

/// Files are served over https on the same host without the API port.
pub fn file_base(host: &str) -> String {
    if host.starts_with("http://") || host.starts_with("https://") {
        return host.trim_end_matches('/').to_string();
    }
    format!("https://{}", host.split(':').next().unwrap_or(host))
}

pub async fn list(client: &reqwest::Client, base: &str, bucket: &str, folder: &str, creds: Option<&Creds>) -> Result<Vec<Object>> {
    let mut all = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let prefix = format!("{}/", folder.trim_matches('/'));
        let mut params = vec![("list-type", "2"), ("prefix", prefix.as_str())];
        if let Some(t) = &token {
            params.push(("continuation-token", t.as_str()));
        }
        // one encoding for the wire and the signature, or signed requests would be rejected
        let query = sigv4::canonical_query(&params);
        let path = format!("/{bucket}/");
        let url = Url::parse(&format!("{}{path}?{query}", base.trim_end_matches('/'))).map_err(|e| Error::msg(e.to_string()))?;
        let mut req = client.get(url.clone());
        if let Some(c) = creds {
            let host = match url.port() {
                Some(p) => format!("{}:{p}", url.host_str().unwrap_or_default()),
                None => url.host_str().unwrap_or_default().to_string(),
            };
            let date = sigv4::now();
            req = req
                .header("x-amz-date", &date)
                .header("x-amz-content-sha256", sigv4::content_sha256())
                .header("Authorization", sigv4::authorization(c, &host, &path, &query, &date));
        }
        let resp = req.send().await.map_err(|e| Error::Transient(e.to_string()))?;
        if !resp.status().is_success() {
            let hint = if matches!(resp.status().as_u16(), 401 | 403) && creds.is_none() {
                " (this storage is not publicly listable and the mod provides no access key)"
            } else {
                ""
            };
            return Err(Error::msg(format!("the mod storage answered {} when listing files{hint}", resp.status())));
        }
        let page = parse_page(&resp.text().await.map_err(|e| Error::Transient(e.to_string()))?, folder)?;
        all.extend(page.objects);
        match page.next_token {
            Some(t) => token = Some(t),
            None => return Ok(all),
        }
    }
}

pub fn object_url(base: &str, bucket: &str, folder: &str, rel: &str) -> Result<String> {
    let mut url = Url::parse(base).map_err(|e| Error::msg(e.to_string()))?;
    {
        let mut seg = url.path_segments_mut().map_err(|_| Error::msg("bad storage address"))?;
        seg.pop_if_empty();
        seg.push(bucket);
        for part in folder.trim_matches('/').split('/') {
            seg.push(part);
        }
        for part in rel.split('/') {
            seg.push(part);
        }
    }
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>rotr</Name><Prefix>rotr-individual-files/</Prefix><NextContinuationToken>abc==</NextContinuationToken><KeyCount>4</KeyCount><IsTruncated>true</IsTruncated>
<Contents><Key>rotr-individual-files/!!!Intrnl_AI.gib</Key><LastModified>2022-01-19T14:21:32.701Z</LastModified><ETag>&#34;add389ed7f38c24b8f0e764904cb3da5&#34;</ETag><Size>2155222</Size></Contents>
<Contents><Key>rotr-individual-files/Data/Art &amp; Sound/a b.tga</Key><ETag>&#34;0123456789abcdef0123456789abcdef-3&#34;</ETag><Size>12</Size></Contents>
<Contents><Key>rotr-individual-files/Data/</Key><ETag>&#34;d41d8cd98f00b204e9800998ecf8427e&#34;</ETag><Size>0</Size></Contents>
<Contents><Key>other-folder/x.big</Key><ETag>&#34;d41d8cd98f00b204e9800998ecf8427e&#34;</ETag><Size>5</Size></Contents>
</ListBucketResult>"#;

    #[test]
    fn parses_a_real_minio_page() {
        let p = parse_page(XML, "rotr-individual-files").unwrap();
        assert_eq!(p.next_token.as_deref(), Some("abc=="));
        assert_eq!(p.objects.len(), 2, "dir marker and foreign folder dropped");
        assert_eq!(p.objects[0].rel, "!!!Intrnl_AI.gib");
        assert_eq!(p.objects[0].md5(), Some("add389ed7f38c24b8f0e764904cb3da5"));
        assert_eq!(p.objects[0].size, 2_155_222);
        assert_eq!(p.objects[1].rel, "Data/Art & Sound/a b.tga");
        assert_eq!(p.objects[1].md5(), None, "multipart etag is not an md5");
    }

    #[test]
    fn last_page_has_no_token_and_garbage_is_rejected() {
        let last = XML.replace("<IsTruncated>true</IsTruncated>", "<IsTruncated>false</IsTruncated>");
        assert!(parse_page(&last, "rotr-individual-files").unwrap().next_token.is_none());
        assert!(parse_page("<html>403</html>", "x").is_err());
    }

    #[test]
    fn entity_decoding() {
        assert_eq!(unescape("a&amp;b &lt;c&gt; &#34;q&#34; &#x41; &unknown; &"), "a&b <c> \"q\" A &unknown; &");
    }

    #[test]
    fn urls() {
        assert_eq!(list_base("gen.insave.ovh:9000"), "http://gen.insave.ovh:9000");
        assert_eq!(file_base("gen.insave.ovh:9000"), "https://gen.insave.ovh");
        assert_eq!(
            object_url("https://gen.insave.ovh", "rotr", "rotr-individual-files", "Data/Art & Sound/a b#1.tga").unwrap(),
            "https://gen.insave.ovh/rotr/rotr-individual-files/Data/Art%20&%20Sound/a%20b%231.tga"
        );
    }
}
