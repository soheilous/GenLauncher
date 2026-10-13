> **Reference document.** This describes the behaviour of the old launcher (GenLauncher 1.0.1.1) that this project reproduces. The new on-disk layout is described in `README.md` (`.genlauncher/` instead of `GLM/` and `.GenLauncherFolder/`).

# GenLauncher 1.0.1.1: Reverse-Engineered Spec

Purpose: describe exactly what the current launcher does so a new one can be built from scratch with feature parity (or better). Source: `GenLauncherNet/` (WPF, .NET Framework 4.6).

## 1. What it is

A mod manager and launcher for **C&C Generals** and **Zero Hour**. It lives *inside the game folder*. Mods are stored in their own folders and **symlinked into the game directory at launch**, then unlinked after the game exits, so the game folder stays clean.

## 2. On-disk layout

| Path (relative to game folder) | Purpose |
|---|---|
| `GenLauncher.exe` | The launcher; must sit next to `generals.exe` |
| `.GenLauncherFolder/GenLauncherCfg.yaml` | All persisted state (settings + known mods). Hidden folder |
| `.GenLauncherFolder/LauncherImages/<mod>/<version>[bg]` | Cached mod art and backgrounds (extensionless files) |
| `.GenLauncherFolder/uam.jpg` | Extracted default art |
| `GLM/<Mod>/<Version>/...` | Installed mod version (old name `GenLauncherModifications` is migrated) |
| `GLM/<Mod>/Patches/<Patch>/<Version>/` | Patches for a mod |
| `GLM/<Mod>/Addons/<Addon>/<Version>/` | Addons for a mod (or for a patch) |
| `GLM/<Mod>/Executables/<Exe>/<Version>/` | Alternate game or WorldBuilder exes |
| `Vulkan/` | Vulkan wrapper DLLs (`d3d8x.dll`, ...) |
| `GlBg.png`, `Colors.yaml` | Optional user theme overrides next to the exe |
| `<name>.GLTC` folder | Temp download folder, renamed on success, deleted on startup |
| Suffix `.GLR` | "GenLauncher Replaced": the original file was renamed aside |
| Suffix `.GOF` | "Original File": backup before the `.big` camera patch |

Game detection: `generals.exe` + `BINKW32.DLL` + a window `.big` must be present. `WindowZH.big` means Zero Hour, `Window.big` means Generals. Game-owned `.big` names are hardcoded lists (~55 per game). Any other `.big` in the folder is treated as foreign and renamed aside at launch.

## 3. Startup sequence

1. Pick UI culture (`.GenLauncherFolder/eng` file forces English; otherwise OS culture).
2. Check .NET Framework 4.6, launcher is in a game folder, and **symlink creation works** (creates a test symlink; requires NTFS plus Developer Mode or admin). Abort with a message otherwise.
3. Delete `dbghelp.dll` (crash-hook leftover).
4. Single-instance mutex `"GenLauncher"`. If it is held, wait 5 s, then re-check; if still held, focus the other window and exit.
5. Splash window (`InitWindow`):
   - Create `GLM/` and `Vulkan/`.
   - **Restore the game folder**: remove all symlinks, strip `.GLR`/`.GOF` suffixes, delete `.GLTC` temp folders. This is crash recovery.
   - Detect game (ZH vs Generals), pick the manifest URL, fill the game-file whitelist.
   - Load the default theme, then apply `Colors.yaml` and `GlBg.png` overrides.
   - Rename `d3d8x.dll`, `d3d9.dll`, `d3d10core.dll` and `d3d11.dll` to `.GLR` (so only the launcher-chosen wrappers load).
   - Extract embedded resources (`7z.dll` x86/x64, default art).
   - Check connectivity by fetching the manifest. On failure, show "cannot connect" and run **offline mode**.
   - `DataHandler.InitData`: load the yaml config, scan `GLM/`, fetch repo data.
6. Main window. On exit, the game folder is restored again.

## 4. Data model

**Config (`GenLauncherCfg.yaml`, YamlDotNet)**: `LauncherData`
- Settings: `ModdedExe`, `Windowed`, `QuickStart`, `CameraHeight`, `LaunchesCount`, `AutoUpdateGentool`, `AutoDeleteOldVersions`, `GameParams`, `CheckModFiles`, `AskBeforeCheck`, `HideLauncherAfterGameStart`, `FirstStart`, `UseVulkan`.
- Lists: `Modifications`, `Addons`, `Patches`, `Exes`. Each is a list of `GameModification` (a mod with `ModificationVersions[]`, `IsSelected`, `NumberInList` for user ordering).
- A corrupt config is deleted and recreated, which silently loses all settings.

