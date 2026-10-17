//! Runs the game health check on the game found on this computer. `cargo test --test live_health -- --ignored --nocapture`

use genlauncher2_lib::{game, health};

#[test]
#[ignore]
fn health_of_the_installed_game() {
    let installs = game::discover(&[]);
    assert!(!installs.is_empty(), "no game found on this computer");
    for i in installs {
        println!("{:?} at {}", i.game, i.dir.display());
        for f in health::check(&i, None, false) {
            println!("  {:?} {} {:?} fix={:?}", f.level, f.id, f.args, f.fix);
        }
    }
}
