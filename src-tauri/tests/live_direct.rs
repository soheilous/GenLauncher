//! Needs the internet, so it is ignored by default: `cargo test --test live_direct -- --ignored`.
//! Downloads one small direct link from the catalog with the launcher's own downloader and unpacks it.

use genlauncher2_lib::download::{self, Cancel, Options};

#[tokio::test]
#[ignore]
async fn a_catalog_direct_link_downloads_and_unpacks() {
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("download.part");
    let client = reqwest::Client::builder().user_agent("GenLauncher-test").build().unwrap();
    let url = "https://gen.insave.ovh/contra/contra-patches/ContraRULeikeze.rar";
    let got = download::fetch(&client, url, &part, &Cancel::new(), &|_| {}, &|_| {}, false, &Options::default()).await.unwrap();
    println!("{} bytes as {}", got.size, got.file_name);
    assert!(got.size > 1000 && got.file_name.to_lowercase().ends_with(".rar"));
}
