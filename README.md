# GenLauncher

A mod launcher for **Command & Conquer: Generals** and **Zero Hour**. Install, update and switch between
mods, patches, addons and alternate executables, then press Play. Your game folder is restored when the game closes,
even if the launcher or the PC dies mid-game.

Rewrite of the original GenLauncher (WPF, .NET Framework) as a small native app: **Tauri 2** (Rust core) with a
**SvelteKit** UI. Mods are listed in the open [mod catalog](https://github.com/soheilous/generals-mod-catalog). Windows first, Linux (Wine/Proton) planned.

## Install

Windows 10 or 11: download `GenLauncher_<version>_x64-setup.exe` from the
[latest release](https://github.com/soheilous/GenLauncher/releases) and run it. It installs for your user account and needs
no administrator rights. The installer is not code-signed yet, so **Windows SmartScreen may say "Windows protected your PC"**:
click *More info*, then *Run anyway*. A winget package is being prepared (`packaging/`).

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

### Updates

Installed copies check `latest.json` on the `updater` release (made by `.github/workflows/update-feed.yml` when a release is
published) and install a newer setup program only if its signature matches the public key in `src-tauri/tauri.conf.json`.
Release builds are signed with the private key in the `TAURI_SIGNING_PRIVATE_KEY` secret. To build the setup program on your
own computer, `npm run tauri build` needs that variable too (any key made with `npx tauri signer generate` works for a private
build; such a build just cannot be updated from the official feed).

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
of its own: `cd website && npm install && npm run dev`. The interactive demo on its home page is the launcher's own interface built with sample data into `website/static/demo`; a pre-commit hook (husky + lint-staged, installed by `npm install`) rebuilds it and adds it to the commit whenever the interface changes, and CI checks that it is current. By hand: `npm run build:demo`. It reads the mod index and the legal pages from this repository
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

A Windows 10/11 launcher in preview. What it does today:

- **Library and play:** mods, versions side by side, patches, add-ons and executables, favourites, sorting, **presets** (one-click start, desktop shortcuts, a command for Steam), a game folder that is restored when the game closes.
- **Getting mods:** the author's page in a built-in browser, or a **direct download** where the author published one; files you already have by drag and drop; an offer to remove the old version after an update; background update checks and **Update all**.
- **The game itself:** a **game health** check with reversible fixes (the usual Steam / Windows 10-11 problems), the community-fixed game executable and World Builder from the game's source-code maintainers, a game options editor, programs that start together with the game, a **map manager**, backups of saved games and replays.
- **The launcher:** self-update with a verified signature, a tray icon (optional start with Windows), settings backups (local and in your own Google Drive), 13 languages.

Next: Linux/Wine, more of the community's requests (see the issues).

### Translations

The launcher speaks English, فارسی, Français, Español, Português (Brasil), Italiano, Deutsch, 简体中文, Русский, Українська,
Polski, Türkçe and العربية (Persian and Arabic are laid out right to left). Settings > Appearance > Language picks one;
"Automatic" follows the system language. The translations were written without native review, so corrections are very
welcome.

The code keeps the English text as the key, `t("Install")`, and each language is one file,
`src/lib/i18n/locales/<code>.ts`, mapping English to that language. A text a language lacks falls back to English.
`node tools/check-i18n.mjs` lists what is missing, stale or has wrong `{values}` per language (CI runs it);
`--missing fa` prints what Persian still lacks. To add a language, add a file, import it in
`src/lib/i18n/index.svelte.ts` and add it to `LANGUAGES`. In development, the "Pseudo (dev)" language wraps every text in
⟦ ⟧, so anything not yet translatable stands out. Messages that come from the Rust core (mostly errors) and the texts in the
mod catalog are not translated yet.

### Where mods come from

The launcher hosts no mods. Files always come from the people who published them, in one of two ways:

- **Direct download** (a bolt icon on the Discover card): the mod's author published a direct download link in their own
  GenLauncher manifest, and the catalog lists it. Pressing Install downloads that file, nothing else to click. If the link
  stops working, the launcher opens the author's page instead.
- **The author's page** (every other entry, and anything you add yourself from a link or a web page): the page (ModDB, Nexus
  Mods, a mod's own site) opens in the launcher's built-in browser, where you pick the version and click the site's own
  download button.

You can also add a file or folder from your computer. Each installed version remembers the page it was downloaded from.

### The mod index

Discover, and the patches / addons / executables listed under each mod, come from the [mod catalog](https://github.com/soheilous/generals-mod-catalog): one JSON
file per game in its own repository, read by the launcher from GitHub (a snapshot is built into every release in
`src-tauri/bundled/`, refreshed with `node tools/sync-catalog.mjs`, and the last copy fetched is cached, so it works
offline). It holds pointers (names, latest version, the page a mod is published on, cover images) and, where the author published one, the
direct download link, never a file. Versions and links are refreshed weekly from the authors' own manifests. Anyone can add or
fix an entry with a pull request there.

## Credits

- **p0ls3r** and the contributors of the original [GenLauncher](https://github.com/p0ls3r/GenLauncher) created the launcher
  this project continues, and gathered the mod list in
  [p0ls3r/GenLauncherModsData](https://github.com/p0ls3r/GenLauncherModsData). The first version of our
  [mod catalog](https://github.com/soheilous/generals-mod-catalog) was converted from that list (names, versions, pages, covers and colors; download links and
  storage details were left out on purpose). Thank you for years of work for the C&C community.
- The authors of every mod listed: the mods and their pages are theirs.
- Command & Conquer is a trademark of Electronic Arts. This is a fan project and is not affiliated with EA.

## License

[MIT](LICENSE) © soheilous and GenLauncher contributors. See [NOTICE.md](NOTICE.md) for third-party material and credits.
