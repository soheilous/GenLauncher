//! Turning the "share" links mod authors put in their manifests into direct download links.
//! Ported from GenLauncher 1.x (`DownloadsLinkParser`).

fn query_value<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|p| p.strip_prefix(key)?.strip_prefix('='))
}

pub fn direct_download_link(link: &str) -> String {
    let link = link.trim();
    if link.contains("www.dropbox.com") {
        // ?dl=0 shows a web page; ?dl=1 serves the file.
        return link.replace("?dl=0", "?dl=1").replace("&dl=0", "&dl=1");
    }
    if link.starts_with("https://onedrive.live.com") {
        return onedrive(link);
    }
    link.to_string()
}

fn onedrive(link: &str) -> String {
    if link.contains("embed") {
        return link.replace("embed", "download");
    }
    if link.contains("/download?") {
        return link.to_string(); // already direct
    }
    let query = link.split_once('?').map(|(_, q)| q).unwrap_or("");
    let cid = query_value(query, "cid");
    let authkey = query_value(query, "authkey");
    // `id=` (not `cid=`): the resource id
    let resid = query.split('&').find_map(|p| p.strip_prefix("id="));
    match (cid, resid) {
        (Some(cid), Some(resid)) => {
            let mut out = format!("https://onedrive.live.com/download?cid={cid}&resid={resid}");
            if let Some(a) = authkey {
                out.push_str("&authkey=");
                out.push_str(a);
            }
            out
        }
        _ => link.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropbox_gets_dl_1() {
        assert_eq!(direct_download_link("https://www.dropbox.com/s/abc/ROTR.7z?dl=0"), "https://www.dropbox.com/s/abc/ROTR.7z?dl=1");
        assert_eq!(direct_download_link("https://www.dropbox.com/s/abc/x.7z?dl=1"), "https://www.dropbox.com/s/abc/x.7z?dl=1");
    }

    #[test]
    fn onedrive_share_link_becomes_download_link() {
        let share = "https://onedrive.live.com/?authkey=%21AIWtLuu54V5qKQ4&cid=896C9369E9176506&id=896C9369E9176506%21464&parId=896C9369E9176506%21463&o=OneUp";
        assert_eq!(
            direct_download_link(share),
            "https://onedrive.live.com/download?cid=896C9369E9176506&resid=896C9369E9176506%21464&authkey=%21AIWtLuu54V5qKQ4"
        );
        let embed = "https://onedrive.live.com/embed?cid=1&resid=2&authkey=3";
        assert_eq!(direct_download_link(embed), "https://onedrive.live.com/download?cid=1&resid=2&authkey=3");
        let direct = "https://onedrive.live.com/download?cid=1&resid=2&authkey=3";
        assert_eq!(direct_download_link(direct), direct);
    }

    #[test]
    fn other_links_untouched() {
        assert_eq!(direct_download_link(" https://example.com/a.zip "), "https://example.com/a.zip");
    }
}
