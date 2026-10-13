//! Network + real-install checks. Read-only. Run with: cargo test --test live -- --ignored --nocapture

use genlauncher2_lib::{catalog, game, library, manifest::Fetcher};

#[tokio::test]
#[ignore]
async fn real_zero_hour_catalog_and_install() {
    let installs = game::discover(&[]);
    println!("installs: {installs:#?}");
    let zh = installs.iter().find(|i| i.game == game::Game::Zh).expect("no Zero Hour install found");

    let fetcher = Fetcher::new().unwrap();
    let started = std::time::Instant::now();
    let cat = catalog::fetch(&fetcher, zh.game.manifest_url()).await.expect("catalog fetch");
    println!("catalog: {} mods ({} resolved) in {:?}", cat.manifest.mod_datas.len(), cat.mods.iter().flatten().count(), started.elapsed());
    assert!(cat.manifest.mod_datas.len() > 5);
    assert!(cat.mods.iter().flatten().count() > cat.manifest.mod_datas.len() / 2, "most mod links should resolve");

    let disk = library::scan(&zh.dir);
    println!("on disk: {:?}", disk.iter().map(|d| (&d.name, &d.versions)).collect::<Vec<_>>());

    let items = catalog::mod_items(&cat, &disk);
    let installed: Vec<_> = items.iter().filter(|i| i.versions.iter().any(|v| v.installed)).map(|i| (&i.name, &i.latest)).collect();
    println!("installed in library: {installed:?}");
    for d in &disk {
        assert!(items.iter().any(|i| i.versions.iter().any(|v| v.installed) && i.name.eq_ignore_ascii_case(&d.name) || catalog::slug(&i.name) == catalog::slug(&d.name)),
            "disk mod {} missing from library", d.name);
    }

    // Extras for the first installed mod that has a catalog entry.
    if let Some(entry) = cat.manifest.mod_datas.iter().find(|e| disk.iter().any(|d| d.name.eq_ignore_ascii_case(&e.name))) {
        let infos = catalog::fetch_extras(&fetcher, &cat.manifest, Some(entry)).await;
        let extras = catalog::extra_items(&catalog::slug(&entry.name), &infos, disk.iter().find(|d| d.name.eq_ignore_ascii_case(&entry.name)), &disk);
        println!("{}: {} patches, {} addons, {} exes", entry.name, infos.patches.len(), infos.addons.len(), infos.exes.len());
        println!("{:#?}", extras.iter().take(4).map(|x| (&x.id, &x.latest)).collect::<Vec<_>>());
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

/// Lists every S3-hosted mod in the real catalog (no downloads except one tiny file).
#[tokio::test]
#[ignore]
async fn real_storage_listing_and_one_small_download() {
    use genlauncher2_lib::{download, s3};
    let fetcher = Fetcher::new().unwrap();
    let cat = catalog::fetch(&fetcher, game::Game::Zh.manifest_url()).await.unwrap();
    let client = reqwest::Client::new();
    let mut smallest: Option<(String, String, String, s3::Object)> = None;
    let (mut mods, mut files, mut bytes, mut multipart) = (0, 0u64, 0u64, 0);
    for info in cat.mods.iter().flatten() {
        let (Some(h), Some(b), Some(f)) = (&info.s3_host, &info.s3_bucket, &info.s3_folder) else { continue };
        let objs = match s3::list(&client, &s3::list_base(h), b, f, None).await {
            Ok(o) => o,
            Err(e) => { println!("  {} -> LIST FAILED: {e}", info.name); continue }
        };
        mods += 1;
        files += objs.len() as u64;
        bytes += objs.iter().map(|o| o.size).sum::<u64>();
        multipart += objs.iter().filter(|o| o.md5().is_none()).count();
        for o in &objs {
            assert!(!o.rel.contains("..") && !o.rel.contains('\\') &&!o.rel.contains(':'), "suspicious name {:?} in {}", o.rel, info.name);
            if o.size > 0 && smallest.as_ref().is_none_or(|s| o.size < s.3.size) && o.md5().is_some() {
                smallest = Some((h.clone(), b.clone(), f.clone(), o.clone()));
            }
        }
        println!("{:<28} {:>5} files {:>9.1} MB", info.name, objs.len(), objs.iter().map(|o| o.size).sum::<u64>() as f64 / 1e6);
    }
    println!("TOTAL: {mods} mods, {files} files, {:.1} GB, {multipart} multipart-etag files", bytes as f64 / 1e9);
    assert!(mods >= 8 && files > 300, "public listings should work for at least the public buckets");

    let (h, b, f, o) = smallest.expect("no object");
    let url = s3::object_url(&s3::file_base(&h), &b, &f, &o.rel).unwrap();
    println!("downloading smallest file: {url} ({} bytes)", o.size);
    let dir = tempfile::tempdir().unwrap();
    let got = download::fetch(&client, &url, &dir.path().join("x.part"), &download::Cancel::new(), &|_| {}, &|_| {}, true, &download::Options::default()).await.unwrap();
    assert_eq!(got.size, o.size);
    assert_eq!(got.md5.as_deref(), o.md5(), "downloaded bytes must match the server's ETag");
    println!("checksum OK over https");
}

/// Prints (never executes) what migrating the real install would do.
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
