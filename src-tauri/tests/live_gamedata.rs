//! Backs up the game's real data folder on this computer into a temporary folder (nothing is changed). `cargo test --test live_gamedata -- --ignored --nocapture`

use genlauncher2_lib::{game::Game, gamedata, maps, profiles};

#[test]
#[ignore]
fn real_data_backs_up() {
    let documents = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap()).join("Documents");
    let data = gamedata::data_dir(&documents, Game::Zh);
    let tmp = tempfile::tempdir().unwrap();
    match gamedata::create(&data, tmp.path()) {
        Ok(b) => println!("backup of {}: {} bytes", data.display(), b.bytes),
        Err(e) => println!("no backup: {e}"),
    }
    println!("profiles (read only): {:?}", profiles::list(&data));
    let found = maps::list(&maps::maps_dir(&documents, Game::Zh));
    println!("{} maps, first: {:?}", found.len(), found.first());
}