**Hierarchy**: Mod → Versions. Patch and Addon have `DependenceName` (a mod name, or a patch name for addons). Executables have a dependency on a mod or on `Original Game`. Selection state: one mod, at most one patch per mod, many addons, many exes.

**Version fields** (`ModificationReposVersion`, same schema as the remote yaml): `Name`, `Version`, `ModificationType` (Mod/Addon/Patch/Advertising/Executable), `SimpleDownloadLink`, `UIImageSourceLink`, `DiscordLink`, `ModDBLink`, `NewsLink`, `SupportLink`, `NetworkInfo`, `DependenceName`, `Deprecated`, `S3HostLink`, `S3BucketName`, `S3FolderName`, `S3HostPublicKey`, `S3HostSecretKey`, `ColorsInformation` (per-mod theme + background image link), `ExceptionNames`, `AdditionalFileNames`, `ExecutableFileName`, `ReplacesOriginalGameFile`, `ReplacesOriginalWBFile`.

**Version ordering**: strip non-digits, pad with zeros to equal length, compare as int. This is lossy (see §10).

**Installed state** is derived from disk, not trusted from the config: every launch scans `GLM/**`, adds unregistered folders as installed, and marks missing ones `Installed=false` (or deletes them if they are not in the repo).

## 5. Remote data (the "repository")

