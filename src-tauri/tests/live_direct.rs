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

/// The community build (a 33 MB zip with the game exe and World Builder) downloads and unpacks with the launcher's own code.
#[tokio::test]
#[ignore]
async fn the_community_build_downloads_and_unpacks() {
    let catalog: serde_json::Value = serde_json::from_str(include_str!("../bundled/zero-hour.json")).unwrap();
    let entry = catalog["executables"].as_array().unwrap().iter().find(|e| e["name"] == "Community build").expect("the catalog lists the community build");
    let url = entry["download"]["url"].as_str().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let client = reqwest::Client::builder().user_agent("GenLauncher-test").build().unwrap();
    let got = download::fetch(&client, url, &dir.path().join("d.part"), &Cancel::new(), &|_| {}, &|_| {}, false, &Options::default()).await.unwrap();
    let zip = dir.path().join(&got.file_name);
    std::fs::rename(dir.path().join("d.part"), &zip).unwrap();
    let out = dir.path().join("content");
    genlauncher2_lib::extract::extract(&zip, &out, &|| false).unwrap();
    for f in [entry["executableFileName"].as_str().unwrap(), entry["worldBuilderFileName"].as_str().unwrap()] {
        assert!(out.join(f).is_file(), "{f} is in the build");
    }
    println!("{} unpacked: {} files", got.file_name, std::fs::read_dir(&out).unwrap().count());
}
