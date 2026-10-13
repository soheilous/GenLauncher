//! Network + real-install checks. Read-only. Run with: cargo test --test live -- --ignored --nocapture

use genlauncher2_lib::{catalog, game, library};

#[tokio::test]
#[ignore]
async fn real_zero_hour_catalog_and_install() {
    let installs = game::discover(&[]);
    println!("installs: {installs:#?}");
    let zh = installs.iter().find(|i| i.game == game::Game::Zh).expect("no Zero Hour install found");

    let http = reqwest::Client::new();
    let started = std::time::Instant::now();
    let cat = catalog::fetch(&http, zh.game.index_url()).await.expect("index fetch");
    println!("index: {} mods in {:?}", cat.index.mods.len(), started.elapsed());
    assert!(cat.index.mods.len() > 5);

    let disk = library::scan(&zh.dir);
    println!("on disk: {:?}", disk.iter().map(|d| (&d.name, &d.versions)).collect::<Vec<_>>());

    let items = catalog::mod_items(&cat, &disk);
    let installed: Vec<_> = items.iter().filter(|i| i.versions.iter().any(|v| v.installed)).map(|i| (&i.name, &i.latest)).collect();
    println!("installed in library: {installed:?}");
    for d in disk.iter().filter(|d| !d.versions.is_empty()) {
        assert!(items.iter().any(|i| i.versions.iter().any(|v| v.installed) && i.name.eq_ignore_ascii_case(&d.name) || catalog::slug(&i.name) == catalog::slug(&d.name)),
            "disk mod {} missing from library", d.name);
    }
}

/// Builds (never applies) the launch plan for the user's real install and prints a summary.
#[test]
#[ignore]
fn dry_run_plan_on_real_install() {
    use genlauncher2_lib::{journal::Op, launch, plan, store::Settings};
    let installs = game::discover(&[]);
    let zh = installs.iter().find(|i| i.game == game::Game::Zh).expect("no Zero Hour install");
    let disk = library::scan(&zh.dir);
    let settings = Settings { active_mod: Some("contra".into()), camera_height: 0, ..Default::default() };
    let resolved = launch::resolve(&zh.dir, &settings, &disk, &Default::default(), launch::Target::Game).expect("resolve");
    println!("layers: {:#?}", resolved.layers.iter().map(|l| &l.root).collect::<Vec<_>>());
    let ops = plan::build(&zh.dir, zh.game, &resolved.layers, 0).expect("plan");
    let (mut parks, mut dirs, mut links, mut patches) = (vec![], 0, 0, 0);
    for o in &ops {
        match o {
            Op::Park { from, .. } => parks.push(from.strip_prefix(&zh.dir).unwrap().to_string_lossy().into_owned()),
            Op::MkDir { .. } => dirs += 1,
            Op::Link { .. } => links += 1,
            Op::Patch { .. } => patches += 1,
        }
    }
    println!("would park {} files, create {dirs} folders, link {links} files, patch {patches}", parks.len());
    println!("parked files: {parks:#?}");
    assert!(!journal_exists(&zh.dir), "dry run must not leave a journal");
}

fn journal_exists(dir: &std::path::Path) -> bool {
    genlauncher2_lib::journal::journal_path(dir).exists()
}

#[test]
#[ignore]
fn migrate_dry_run_on_real_install() {
    use genlauncher2_lib::migrate;
    let zh = game::discover(&[]).into_iter().find(|i| i.game == game::Game::Zh).expect("no Zero Hour install");
    let plan = migrate::plan(&zh.dir);
    println!("game folder: {}", zh.dir.display());
    println!("{} mods to move, {:.2} GB", plan.mods_to_move, plan.bytes_to_move as f64 / 1e9);
    for s in &plan.steps {
        println!("  {}", match s {
            migrate::Step::Move { from, to } => format!("MOVE    {}  ->  {}", rel(&zh.dir, from), rel(&zh.dir, to)),
            migrate::Step::RemoveFile { path } => format!("DELETE  file  {}", rel(&zh.dir, path)),
            migrate::Step::RemoveDir { path } => format!("DELETE  dir   {}", rel(&zh.dir, path)),
            migrate::Step::RemoveDirIfEmpty { path } => format!("DELETE  dir (only if empty)  {}", rel(&zh.dir, path)),
        });
    }
    for n in &plan.notes {
        println!("  NOTE    {n}");
    }
}

fn rel(base: &std::path::Path, p: &std::path::Path) -> String {
    p.strip_prefix(base).unwrap_or(p).display().to_string()
}

/// DESTRUCTIVE: runs the migration on the real install. Only with GENLAUNCHER_MIGRATE_REAL=yes.
#[test]
#[ignore]
fn migrate_real_install_execute() {
    use genlauncher2_lib::migrate;
    assert_eq!(std::env::var("GENLAUNCHER_MIGRATE_REAL").as_deref(), Ok("yes"), "refusing to modify a real install");
    let zh = game::discover(&[]).into_iter().find(|i| i.game == game::Game::Zh).expect("no Zero Hour install");
    let plan = migrate::plan(&zh.dir);
    let report = migrate::execute(&plan);
    println!("steps done: {} of {}; error: {:?}", report.steps_done, plan.steps.len(), report.error);
    assert!(report.error.is_none());
    assert!(migrate::plan(&zh.dir).is_empty(), "nothing left to migrate");
}