All hosted in GitHub raw yaml (`p0ls3r/GenLauncherModsData`):
- Main manifest per game: `ReposModificationDataZH4.yaml` and `ReposModificationDataGenerals3.yaml`. Fields: `LauncherVersion`, `DownloadLink` (self-update), `VulkanReposData` (URL), `modDatas[]` (`ModName`, `ModLink`, `ModPatches[]`, `ModAddons[]`), `originalGamePatches[]`, `originalGameAddons[]`, `globalAddonsData[]`, `executables[]` (`ModName`, `ModLink`, `DependencyName`), `AdvData[]` (promoted mod with images).
- Each `ModLink`/patch/addon/exe URL points to a small yaml describing the latest version (the fields above).
- Loading strategy: the manifest is fetched first. Full yaml is fetched only for **already installed** mods (cheap startup), and for the others on demand when the user opens "Add mod". Patches, addons and exes are loaded lazily when a mod is selected.
- Images are downloaded and cached by `mod/version` name.
- Other endpoints: `modded.exe` (xezon's exe, from `p0ls3r/moddedExe`), GenTool (`gentool.net`, version scraped from `var gentool_ver` in the HTML), WorldBuilder (OneDrive link), Vulkan zip.

## 6. Downloading and installing

`UpdaterFactory` chooses:
- **HttpSingleFileUpdater** (default): one archive URL. Link rewriting for Dropbox (`dl=1`) and OneDrive. It resolves the filename from `Content-Disposition` or the URL, and rejects non-zip/octet-stream content types without a disposition. It downloads into `<versionfolder>.GLTC/`, **resumes with HTTP Range** if a partial file exists, runs a stall watchdog (no progress for ~10 s means timeout, up to 5 retries), extracts `.zip/.rar/.7z` with SevenZipExtractor, **renames `*.big` to `*.gib`** (so the game never loads an unlinked archive), deletes the archive, and moves the temp folder to the final path.
- **S3Updater** (when `S3HostLink` + `S3FolderName` set): lists objects through MinIO, **copies unchanged files from the latest installed version** (delta update), downloads only changed files over HTTPS, and verifies each file by ETag/MD5, looping until it matches. It requires system clock sync (`TimeUtility`) because S3 signing fails with a skewed clock. There is a default public/secret key pair hardcoded in source (`S3StorageHandler`).
- Progress events drive the per-item progress bar. Cancel is supported. Optional "auto-delete old versions" after a successful update.

**Manual add**: the user supplies files (archive, `.big`, folder contents), a name and a version. The launcher copies them into `GLM/...` and extracts archives. A patch or addon must be assigned to a mod.

## 7. Game launch pipeline (the core logic)

Pre-flight checks (`Launch_Click`):
1. Not already running. Selected items not mid-download. Everything selected is installed.
2. At most one exe replaces the game exe (and at most one replaces WorldBuilder).
3. First run → offer to apply default `Options.ini` graphics settings.
4. If a newer version of the selected mod, patch or addon exists, warn with Continue/Cancel.
5. If a patch or addon is `Deprecated`, warn.
6. Optional **file integrity check** (S3 mods only, latest version only, if online and the setting is on, optionally asking first): link the mod, hash every relevant file (`.w3d .dds .tga .ini .scb .wnd .csf .str .big .bik`, except `.exe/.dll`) against S3 ETags, then unlink. On mismatch, abort with "mod installed incorrectly".

`PrepareGameFiles` builds the virtual game dir:
1. Walk the whole game folder. Rename foreign `.big` to `.GLR` (anything not in the game whitelist and not under the Steam subfolder). Rename loose custom files (`.w3d .dds .tga .ini .scb .wnd .csf .str`) outside `GLM/` to `.GLR`. Remove stale symlinks.
2. If no mod is selected (vanilla), restore the original `Scripts.ini`, `SkirmishScripts.scb` and `MultiplayerScripts.scb`.
3. **Camera height** (Zero Hour only, if set): find the first `.big` (sorted by name) across all selected versions that contains `Data\INI\GameData.ini`. Back it up as `.GOF`. Binary-patch the `MaxCameraHeight` value in place.
4. For each selected version (mod, patch, addons, exes), in order, mirror its tree into the game folder: files become **file symlinks**, subfolders are recreated. `.big` is linked as `.big` from the stored `.gib`. A placeholder `.big` of exactly 24 bytes is skipped unless explicitly allowed. A conflicting existing file is renamed to `.GLR`. Exes get linked at the game root, and subfolders become directory links.
5. Later layers override earlier ones (mod < patch < addons).

`RunGame`:
- Start the chosen exe (`generals.exe`, or the exe flagged `ReplacesOriginalGameFile`, or `modded.exe`) with args `-win` (windowed), `-quickstart -noshellmap` (quickstart), plus user args. Start any non-replacing helper exes too.
- Poll every 5 s until it exits. Optionally hide the launcher meanwhile.
- **Always** run `RenameGameFilesToOriginalState`: remove all symlinks, strip `.GLR`/`.GOF` suffixes.
- If the game stayed up more than a threshold, blink the "Support" button on the mod (a post-play nudge).

WorldBuilder follows the same pipeline with the WB exe (`WorldBuilderNT27.exe`, auto-downloaded if missing, or a mod-provided one).

## 8. Helper integrations

- **GenTool** (`d3d8.dll`): auto-check the version via `GetFileVersionInfo` vs the website, then download and replace. Can be disabled. `d3d8.cfg` is edited to `window=3` for windowed. Keep the default cfg embedded.
- **Vulkan (DXVK-style wrapper)**: optional, with a version manifest. It downloads into `Vulkan/` and links the DLLs into the game dir at launch. If GenTool is off, `d3d8x.dll` is linked as `d3d8.dll`.
- **modded.exe** (ZH only): xezon's build that lifts the LAN camera-height restriction. Auto-downloaded and used as the launch exe when enabled.
- **Self-update**: compare the manifest `LauncherVersion` with the local one, download the new exe as a temp file, write a `.bat` that renames the old exe, renames the new one, starts it and deletes the leftovers.
- **Advertising**: every 100 launches an "advertised" mod from the manifest is shown in the list for promotion.

## 9. UI inventory (what the new UI must cover)

**Main window**: background art (per mod or default) and a themed palette; four tabs/lists: **Mods**, **Patches**, **Addons**, **Executables** (Patches/Addons/Exes depend on the selected mod). Each item is a card with name, latest version, a versions dropdown (per-version delete), an Update/Install/Download button with progress, and info/changelog/network/support buttons. Mods list supports **drag-and-drop reordering** and a **context menu** (open game folder, mod folder, replays, maps, ModDB, Discord, change image). Bottom/side controls: Launch, World Builder, Options, Add mod (repo browser), Manual add (mod/patch/addon), Windowed toggle, QuickStart toggle, launcher version and update status, a self-update progress bar.

**Windows**: `AddModificationWindow` (browse repo mods), `ManualAddMidificationWindow`, `OptionsWindow`, `InfoWindow` (confirm/warn/error dialog), `UpdateAvailable`, `InitWindow` (splash).

**Options window**: all of `Options.ini` graphics (resolution, particle count, texture reduction, shadow volumes/decals, building occlusion, trees, cloud map, light map, soft water edge, heat effects, extra animations, dynamic LOD, alternate mouse), camera-height slider, GenTool auto-update, modded exe, Vulkan, hide launcher, check files + ask, auto-delete old versions, custom command-line args, and reset to defaults. The game's real settings file is `Documents\Command and Conquer Generals (Zero Hour) Data\Options.ini`.

**Localization**: resx for en, ru, de, es, fr, hr, pt, tr, uk, zh, ar.

## 10. Defects and design debts found (do not carry these over)

1. **Hardcoded S3 credentials** in source (`S3StorageHandler.GenInsavePKey/SKey`).
2. **Crash-unsafe design**: the game folder is mutated in place. If the launcher or PC dies mid-game, files stay renamed or symlinked until the next launcher start. There is no journal or transaction.
3. **Version comparison is wrong** for things like `1.10` vs `1.9` (digits are concatenated). The launcher self-update compare has the same flaw and overflows `int` for long versions.
4. `StartGameExe` has a unit bug: `secondsPassed` counts to `120000` but compares against `12000`, so the "play session length" heuristic is meaningless.
5. `CheckAndUpdateVulkan` inverts its folder check (`if Exists → CreateDirectory`).
6. `ColorsInfo` swaps `ListBoxSelectionColor1/2` when parsing.
7. **Busy-wait loops**: S3 listing spins `while(!finished){}`, and exe monitoring uses `Thread.Sleep` polling instead of `WaitForExit`.
8. **Sync-over-async** (`.GetAwaiter().GetResult()`) in Gentool version lookup.
9. Errors are swallowed everywhere (`catch { //TODO logger }`). There is **no log file**, so user bug reports are hard to diagnose.
10. TLS config enables **SSL3/TLS1.0/1.1**. Downloads have **no signature or hash verification** except for S3 mods, so a repo or link compromise means arbitrary code (mods ship `.exe`).
11. Self-update replaces the exe via a `.bat` with no integrity check.
12. Single-instance logic can focus *itself*, `Thread.Sleep(5000)` in startup, and static `Directory.GetCurrentDirectory()` snapshots.
13. Whole-tree scan of the game folder on every launch and startup, with a hardcoded whitelist of game `.big` names (breaks for new or localized game files).
14. A corrupt config is deleted without a backup. Settings and mod cache are mixed in one file.
15. `MainWindow.xaml.cs` is 2.7k lines of code-behind. UI, state and logic are tangled, with no tests and no MVVM.
16. The camera patch edits `.big` bytes in place via a text search from offset 3000, and fails if `MaxCameraHeight` is missing.
17. .NET Framework 4.6, `packages.config`, a Costura-fused exe, and no HiDPI work.

## 11. Proposed requirements for the new launcher

**Keep (parity)**: everything in §6–§8, the in-game-folder symlink model, manifest compatibility (so the existing mod repo keeps working), the mod/patch/addon/exe hierarchy, per-mod theming, offline mode, drag-reorder, manual add, Options editor, GenTool/Vulkan/modded.exe handling, self-update, localization.

**Fix**: items 1–17 above. In particular:
- A **launch journal**: write the list of changes to disk before mutating, and roll back on the next start or on exit (replaces the folder-wide scan).
- Structured **logging** with an in-app "copy diagnostics" button.
- **Hash/signature verification** for all downloads, and semver-style version parsing.
- Cancelable, resumable, parallel downloads with a global **download queue**.
- Separate **settings.json** from the **library cache**, with atomic writes and backups.

**New feature candidates**: profiles (saved mod+patch+addon sets, one-click launch), shortcuts to a profile, mod search/filter/tags/favorites, load-order conflict detection (which mod overrides which file), per-mod playtime and last-played, changelog viewer, disk usage and cleanup of old versions, screenshot/replay browser, a GenTool/DXVK settings panel, a first-run wizard that checks symlink permission and offers Developer Mode, and a Light/Dark Material-glass theme with a per-mod accent color derived from its art.
