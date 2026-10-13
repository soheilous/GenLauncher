# GenLauncher

A mod launcher for **Command & Conquer: Generals** and **Zero Hour**. Install, update and switch between
mods, patches, addons and alternate executables, then press Play. Your game folder is restored when the game closes,
even if the launcher or the PC dies mid-game.

Rewrite of the original GenLauncher (WPF, .NET Framework) as a small native app: **Tauri 2** (Rust core) with a
**SvelteKit** UI. Same mod catalog, so the existing mod repository keeps working. Windows first, Linux (Wine/Proton) planned.

## How it works

Everything the launcher stores in a game folder lives in one directory, `.genlauncher/`. Deleting it removes the launcher.

```text
<game folder>/.genlauncher/
  mods/<Mod>/<Version>/…                         installed mod versions
  mods/<Mod>/{patches,addons,exes}/<Name>/<Version>/…
  state/                                         settings.json, launch-journal.json, catalog-<game>.json
  tmp/                                           download staging; a version appears in mods/ only when complete
```

**Launching** (`plan.rs`, `journal.rs`): the launcher computes the full list of file changes (set aside files the
game must not see, link in the mod, patch the camera height), writes that list to disk, applies it, starts the game, and
undoes the list when the game exits. On the next start an unfinished session is undone automatically. Links are hard
links on Windows (no admin rights or Developer Mode needed) and symlinks elsewhere.

**Installing** (`install.rs`, `download.rs`, `s3.rs`, `extract.rs`): resumable downloads with retries; files from the
mod storage are verified against their checksums and unchanged files are reused from the previous version;
archives (zip, 7z, rar) are extracted with path-traversal protection. Each install is built in `tmp/` and swapped in
only when complete.

**Migrating from GenLauncher 1.x** (`migrate.rs`): on first start the launcher offers to move an old `GLM/` library into
`.genlauncher/mods` (a rename: nothing is re-downloaded) and to delete the old launcher's own files.

## Development

Prerequisites: Node.js 20+, Rust (stable), and on Windows the MSVC C++ Build Tools. On Linux: the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri dev        # run the app
npm run tauri build      # installer / bundles
npm run check            # type-check the UI
cd src-tauri && cargo test               # unit + integration tests (offline)
```

Some tests talk to the real mod repository and read a real game install. They are `#[ignore]`d and read-only
unless their name says otherwise (see `src-tauri/tests/live.rs`):

```bash
cargo test --test live -- --ignored --nocapture <test name>
```

Without Tauri (plain `npm run dev` in a browser) the UI runs on mock data, which is what the visual tests use.

### Layout of the code

| Path | Purpose |
|---|---|
| `src-tauri/src/layout.rs` | the on-disk structure above |
| `game.rs`, `game_files.rs` | find and recognise game installs; the stock archive lists |
| `manifest.rs`, `catalog.rs`, `target.rs` | the mod repository format; merging it with what is on disk |
| `library.rs`, `store.rs`, `version.rs` | installed-mod scan, JSON state with atomic writes, version ordering |
| `plan.rs`, `journal.rs`, `launch.rs`, `big.rs` | launching, crash-safe changes, camera-height patch |
| `download.rs`, `install.rs`, `s3.rs`, `sigv4.rs`, `extract.rs`, `links.rs` | downloading and installing |
| `migrate.rs` | moving from GenLauncher 1.x |
| `src/` | SvelteKit UI (`lib/app.svelte.ts` is the state, `lib/components/` the pieces) |
| `GENLAUNCHER-SPEC.md` | behaviour of the old launcher this project reproduces and improves on |
| `reference/old-launcher/` | old code and translations kept for features not yet ported |

## Status

Done: library, launching with journaled restore, installing/updating/removing, adding mods from a file, a folder, a direct link or any web page (ModDB included) through a built-in browser window that takes the download, with the source remembered next to each install, migration from 1.x, light/dark Material
UI with per-mod covers and accent colors.

Next: a community index of mod pages for Discover, "check for update" from the saved page.

Not yet: in-app graphics options editor, GenTool and Vulkan handling, World Builder download, self-update,
localisation (translations are in `reference/`), Linux/Wine prefix selection, and download access for mod storage that is
not publicly listable (see below).

### Open question: private mod storage

Most mods are hosted on the maintainers' S3 server. For 11 of the 24 S3-hosted mods the file list is public. The others
refuse anonymous listing; the old launcher read them with a key pair embedded in its source. The new launcher
supports per-mod keys from the catalog (`S3HostPublicKey` / `S3HostSecretKey`, signed with SigV4) but ships none.
