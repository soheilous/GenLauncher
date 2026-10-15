# GenLauncher

A mod launcher for **Command & Conquer: Generals** and **Zero Hour**. Install, update and switch between
mods, patches, addons and alternate executables, then press Play. Your game folder is restored when the game closes,
even if the launcher or the PC dies mid-game.

Rewrite of the original GenLauncher (WPF, .NET Framework) as a small native app: **Tauri 2** (Rust core) with a
**SvelteKit** UI. Mods are listed in this repository's own [mod index](index/README.md). Windows first, Linux (Wine/Proton) planned.

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

**Installing** (`install.rs`, `download.rs`, `extract.rs`): resumable downloads with retries; archives (zip, 7z, rar) are extracted with path-traversal protection. Each install is built in `tmp/` and swapped in
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

### Google sign-in (optional feature)

Backups to Google Drive use the OAuth client in `src-tauri/src/google.rs`. Google's token endpoint also wants the client
"secret" of a desktop client (not confidential, but not stored in this repository): set `GENLAUNCHER_GOOGLE_CLIENT_SECRET`
before running (`npm run tauri dev`) or building a release. Without it the rest of the launcher works and signing in
says it is not set up. See [PRIVACY.md](PRIVACY.md) and [TERMS.md](TERMS.md).

### Website

`website/` is the project's site (landing page, searchable mod index, guide, privacy and terms), a static SvelteKit project
of its own: `cd website && npm install && npm run dev`. It reads the mod index and the legal pages from this repository
at build time. Deploying on Vercel: import the repository, set **Root Directory** to `website` and keep the detected
SvelteKit preset; nothing else is needed.

### Layout of the code

| Path | Purpose |
|---|---|
| `src-tauri/src/layout.rs` | the on-disk structure above |
| `game.rs`, `game_files.rs` | find and recognise game installs; the stock archive lists |
| `index.rs`, `catalog.rs`, `target.rs` | the mod index format; merging it with what is on disk |
| `library.rs`, `store.rs`, `version.rs` | installed-mod scan, JSON state with atomic writes, version ordering |
| `plan.rs`, `journal.rs`, `launch.rs`, `big.rs` | launching, crash-safe changes, camera-height patch |
| `download.rs`, `install.rs`, `extract.rs`, `links.rs` | downloading and installing |
| `migrate.rs` | moving from GenLauncher 1.x |
| `src/` | SvelteKit UI (`lib/app.svelte.ts` is the state, `lib/components/` the pieces) |
| `GENLAUNCHER-SPEC.md` | behaviour of the old launcher this project reproduces and improves on |
| `reference/old-launcher/` | old code and translations kept for features not yet ported |

## Status

Done: library, launching with journaled restore, installing/updating/removing, adding mods from a file, a folder, a direct link or any web page (ModDB included) through a built-in browser window that takes the download, with the source remembered next to each install, migration from 1.x, light/dark Material
UI with per-mod covers and accent colors.

Next: "check for update" from the saved page, and growing the index with the community.

Not yet: in-app graphics options editor, GenTool and Vulkan handling, World Builder download, self-update,
localisation (translations are in `reference/`) and Linux/Wine prefix selection.

### Where mods come from

The launcher hosts no mods and downloads none by itself. Mods are added from a file or folder on your computer, a direct
link, or a web page (ModDB, Nexus Mods, a mod's own site) opened in the launcher's built-in browser, where you pick the
version and click the site's own download button. Files therefore always come from the people who published them.
"Install" on a Discover entry does the same: it opens that mod's page. Each installed version remembers the page it was
downloaded from.

### The mod index

Discover, and the patches / addons / executables listed under each mod, come from [`index/`](index/README.md): one JSON
file per game, kept in this repository and read by the launcher from GitHub (a copy is built into every release, and the
last one fetched is cached, so it works offline). It holds pointers only (names, latest version, the page a mod is
published on, cover images), never a download link or a file. Anyone can add or fix an entry with a pull request; `node tools/check-index.mjs` checks the structure and that the pages still exist (it also runs weekly on GitHub).

## Credits

- **p0ls3r** and the contributors of the original [GenLauncher](https://github.com/p0ls3r/GenLauncher) created the launcher
  this project continues, and gathered the mod list in
  [p0ls3r/GenLauncherModsData](https://github.com/p0ls3r/GenLauncherModsData). The first version of our
  [mod index](index/README.md) was converted from that list (names, versions, pages, covers and colors; download links and
  storage details were left out on purpose). Thank you for years of work for the C&C community.
- The authors of every mod listed: the mods and their pages are theirs.
- Command & Conquer is a trademark of Electronic Arts. This is a fan project and is not affiliated with EA.
