//! AWS Signature Version 4 for read-only S3 GET requests (what MinIO expects when a bucket is
//! not publicly listable). Keys come from a mod's own manifest; none are built in.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub const REGION: &str = "us-east-1"; // MinIO's default; the server does not care for single-site setups
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creds {
    pub access_key: String,
    pub secret_key: String,
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    m.update(data.as_bytes());
    m.finalize().into_bytes().to_vec()
}

/// RFC 3986 percent-encoding as SigV4 wants it (unreserved characters stay, `/` is encoded
/// unless `keep_slash`).
pub fn uri_encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b'/' if keep_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `query` sorted and encoded exactly as it must appear both on the wire and in the signature.
pub fn canonical_query(query: &[(&str, &str)]) -> String {
    let mut pairs: Vec<(String, String)> = query.iter().map(|(k, v)| (uri_encode(k, false), uri_encode(v, false))).collect();
    pairs.sort();
    pairs.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")
}

/// `20130524T000000Z` for the given unix time.
pub fn amz_date(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z", secs / 3600, secs % 3600 / 60, secs % 60)
}

pub fn now() -> String {
    amz_date(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0))
}

/// The `Authorization` header for `GET {path}?{canonical_query}` with an empty body, signing
/// `host`, `x-amz-content-sha256` and `x-amz-date` (all of which must be sent).
pub fn authorization(creds: &Creds, host: &str, path: &str, canonical_query: &str, amz_date: &str) -> String {
    let date = &amz_date[..8];
    let signed = "host;x-amz-content-sha256;x-amz-date";
    let canonical = format!(
        "GET\n{}\n{canonical_query}\nhost:{host}\nx-amz-content-sha256:{EMPTY_SHA256}\nx-amz-date:{amz_date}\n\n{signed}\n{EMPTY_SHA256}",
        uri_encode(path, true)
    );
    let scope = format!("{date}/{REGION}/s3/aws4_request");
    let to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}", hex::encode(Sha256::digest(canonical.as_bytes())));

    let k_date = hmac(format!("AWS4{}", creds.secret_key).as_bytes(), date);
    let k_region = hmac(&k_date, REGION);
    let k_service = hmac(&k_region, "s3");
    let k_signing = hmac(&k_service, "aws4_request");
    let signature = hex::encode(hmac(&k_signing, &to_sign));
    format!("AWS4-HMAC-SHA256 Credential={}/{scope},SignedHeaders={signed},Signature={signature}", creds.access_key)
}

pub fn content_sha256() -> &'static str {
    EMPTY_SHA256
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "GET Bucket (List Objects)" example from the AWS Signature V4 documentation.
    #[test]
    fn matches_the_aws_documentation_example() {
        let creds = Creds { access_key: "AKIAIOSFODNN7EXAMPLE".into(), secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into() };
        let q = canonical_query(&[("max-keys", "2"), ("prefix", "J")]);
        assert_eq!(q, "max-keys=2&prefix=J");
        let auth = authorization(&creds, "examplebucket.s3.amazonaws.com", "/", &q, "20130524T000000Z");
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request,SignedHeaders=host;x-amz-content-sha256;x-amz-date,Signature=34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7"
        );
    }

    #[test]
    fn dates_and_encoding() {
        assert_eq!(amz_date(1_369_353_600), "20130524T000000Z");
        assert_eq!(amz_date(1_440_938_160), "20150830T123600Z");
        assert_eq!(amz_date(951_782_400 + 86_399), "20000229T235959Z"); // leap day
        assert_eq!(uri_encode("a b/c&d~é", false), "a%20b%2Fc%26d~%C3%A9");
        assert_eq!(uri_encode("/b/k ey", true), "/b/k%20ey");
        assert_eq!(canonical_query(&[("prefix", "rotr/"), ("list-type", "2")]), "list-type=2&prefix=rotr%2F");
    }
}
