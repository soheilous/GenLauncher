import {
  addItem, backend, cloudBackupCreate, cloudBackupDelete, cloudBackupList, cloudBackupRestore, profilesList, profileFiles, profileMove, profileDelete, profileOpen, type SaveProfile, type SaveFile, gamedataList, gamedataBackup, gamedataRestore, gamedataDelete, gamedataOpenFolder, type GameDataBackup, onExtraProgramFailed, osNotify, mapsList, mapsImport, mapsRemove, mapsOpenFolder, pickArchiveFile, pickModFolder as pickFolderForMaps, type MapInfo, takePendingPlay, launcherCommand, createShortcut, onPlayPreset, healthCheck, healthFix, backgroundSet, autostartStatus, autostartSet, onTrayPlay, renameVersion, updateCheck, updateInstall, type UpdateInfo, googleSignIn, googleSignOut, googleStatus, backupCreate, backupDelete, backupList, backupRestore, cancelInstall, exportConfig, onFileDrop, importConfig, loadAnnouncements, pickConfigFile, pickConfigSavePath, confirmDialog, isTauri, launchGame, migrateLegacy, onCloseBlocked, onInstallFinished,
  openInBrowser, openItemFolder,
  onInstallProgress, onLaunchFinished, pickFolder, uninstallVersion, type InstallFinished, type InstallProgress,
  type AddRequest, type Backup, type CloudBackup, type LaunchTarget,
} from "./backend";
import { storeKey } from "./env";
import { needsAttention, type Finding } from "./health";
import type { Preset } from "./types";
import pkg from "../../package.json";
import { slugify } from "./guess";
import { N, i18n, t, tn } from "./i18n/index.svelte";
import { mockItems } from "./mock";
import type { Bootstrap, Game, GameInstall, Item, LegacySummary, ModKind, Notice, NoticeLevel, Settings } from "./types";

/** What "Install" on a catalog entry hands to the Add dialog. */
export interface AddPreset {
  name: string;
  /** Where to get it (opened in the built-in browser), if the catalog knows. */
  pageUrl: string | null;
  /** Another version of something already installed: no "same name" confirmation needed. */
  update: boolean;
  /** The catalog entry this is for: its name is fixed (the dialog does not let it be edited) and its newest version is what is expected. */
  fixedName?: boolean;
  expectedVersion?: string;
  /** A file or folder already chosen (dropped onto the window). */
  file?: string;
  /** Nothing says what it is (a dropped file), so the dialog asks: mod, patch, add-on or executable, and for which mod. */
  askType?: boolean;
}

export type View = "library" | "discover" | "maps" | "saves" | "downloads" | "notifications";
/** The catalog's community-fixed game executable (an executable of the original game). */
const COMMUNITY_BUILD = "original-game/exe/community-build";

export type LibrarySort = "name" | "nameDesc" | "updates" | "recent";
export type DiscoverSort = "name" | "nameDesc" | "direct";
export type LaunchState = "idle" | "preparing" | "running";
export type Theme = "dark" | "light";
/** loading: talking to the core; setup: no game folder yet; ready: library available */
export type Phase = "loading" | "setup" | "ready";

const PLACEHOLDER: Item = {
  id: "none", kind: "mod", name: "GenLauncher", summary: "", hue: 262, versions: [], latest: "",
};

const DEFAULT_SETTINGS: Settings = {
  windowed: true, quickStart: true, cameraHeight: 0, gameParams: "", checkModFiles: true, askBeforeCheck: true,
  hideLauncherWhilePlaying: false, gentoolAutoUpdate: true, autoDeleteOldVersions: false, moddedExe: true,
  useVulkan: false, extraPrograms: [], savesByMod: false, activeMod: null, enabled: [], chosenVersions: {},
};

function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(storeKey(key));
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}
function save(key: string, value: unknown) {
  try { localStorage.setItem(storeKey(key), JSON.stringify(value)); } catch { /* storage unavailable */ }
}
const gameName = (g: string) => (g === "zh" ? t("Zero Hour") : t("Generals"));
const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));
/** A stable accent hue for something without art yet. */
const hueOf = (s: string) => [...s].reduce((h, c) => (h * 33 + c.charCodeAt(0)) % 360, 7);

class AppState {
  // --- library ---
  items = $state<Item[]>(isTauri ? [] : structuredClone(mockItems));
  phase = $state<Phase>(isTauri ? "loading" : "ready");
  installs = $state<GameInstall[]>([]);
  active = $state<GameInstall | null>(null);
  online = $state(true);
  catalogError = $state<string | null>(null);
  loadingExtras = $state(false);
  private extrasLoaded = new Set<string>();

  // --- ui ---
  view = $state<View>("library");
  game = $state<Game>("zh");
  search = $state("");
  activeModId = $state(isTauri ? "" : "rotr");
  launch = $state<LaunchState>("idle");
  settingsOpen = $state(false);
  optionsOpen = $state(false);
  toast = $state<string | null>(null);
  /** Everything the launcher told the user, newest first (kept between runs). */
  notices = $state<Notice[]>(load<Notice[]>("notices", []));
  unread = $derived(this.notices.filter((n) => !n.read).length);
  theme = $state<Theme>("dark");
  accentFromMod = $state(true);
  sidebarCollapsed = $state(false);
  coverBackdrop = $state(true);
  /** User-chosen covers keyed by item id. Persisted in localStorage for now. */
  customCovers = $state<Record<string, { cover: string; hue: number }>>({});

  // --- game settings (persisted by the Rust core per game folder) ---
  windowed = $state(true);
  quickStart = $state(true);
  cameraHeight = $state(0);
  enabled = $state<string[]>([]);
  chosenVersions = $state<Record<string, string>>({});
  private rest: Settings = { ...DEFAULT_SETTINGS }; // settings the UI does not edit yet; kept so saves don't drop them

  mods = $derived(this.items.filter((i) => i.kind === "mod"));
  activeMod = $derived(this.mods.find((m) => m.id === this.activeModId) ?? this.mods[0] ?? PLACEHOLDER);
  children = $derived(this.items.filter((i) => i.parent === this.activeMod.id));
  downloading = $derived(this.items.filter((i) => i.progress !== undefined));
  filteredMods = $derived(
    this.mods.filter((m) => m.name.toLowerCase().includes(this.search.trim().toLowerCase()))
  );

  // --- sorting and favourites ---
  librarySort = $state<LibrarySort>(load("librarySort", "name"));
  discoverSort = $state<DiscoverSort>(load("discoverSort", "name"));
  /** Ids of the mods the user starred; they stay at the top of the library. */
  favourites = $state<string[]>(load("favourites", []));
  favouritesOnly = $state(load("favouritesOnly", false));
  /** When each mod was last started (ms since 1970), for "Recently played". */
  lastPlayed = $state<Record<string, number>>(load("lastPlayed", {}));

  setLibrarySort(v: LibrarySort) { this.librarySort = v; save("librarySort", v); }
  setDiscoverSort(v: DiscoverSort) { this.discoverSort = v; save("discoverSort", v); }
  setFavouritesOnly(v: boolean) { this.favouritesOnly = v; save("favouritesOnly", v); }
  isFavourite(id: string) { return this.favourites.includes(id); }
  toggleFavourite(id: string) {
    this.favourites = this.isFavourite(id) ? this.favourites.filter((f) => f !== id) : [...this.favourites, id];
    save("favourites", this.favourites);
  }

  private byName = (a: Item, b: Item) => a.name.localeCompare(b.name, undefined, { sensitivity: "base", numeric: true });
  private hasUpdate = (m: Item) => m.versions.some((v) => v.installed) && !(m.versions.find((v) => v.v === m.latest)?.installed ?? false);

  // Original Game is always listed first in the library, then the favourites, then the chosen order
  installedMods = $derived.by(() => {
    const sort = this.librarySort;
    const rank = (m: Item) => (m.id === "original-game" ? 0 : this.isFavourite(m.id) ? 1 : 2);
    const order = (a: Item, b: Item) => {
      if (sort === "nameDesc") return this.byName(b, a);
      if (sort === "updates") return Number(this.hasUpdate(b)) - Number(this.hasUpdate(a)) || this.byName(a, b);
      if (sort === "recent") return (this.lastPlayed[b.id] ?? 0) - (this.lastPlayed[a.id] ?? 0) || this.byName(a, b);
      return this.byName(a, b);
    };
    return this.filteredMods
      .filter((m) => m.versions.some((v) => v.installed) && (!this.favouritesOnly || this.isFavourite(m.id) || m.id === "original-game"))
      .toSorted((a, b) => rank(a) - rank(b) || order(a, b));
  });
  availableMods = $derived.by(() => {
    const sort = this.discoverSort;
    return this.filteredMods
      .filter((m) => !m.versions.some((v) => v.installed))
      .toSorted((a, b) => {
        if (sort === "nameDesc") return this.byName(b, a);
        if (sort === "direct") return Number(!!b.direct) - Number(!!a.direct) || this.byName(a, b);
        return this.byName(a, b);
      });
  });

  async init() {
    const stored = load<Theme | null>("theme", null);
    this.theme = stored ?? (matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark");
    this.accentFromMod = load("accentFromMod", true);
    this.sidebarCollapsed = load("sidebarCollapsed", false);
    this.coverBackdrop = load("coverBackdrop", true);
    this.customCovers = load("customCovers", {});

    if (!isTauri) {
      const s = load<Settings>("settings", DEFAULT_SETTINGS);
      this.applySettings({ ...DEFAULT_SETTINGS, ...s });
      this.applyCovers();
      this.applyTheme();
      return;
    }
    this.applyTheme();
    this.listen();
    try {
      await this.useBootstrap(await backend.bootstrap());
    } catch (e) {
      this.say(t("Could not start: {error}", { error: errText(e) }), "error");
      this.phase = "setup";
    }
  }

  private listening = false;
  /** Events from the Rust core about the running game session. */
  private listen() {
    if (this.listening) return;
    this.listening = true;
    void onLaunchFinished((r) => {
      this.launch = "idle";
      for (const w of r.warnings) console.warn("restore:", w);
      if (r.error) this.say(r.error);
      else {
        const m = Math.floor(r.playedSecs / 60);
        const played = m >= 1 ? t("{n} min", { n: m }) : t("{n} s", { n: r.playedSecs });
        this.say(r.warnings.length ? t("Game closed after {time}. Game folder restored ({n} notes in the log).", { time: played, n: r.warnings.length }) : t("Game closed after {time}. Game folder restored.", { time: played }));
      }
    });
    void onTrayPlay(() => void this.play());
    void onExtraProgramFailed((p) => this.say(t("A program you listed did not start: {path}", { path: p }), "error"));
    void this.syncBackground();
    void this.refreshAutostart();
    void onCloseBlocked(() => this.say(t("The game is still running. Close it first so your game folder can be restored.")));
    void onInstallProgress((p) => {
      this.installProgress[p.id] = p;
      this.applyProgress();
    });
    void onInstallFinished((r) => void this.installFinished(r));
    void onFileDrop((e) => {
      if (e.type === "enter" || e.type === "over") this.dragging = this.phase === "ready" && !this.addDialog && !this.optionsOpen;
      else this.dragging = false;
      if (e.type === "drop" && e.paths.length && this.phase === "ready" && !this.addDialog && !this.optionsOpen) {
        if (this.view === "maps") void this.importMaps(e.paths[0]);
        else this.addDropped(e.paths);
      }
    });
  }

  /** True while something is being dragged over the window. */
  dragging = $state(false);

  /** A file or folder dropped onto the window: open the Add dialog with it chosen. */
  private addDropped(paths: string[]) {
    const first = paths[0];
    if (/\.(png|jpe?g|webp|gif|bmp|ico)$/i.test(first)) {
      this.say(t("That is an image. To change a mod's cover, use the Change cover button on its panel."));
      return;
    }
    if (paths.length > 1) this.say(t("One at a time: adding the first of the dropped files."));
    this.view = "library";
    this.openAdd("mod", null, { name: "", pageUrl: null, update: false, file: first, askType: true });
  }

  /** Installs run in the Rust core; this mirrors their progress onto whichever items are on screen. */
  installProgress = $state<Record<string, InstallProgress>>({});

  /** The version being installed for an item that was added by hand (it can differ from the item's `latest`). */
  pending = $state<Record<string, { version: string }>>({});
  /** What to show as the version of an item that is installing: the one being added, else its latest. */
  versionLabel(item: Pick<Item, "id" | "latest">) {
    return this.pending[item.id]?.version ?? item.latest;
  }

  phaseLabel(item: Pick<Item, "phase">) {
    return t(({ preparing: N("Preparing"), downloading: N("Downloading"), verifying: N("Checking files"), extracting: N("Extracting"), finishing: N("Finishing") } as Record<string, string>)[item.phase ?? ""] ?? N("Working"));
  }

  private applyProgress() {
    for (const item of this.items) {
      const p = this.installProgress[item.id];
      item.progress = p ? (p.total > 0 ? Math.min(1, p.done / p.total) : 0) : undefined;
      item.phase = p?.phase;
      item.bytes = p && p.total === 0 && p.done > 0 ? p.done : undefined;
    }
  }

  /** The versions an item had installed when an install of it started (to offer removing the old ones after an update). */
  private installedBefore: Record<string, string[]> = {};
  /** After an update, ask whether to remove the old version(s). */
  askRemoveOld = $state(load("askRemoveOld", true));
  setAskRemoveOld(v: boolean) { this.askRemoveOld = v; save("askRemoveOld", v); }

  private async offerRemoveOld(item: Item, version: string, olderBefore: string[]) {
    const older = olderBefore.filter((v) => item.versions.some((x) => x.v === v && x.installed));
    if (!older.length) return;
    const msg = t("{name} {version} installed", { name: item.name, version }) + "\n"
      + tn(older.length, "Remove the older version ({versions})?", "Remove the older versions ({versions})?", { versions: older.join(", ") }) + "\n"
      + t("Its files are deleted from GenLauncher's folder; your game files are not touched.");
    if (!(await confirmDialog(msg, t("Old versions")))) return;
    for (const v of older) {
      try {
        await uninstallVersion(item.id, v);
        this.say(t("{name} {version} removed", { name: item.name, version: v }));
      } catch (e) {
        this.say(errText(e), "error");
      }
    }
    if (older.includes(this.chosenVersions[item.id])) this.chosenVersions[item.id] = version;
    this.persist();
    await this.refreshLibrary();
  }

  /** Items whose install started from the catalog's direct link: if it fails, the author's page is the way to get it. */
  private directStarted: Record<string, Item> = {};

  private async installFinished(r: InstallFinished) {
    delete this.installProgress[r.id];
    delete this.pending[r.id];
    const direct = this.directStarted[r.id];
    delete this.directStarted[r.id];
    const switchOn = this.enableAfterInstall.delete(r.id) && r.ok;
    this.applyProgress();
    const name = this.items.find((i) => i.id === r.id)?.name ?? "Item";
    if (!r.ok) {
      // a placeholder shown for something the user added (it has no versions yet) must not linger
      this.items = this.items.filter((i) => !(i.id === r.id && i.versions.length === 0));
    }
    if (r.cancelled) {
      this.say(t("{name}: installation cancelled. Progress is kept if you try again.", { name }));
      return;
    }
    if (!r.ok) {
      this.say(t("{name}: {error}", { name, error: r.error ?? t("installation failed") }), "error");
      if (direct?.links?.moddb) {
        // the author's link did not work (moved, removed): fall back to the page the author publishes the mod on
        this.notify(t("The direct download of {name} did not work. Its page is open instead.", { name }), "info");
        this.openAdd(direct.kind, direct.kind === "mod" ? null : (direct.parent ?? this.activeModId), { name: direct.name, pageUrl: direct.links.moddb, update: direct.versions.some((v) => v.installed), fixedName: true, expectedVersion: direct.latest });
      }
      return;
    }
    this.say(t("{name} {version} installed", { name, version: r.version ?? "" }).replace(/\s+/g, " ").trim(), "success");
    await this.refreshLibrary();
    if (switchOn) {
      this.useCommunityBuild(true);
      this.notify(t("The community build is installed and switched on for the original game. You can switch it off in Game health."), "success");
    }
    const older = (this.installedBefore[r.id] ?? []).filter((v) => v !== r.version);
    delete this.installedBefore[r.id];
    const item = this.items.find((i) => i.id === r.id);
    // an update: the catalog's newest version was just installed while older ones are still there
    if (this.askRemoveOld && r.version && item && item.latest === r.version && older.length) await this.offerRemoveOld(item, r.version, older);
  }

  // --- "Add mod": things the user picked or linked themselves ---
  addDialog = $state<{ kind: ModKind; parent: string | null; preset?: AddPreset } | null>(null);

  /** `preset`: something the launcher already knows about (a Discover entry): its name and the page to fetch it from. */
  openAdd(kind: ModKind = "mod", parent: string | null = null, preset?: AddPreset) {
    this.addDialog = { kind, parent, preset };
  }
  /** Reopen the page an installed version came from, ready to add a newer version of it. */
  checkForUpdate(item: Item, pageUrl: string) {
    this.openAdd(item.kind, item.kind === "mod" ? null : (item.parent ?? this.activeModId), { name: item.name, pageUrl, update: true, fixedName: true, expectedVersion: item.latest });
  }

  closeAdd() {
    this.addDialog = null;
  }

  /** Start installing what the dialog collected. Resolves true when it was accepted. */
  async add(req: AddRequest): Promise<boolean> {
    const name = req.name.trim();
    if (!isTauri) {
      // browser preview: pretend it worked so the flow can be seen
      const id = req.kind === "mod" ? slugify(name) : `${req.parent}/${req.kind}/${slugify(name)}`;
      const version = req.version.trim();
      const existing = this.items.find((i) => i.id === id);
      if (existing) {
        existing.versions.unshift({ v: version, installed: true }); // another version of the same item
        existing.latest = version;
      } else {
        this.items.push({ id, kind: req.kind, name, summary: "", hue: hueOf(name), versions: [{ v: version, installed: true }], latest: version, parent: req.kind === "mod" ? null : req.parent });
      }
      this.say(t("{name} {version} added (preview)", { name, version }));
      return true;
    }
    try {
      const id = await addItem(req);
      this.installedBefore[id] = (this.items.find((i) => i.id === id)?.versions ?? []).filter((v) => v.installed).map((v) => v.v);
      this.installProgress[id] = { id, phase: "preparing", done: 0, total: 0 };
      this.pending[id] = { version: req.version.trim() };
      if (!this.items.some((i) => i.id === id)) {
        // shown while it installs; replaced by the real item when the library is re-read
        this.items.push({ id, kind: req.kind, name, summary: "", hue: hueOf(name), versions: [], latest: req.version.trim(), parent: req.kind === "mod" ? null : req.parent });
      }
      this.applyProgress();
      this.view = "downloads";
      return true;
    } catch (e) {
      this.say(errText(e), "error");
      return false;
    }
  }

  /** Open a web page in the user's own browser. */
  async openPage(url: string) {
    try {
      if (isTauri) await openInBrowser(url);
      else window.open(url, "_blank");
    } catch {
      this.say(t("Could not open your browser. The address is: {url}", { url }));
    }
  }

  /** Show the installed files of `item` (the given version) in the file manager. */
  async openFolder(item: Item, version?: string) {
    if (!isTauri) {
      this.say(t("Opening folders works in the desktop app."));
      return;
    }
    try {
      await openItemFolder(item.id, version);
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  /** Re-read the library from disk (after an install or removal) keeping the current mod selected. */
  async refreshLibrary() {
    this.extrasLoaded.clear();
    if (!isTauri) return;
    try {
      // disk only: the catalog was fetched at startup, no need to download it again
      this.items = await backend.catalogReload();
      this.applyCovers();
      this.applyProgress();
      this.applyTheme();
      await this.ensureExtras(this.activeModId);
    } catch {
      await this.loadCatalog(); // e.g. the catalog was never loaded: do the full load
    }
  }

  private applySettings(s: Settings) {
    this.rest = { ...s };
    this.savesByMod = s.savesByMod ?? false;
    this.windowed = s.windowed;
    this.quickStart = s.quickStart;
    this.cameraHeight = s.cameraHeight;
    this.enabled = [...s.enabled];
    this.chosenVersions = { ...s.chosenVersions };
    if (s.activeMod) this.activeModId = s.activeMod;
  }

  private async useBootstrap(b: Bootstrap) {
    this.installs = b.installs;
    this.active = b.active;
    this.extrasLoaded.clear();
    if (!b.active) {
      this.phase = "setup";
      return;
    }
    this.game = b.active.game;
    this.applySettings(b.settings);
    if (b.recovered) {
      const r = b.recovered;
      for (const w of r.warnings) console.warn("restore:", w);
      this.say(r.opsUndone > 0
        ? t("The last session did not finish cleanly. Your game folder has been restored.")
        : t("Could not fully restore the game folder: {problem}", { problem: r.warnings[0] ?? t("unknown problem") }));
    }
    this.phase = "loading";
    if (b.legacy) await this.offerMigration(b.legacy); // the library is scanned fresh right after
    await this.loadCatalog();
  }

  /** Old GenLauncher 1.x data found: explain exactly what will happen, then do it only on a yes. */
  private async offerMigration(l: LegacySummary): Promise<boolean> {
    const gb = (l.bytes / 1e9).toFixed(1);
    const msg =
      t("Data from the old GenLauncher was found in this game folder.") + "\n\n" +
      t("• {n} installed mods ({gb} GB) will be MOVED into the new library. They are kept, not re-downloaded.", { n: l.mods, gb }) + "\n" +
      t("• {n} leftover files and folders of the old launcher (its program, settings and caches) will be deleted.", { n: l.deletions }) + "\n" +
      t("• Your game files, GenTool and Steam files are not touched.") +
      (l.notes.length ? "\n\n" + t("Left alone:") + "\n" + l.notes.map((n) => "• " + n).join("\n") : "");
    if (!(await confirmDialog(msg, t("Move to the new library"), t("Move and clean up"), t("Not now")))) return false;
    try {
      await migrateLegacy();
      this.say(t("Your library was moved and the old launcher's files were removed."));
      return true;
    } catch (e) {
      this.say(errText(e), "error");
      return false;
    }
  }

  async loadCatalog() {
    try {
      const res = await backend.catalogLoad();
      this.items = res.items;
      this.online = res.online;
      this.catalogError = res.error;
      if (!res.online) this.notify((t("Could not reach the mod catalog, showing the saved copy.") + " " + (res.error ?? "")).trim(), "error", { id: "index-offline" });
      if (!this.mods.some((m) => m.id === this.activeModId)) {
        this.activeModId = (this.mods.find((m) => m.versions.some((v) => v.installed)) ?? this.mods[0])?.id ?? "";
      }
      this.applyCovers();
      this.applyProgress();
      this.phase = "ready";
      this.applyTheme();
      void this.checkAnnouncements();
      void this.checkUpdate(false);
      void this.quietHealthCheck();
      void this.startPendingPreset();
      if (isTauri) {
        setTimeout(() => void this.checkModUpdates(), 30_000);
        setInterval(() => void this.checkModUpdates(), 6 * 3600_000);
      }
      void this.refreshGoogle();
      await this.ensureExtras(this.activeModId);
    } catch (e) {
      this.say(t("Could not load the library: {error}", { error: errText(e) }), "error");
      this.phase = "setup";
    }
  }

  /** Patches/addons/executables are fetched per mod the first time it is selected. */
  async ensureExtras(modId: string) {
    if (!isTauri || !modId || this.extrasLoaded.has(modId)) return;
    this.extrasLoaded.add(modId);
    this.loadingExtras = true;
    try {
      const extras = await backend.loadExtras(modId);
      for (const x of extras) x.selected = this.enabled.includes(x.id);
      this.items = [...this.items.filter((i) => i.parent !== modId), ...extras];
      this.applyProgress();
    } catch (e) {
      this.extrasLoaded.delete(modId);
      this.say(t("Could not load add-ons: {error}", { error: errText(e) }), "error");
    } finally {
      this.loadingExtras = false;
    }
  }

  async chooseFolder() {
    const dir = await pickFolder();
    if (dir) await this.openInstall(dir);
  }

  async openInstall(dir: string) {
    try {
      this.phase = "loading";
      await this.useBootstrap(await backend.selectInstall(dir));
    } catch (e) {
      this.say(errText(e), "error");
      this.phase = this.active ? "ready" : "setup";
    }
  }

  /** Games map to install folders; switching games switches folders. */
  installFor(game: Game) {
    return this.installs.find((i) => i.game === game);
  }
  async switchGame(game: Game) {
    if (!isTauri) { this.game = game; return; }
    const target = this.installFor(game);
    if (target && target.game !== this.active?.game) await this.openInstall(target.dir);
  }

  private applyCovers() {
    for (const [id, c] of Object.entries(this.customCovers)) {
      const item = this.items.find((i) => i.id === id);
      if (item) { item.cover = c.cover; item.hue = c.hue; }
    }
  }

  private currentSettings(): Settings {
    return {
      ...this.rest,
      savesByMod: this.savesByMod,
      windowed: this.windowed,
      quickStart: this.quickStart,
      cameraHeight: this.cameraHeight,
      enabled: [...this.enabled],
      chosenVersions: { ...this.chosenVersions },
      activeMod: this.activeModId || null,
    };
  }

  private saveTimer: ReturnType<typeof setTimeout> | undefined;
  /** Debounced: sliders and rapid toggles produce one write. */
  persist() {
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => void this.flush().catch(() => {}), 300);
  }

  /** Write settings now (a launch must see exactly what is on screen). `auto: false` skips the automatic Drive backup the write can trigger. */
  async flush(auto = true) {
    clearTimeout(this.saveTimer);
    const s = this.currentSettings();
    if (!isTauri) { save("settings", s); return; }
    try {
      await backend.saveSettings(s);
      if (auto) this.maybeAutoCloudBackup();
    } catch (e) {
      this.say(t("Could not save settings: {error}", { error: errText(e) }), "error");
      throw e;
    }
  }

  // --- configuration files ---

  /** The look-and-feel part of a configuration file. */
  private uiBundle(includeCovers: boolean) {
    return {
      theme: this.theme, sidebarCollapsed: this.sidebarCollapsed, accentFromMod: this.accentFromMod, coverBackdrop: this.coverBackdrop,
      presets: this.presets, favourites: this.favourites,
      ...(includeCovers ? { customCovers: this.customCovers } : {}),
    };
  }

  /** Take over what a configuration file says about the look, ignoring anything of the wrong type. */
  private applyUiBundle(ui: unknown) {
    if (!ui || typeof ui !== "object") return;
    const u = ui as Record<string, unknown>;
    if (u.theme === "dark" || u.theme === "light") save("theme", u.theme);
    for (const k of ["sidebarCollapsed", "accentFromMod", "coverBackdrop"]) if (typeof u[k] === "boolean") save(k, u[k]);
    // presets and favourites of a configuration file are added to the ones here (the same preset id is not added twice)
    if (Array.isArray(u.presets)) {
      const have = new Set(this.presets.map((p) => p.id));
      const ok = (p: unknown): p is Preset => {
        const x = p as Preset;
        return !!x && typeof x.id === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(x.id) && typeof x.name === "string" && (x.game === "zh" || x.game === "generals")
          && typeof x.mod === "string" && Array.isArray(x.enabled) && x.enabled.every((e) => typeof e === "string") && typeof x.windowed === "boolean" && typeof x.quickStart === "boolean";
      };
      save("presets", [...this.presets, ...(u.presets as unknown[]).filter(ok).filter((p) => !have.has(p.id))]);
    }
    if (Array.isArray(u.favourites)) save("favourites", [...new Set([...this.favourites, ...u.favourites.filter((f): f is string => typeof f === "string")])]);
    if (u.customCovers && typeof u.customCovers === "object") {
      const covers: Record<string, { cover: string; hue: number }> = {};
      for (const [id, c] of Object.entries(u.customCovers as Record<string, { cover?: unknown; hue?: unknown }>)) {
        // only images and numbers: a configuration file must not be able to put anything else into a cover
        if (typeof c?.cover === "string" && /^data:image\//.test(c.cover) && typeof c.hue === "number") covers[id] = { cover: c.cover, hue: c.hue };
      }
      save("customCovers", { ...this.customCovers, ...covers });
    }
  }

  /** Save the settings of every game on this computer, and the look, to a file the user chooses. */
  async exportConfiguration(includeCovers: boolean) {
    if (!isTauri) { this.say(t("Exporting works in the desktop app.")); return; }
    try {
      const path = await pickConfigSavePath();
      if (!path) return;
      await this.flush();
      const games = await exportConfig(path, this.uiBundle(includeCovers));
      this.say(t("Configuration exported ({games})", { games: games.map(gameName).join(", ") || t("look only") }), "success");
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  /** Replace this launcher's settings with those from a file (asks first), then restart the screen. */
  backups = $state<Backup[]>([]);

  // --- saved games and replays per mod version ---
  /** (a $state of its own: `rest` is a plain object, so a switch bound to it would not redraw) */
  savesByMod = $state(false);
  setSavesByMod(v: boolean) { this.savesByMod = v; this.persist(); void this.refreshProfiles(); }
  profiles = $state<SaveProfile[]>([]);
  profileSelected = $state("");
  profileFileList = $state<SaveFile[]>([]);

  /** What a profile key means to the user: "Contra 009 Beta 3", "Original Game", or the files from before the launcher kept them apart. */
  profileLabel(key: string): string {
    if (key === "everything-before") return t("Everything from before the launcher kept them apart");
    if (key === "original-game") return t("Original Game");
    const at = key.indexOf("@");
    const id = at < 0 ? key : key.slice(0, at);
    const mod = this.mods.find((m) => m.id === id);
    return at < 0 ? (mod?.name ?? key) : `${mod?.name ?? id} ${key.slice(at + 1)}`;
  }

  /** The profiles one can put a save into: every one that exists, and every installed mod version. */
  profileTargets(): string[] {
    const keys = new Set(this.profiles.map((p) => p.key));
    keys.add("original-game");
    for (const m of this.mods) for (const v of m.versions) if (v.installed && m.id !== "original-game") keys.add(`${m.id}@${v.v}`);
    return [...keys];
  }

  async refreshProfiles() {
    if (!isTauri) {
      this.profiles = [{ key: "contra@009 Beta 3", active: true, saves: 3, replays: 12, bytes: 2_400_000, modified: 1_760_000_000 }, { key: "everything-before", active: false, saves: 7, replays: 31, bytes: 9_100_000, modified: 1_750_000_000 }];
    } else {
      try { this.profiles = await profilesList(); } catch (e) { this.say(errText(e), "error"); return; }
    }
    if (!this.profiles.some((p) => p.key === this.profileSelected)) this.profileSelected = this.profiles[0]?.key ?? "";
    await this.refreshProfileFiles();
  }

  async selectProfile(key: string) { this.profileSelected = key; await this.refreshProfileFiles(); }

  async refreshProfileFiles() {
    if (!this.profileSelected) { this.profileFileList = []; return; }
    if (!isTauri) { this.profileFileList = [{ kind: "save", name: "Mission 3.sav", bytes: 410_000, modified: 1_760_000_000 }, { kind: "replay", name: "Last Replay.rep", bytes: 120_000, modified: 1_759_000_000 }]; return; }
    try { this.profileFileList = await profileFiles(this.profileSelected); } catch { this.profileFileList = []; }
  }

  async moveSaveFile(f: SaveFile, to: string) {
    if (!to || to === this.profileSelected) return;
    if (!isTauri) { this.say(t("Moving works in the desktop app.")); return; }
    try { await profileMove(this.profileSelected, f.kind, f.name, to); await this.refreshProfiles(); } catch (e) { this.say(errText(e), "error"); }
  }

  async deleteSaveFile(f: SaveFile) {
    if (!(await confirmDialog(t("Delete {name} from your computer?", { name: f.name }), t("Delete {name}", { name: f.name }), t("Delete"), t("Keep")))) return;
    if (!isTauri) { this.profileFileList = this.profileFileList.filter((x) => x !== f); return; }
    try { await profileDelete(this.profileSelected, f.kind, f.name); await this.refreshProfiles(); } catch (e) { this.say(errText(e), "error"); }
  }

  async openProfileFolder() {
    try { await profileOpen(this.profileSelected); } catch (e) { this.say(errText(e), "error"); }
  }

  // --- the game's saved games, replays and settings ---
  gameData = $state<GameDataBackup[]>([]);
  gameDataBusy = $state(false);

  async refreshGameData() {
    if (!isTauri) { this.gameData = [{ name: "backup-1760000000.zip", at: 1_760_000_000, bytes: 3_400_000 }]; return; }
    try { this.gameData = await gamedataList(); } catch { this.gameData = []; }
  }

  async backUpGameData() {
    if (!isTauri) { this.say(t("Backing up works in the desktop app.")); return; }
    this.gameDataBusy = true;
    try { await gamedataBackup(); this.say(t("Saved games, replays and settings backed up."), "success"); await this.refreshGameData(); }
    catch (e) { this.say(errText(e), "error"); }
    finally { this.gameDataBusy = false; }
  }

  async restoreGameData(b: GameDataBackup) {
    const when = i18n.dateTime(b.at * 1000);
    const msg = t("Put back the saved games, replays and settings backed up on {when}?", { when }) + "\n" + t("Files with the same name are replaced. What is there now is backed up first, so you can undo this.");
    if (!(await confirmDialog(msg, t("Restore backup"), t("Restore"), t("Cancel")))) return;
    this.gameDataBusy = true;
    try { await gamedataRestore(b.name); this.say(t("Saved games, replays and settings restored."), "success"); await this.refreshGameData(); }
    catch (e) { this.say(errText(e), "error"); }
    finally { this.gameDataBusy = false; }
  }

  async deleteGameData(b: GameDataBackup) {
    try { await gamedataDelete(b.name); await this.refreshGameData(); } catch (e) { this.say(errText(e), "error"); }
  }

  async openGameDataFolder() {
    try { await gamedataOpenFolder(); } catch (e) { this.say(errText(e), "error"); }
  }

  // --- updates of mods, patches and add-ons ---
  /** Look in the background (every few hours) for newer versions of what is installed, and say so once per version. */
  backgroundUpdates = $state(load("backgroundUpdates", true));
  private notifiedUpdates: Record<string, string> = load("notifiedUpdates", {});

  setBackgroundUpdates(v: boolean) { this.backgroundUpdates = v; save("backgroundUpdates", v); if (v) void this.checkModUpdates(); }

  /** Installed things whose newest version in the catalog is not installed. */
  outdated = $derived(
    this.items.filter((i) => i.id !== "original-game" && !i.deprecated && i.versions.some((v) => v.installed) && !(i.versions.find((v) => v.v === i.latest)?.installed ?? false))
  );
  /** Of those, the ones the author offers as a direct download: they can all be updated with one click. */
  updatable = $derived(this.outdated.filter((i) => i.direct));

  /** Fetch the catalog again, read what the installed mods have, and tell the user about versions they have not been told of. */
  async checkModUpdates() {
    if (!isTauri || !this.backgroundUpdates || this.phase !== "ready" || this.launch !== "idle") return;
    try {
      await backend.catalogLoad();
      await this.refreshLibrary();
      for (const m of this.mods.filter((x) => x.id !== "original-game" && x.versions.some((v) => v.installed))) await this.ensureExtras(m.id);
    } catch { return; /* offline: try again later */ }
    const fresh = this.outdated.filter((i) => this.notifiedUpdates[i.id] !== i.latest);
    if (!fresh.length) return;
    for (const i of fresh) this.notifiedUpdates[i.id] = i.latest;
    save("notifiedUpdates", this.notifiedUpdates);
    const names = fresh.slice(0, 4).map((i) => `${i.name} ${i.latest}`).join(", ") + (fresh.length > 4 ? "…" : "");
    const text = tn(fresh.length, "A newer version is available: {names}", "{n} newer versions are available: {names}", { names });
    this.notify(text, "info", { id: `updates:${fresh.map((i) => i.id + i.latest).join("|")}`.slice(0, 200) });
    // the window is hidden or in the background: a Windows notification too
    if (!document.hasFocus()) { try { await osNotify("GenLauncher", text); } catch { /* the in-app message is there */ } }
  }

  /** Update everything that can be updated with one click. */
  async updateAll() {
    const list = this.updatable.filter((i) => !this.installProgress[i.id]);
    if (!list.length) return;
    const names = list.map((i) => `${i.name} ${i.latest}`);
    const msg = tn(list.length, "Update this to its newest version?", "Update these {n} to their newest versions?") + "\n" + names.slice(0, 12).join("\n") + (names.length > 12 ? "\n…" : "");
    if (!(await confirmDialog(msg, t("Update all"), t("Update"), t("Cancel")))) return;
    for (const i of list) await this.install(i);
  }

  // --- custom maps ---
  maps = $state<MapInfo[]>([]);
  /** The maps as picture cards or as a list. */
  mapsView = $state<"cards" | "list">(load("mapsView", "cards"));
  setMapsView(v: "cards" | "list") { this.mapsView = v; save("mapsView", v); }
  mapsBusy = $state(false);

  async refreshMaps() {
    if (!isTauri) { this.maps = [{ name: "Alpine Assault", bytes: 4_200_000, modified: 1_760_000_000 }, { name: "Desert Crossing", bytes: 1_100_000, modified: 1_750_000_000 }]; return; }
    this.mapsBusy = true;
    try { this.maps = await mapsList(); } catch (e) { this.say(errText(e), "error"); } finally { this.mapsBusy = false; }
  }

  async importMaps(path: string) {
    if (!isTauri) { this.say(t("Adding maps works in the desktop app.")); return; }
    this.mapsBusy = true;
    try {
      const r = await mapsImport(path);
      const n = r.added.length + r.replaced.length;
      this.say(tn(n, "{n} map added", "{n} maps added", { n }) + (r.replaced.length ? " " + t("({count} replaced an older copy)", { count: r.replaced.length }) : ""), "success");
      this.maps = await mapsList();
    } catch (e) {
      this.say(errText(e), "error");
    } finally {
      this.mapsBusy = false;
    }
  }

  /** Pick a pack (a zip, 7z or rar file, or a folder) and add its maps. */
  async addMaps(folder: boolean) {
    const path = isTauri ? (folder ? await pickFolderForMaps() : await pickArchiveFile()) : "C:\Downloads\Maps.zip";
    if (path) await this.importMaps(path);
  }

  async removeMap(name: string) {
    if (!(await confirmDialog(t("Delete the map {name} from your computer?", { name }), t("Delete {name}", { name }), t("Delete"), t("Keep")))) return;
    if (!isTauri) { this.maps = this.maps.filter((m) => m.name !== name); return; }
    try { await mapsRemove(name); this.maps = await mapsList(); } catch (e) { this.say(errText(e), "error"); }
  }

  async openMapsFolder() {
    try { await mapsOpenFolder(); } catch (e) { this.say(errText(e), "error"); }
  }

  // --- presets ---
  presets = $state<Preset[]>(load("presets", []));

  private savePresets() { save("presets", this.presets); }

  /** Remember what is switched on right now (the active mod, its version and extras, windowed, quick start) under a name. */
  addPreset(name: string): Preset | null {
    const n = name.trim();
    if (!n || !this.activeModId) return null;
    const mod = this.activeModId;
    const p: Preset = {
      id: "p" + Date.now().toString(36) + Math.random().toString(36).slice(2, 5),
      name: n,
      game: this.game,
      mod,
      version: this.chosenVersions[mod],
      enabled: this.enabled.filter((id) => id.startsWith(mod + "/")),
      windowed: this.windowed,
      quickStart: this.quickStart,
    };
    this.presets = [...this.presets, p];
    this.savePresets();
    return p;
  }

  removePreset(id: string) { this.presets = this.presets.filter((p) => p.id !== id); this.savePresets(); }
  renamePreset(id: string, name: string) {
    const n = name.trim();
    if (!n) return;
    this.presets = this.presets.map((p) => (p.id === id ? { ...p, name: n } : p));
    this.savePresets();
  }

  /** Put a preset's choices on screen (switching game and mod when needed). Returns false when its mod is not installed. */
  async applyPreset(p: Preset): Promise<boolean> {
    if (p.game !== this.game) await this.switchGame(p.game);
    await this.ensureExtras(p.mod);
    const mod = this.mods.find((m) => m.id === p.mod);
    if (!mod || !mod.versions.some((v) => v.installed)) {
      this.say(t("{name} is not installed, so the preset \"{preset}\" cannot start.", { name: mod?.name ?? p.mod, preset: p.name }), "error");
      return false;
    }
    await this.selectMod(p.mod);
    if (p.version) this.chosenVersions[p.mod] = p.version;
    // only the extras that are still installed are switched on
    const installed = new Set(this.items.filter((i) => i.parent === p.mod && i.versions.some((v) => v.installed)).map((i) => i.id));
    const want = p.enabled.filter((id) => installed.has(id));
    this.enabled = [...this.enabled.filter((id) => !id.startsWith(p.mod + "/")), ...want];
    for (const i of this.items) if (i.parent === p.mod) i.selected = this.enabled.includes(i.id);
    this.windowed = p.windowed;
    this.quickStart = p.quickStart;
    this.persist();
    if (want.length < p.enabled.length) this.say(t("Some parts of the preset \"{preset}\" are no longer installed and were left out.", { preset: p.name }));
    return true;
  }

  async playPreset(id: string) {
    const p = this.presets.find((x) => x.id === id);
    if (!p) { this.say(t("That preset does not exist (any more)."), "error"); return; }
    if (await this.applyPreset(p)) await this.play("game");
  }

  /** The desktop shortcut that starts a preset. */
  async makeShortcut(p: Preset) {
    try {
      await createShortcut(p.id, p.name);
      this.say(t("A shortcut for \"{preset}\" is on your desktop.", { preset: p.name }), "success");
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  /** The command line to paste into Steam (Add a non-Steam game, Launch options) so a preset starts from Steam. */
  async commandFor(p: Preset): Promise<string> {
    try { return await launcherCommand(p.id); } catch (e) { this.say(errText(e), "error"); return ""; }
  }

  /** Started with a shortcut (`--play <id>`): play that preset as soon as the library is ready. */
  private async startPendingPreset() {
    if (!isTauri) return;
    void onPlayPreset((id) => void this.playPreset(id));
    try {
      const id = await takePendingPlay();
      if (id) await this.playPreset(id);
    } catch { /* nothing pending */ }
  }

  // --- game health ---
  healthOpen = $state(false);
  health = $state<Finding[]>([]);
  healthBusy = $state(false);

  /** The community-fixed game executable, as a suggestion, when the catalog offers it as a direct download. */
  private async communityBuildFinding(): Promise<Finding | null> {
    await this.ensureExtras("original-game");
    const item = this.items.find((i) => i.id === COMMUNITY_BUILD);
    if (!item?.direct) return null;
    const have = item.versions.find((v) => v.installed)?.v;
    if (!have) return { id: "community-build", level: "info", args: { version: item.latest }, fix: "install-community-build" };
    if (have !== item.latest) return { id: "community-build-update", level: "info", args: { version: item.latest, current: have }, fix: "update-community-build" };
    if (this.enabled.includes(COMMUNITY_BUILD)) return { id: "community-build-active", level: "info", args: { version: have }, fix: "stop-community-build" };
    return { id: "community-build-installed", level: "info", args: {}, fix: "use-community-build" };
  }

  /** Items to switch on as soon as their install finishes (the community build, when it was installed from the health check). */
  private enableAfterInstall = new Set<string>();

  private useCommunityBuild(on: boolean) {
    this.enabled = this.enabled.filter((id) => !id.startsWith("original-game/exe/"));
    if (on) this.enabled = [...this.enabled, COMMUNITY_BUILD];
    for (const i of this.items) if (i.id.startsWith("original-game/exe/")) i.selected = on && i.id === COMMUNITY_BUILD;
    this.persist();
  }

  async runHealthCheck() {
    this.healthBusy = true;
    try {
      if (isTauri) {
        const found = await healthCheck();
        const build = await this.communityBuildFinding();
        this.health = build ? [...found.filter((f) => f.id !== "all-good"), build] : found;
      } else {
        this.health = this.sampleHealth;
      }
    } catch (e) {
      this.say(errText(e), "error");
    } finally {
      this.healthBusy = false;
    }
  }

  /** What the preview (no desktop app) shows. */
  private sampleHealth: Finding[] = [
    { id: "dbghelp", level: "warn", args: {}, fix: "disable-dbghelp" },
    { id: "registry-missing", level: "info", args: {}, fix: null },
    { id: "gentool", level: "info", args: {}, fix: null },
  ];

  async applyHealthFix(fix: string) {
    if (fix.endsWith("-community-build")) { await this.applyCommunityBuildFix(fix); return; }
    this.healthBusy = true;
    try {
      if (isTauri) await healthFix(fix);
      else this.sampleHealth = fix === "disable-dbghelp" ? [{ id: "dbghelp-disabled", level: "info", args: {}, fix: "restore-dbghelp" }, ...this.sampleHealth.slice(1)] : [{ id: "dbghelp", level: "warn", args: {}, fix: "disable-dbghelp" }, ...this.sampleHealth.slice(1)];
      this.health = isTauri ? await healthCheck() : this.sampleHealth;
    } catch (e) {
      this.say(errText(e), "error");
    } finally {
      this.healthBusy = false;
    }
  }

  private async applyCommunityBuildFix(fix: string) {
    const item = this.items.find((i) => i.id === COMMUNITY_BUILD);
    if (!item) return;
    if (fix === "use-community-build" || fix === "stop-community-build") {
      this.useCommunityBuild(fix === "use-community-build");
      await this.runHealthCheck();
      return;
    }
    // install or update: it is switched on once it has arrived
    this.enableAfterInstall.add(COMMUNITY_BUILD);
    this.healthOpen = false;
    await this.install(item);
  }

  /** At start-up: look quietly, and tell the user once when something is likely to cause trouble. */
  async quietHealthCheck() {
    if (!isTauri) return;
    try {
      const found = needsAttention(await healthCheck());
      if (!found.length) return;
      const key = found.map((f) => f.id).sort().join(",");
      this.notify(t("Your game folder has {n} things that may cause trouble. Open Settings, Game health, to see them.", { n: found.length }), "info", { id: `health:${key}` });
    } catch { /* a check that cannot run is not worth a message */ }
  }

  // --- running in the background ---
  /** Closing the window keeps the launcher running in the tray. */
  closeToTray = $state(load("closeToTray", true));
  autostart = $state(false);

  /** Tell the core the tray setting and the tray menu's words (in the chosen language). */
  async syncBackground() {
    if (!isTauri) return;
    try { await backgroundSet(this.closeToTray, t("Open GenLauncher"), t("Play"), t("Quit")); } catch { /* the tray is a convenience */ }
  }

  setCloseToTray(v: boolean) {
    this.closeToTray = v;
    save("closeToTray", v);
    void this.syncBackground();
  }

  async refreshAutostart() {
    if (!isTauri) return;
    try { this.autostart = await autostartStatus(); } catch { /* unknown */ }
  }

  async setAutostart(v: boolean) {
    try {
      await autostartSet(v);
      this.autostart = v;
    } catch (e) {
      this.say(errText(e), "error");
      await this.refreshAutostart();
    }
  }

  // --- updating GenLauncher ---
  update = $state<UpdateInfo | null>(null);
  updateBusy = $state(false);
  /** The version of this build (package.json is bumped with every release, together with the Rust and Tauri files). */
  readonly appVersion: string = pkg.version;

  /** Ask whether a newer version is published. Quiet at start-up; with `manual` it also says when there is none. */
  async checkUpdate(manual: boolean) {
    if (!isTauri) return;
    if (manual) this.updateBusy = true;
    try {
      this.update = await updateCheck();
      if (this.update) this.notify(t("GenLauncher {version} is available. You can update in Settings.", { version: this.update.version }), "info", { id: `update:${this.update.version}` });
      else if (manual) this.say(t("You have the newest version."), "success");
    } catch (e) {
      if (manual) this.say(errText(e), "error");
    } finally {
      this.updateBusy = false;
    }
  }

  async installUpdate() {
    this.updateBusy = true;
    try {
      await this.flush();
      await updateInstall(); // the launcher closes itself and the setup program starts the new version
    } catch (e) {
      this.say(errText(e), "error");
      this.updateBusy = false;
    }
  }

  // --- Google Drive ---
  google = $state<{ email: string } | null>(null);
  cloudBackups = $state<CloudBackup[]>([]);
  googleBusy = $state(false);
  autoCloud = $state(load("autoCloud", true));

  private googleFailed(e: unknown) {
    const msg = errText(e);
    if (/sign-in has expired/i.test(msg)) this.google = null; // the saved sign-in no longer works
    this.say(msg, "error");
  }

  async refreshGoogle() {
    if (!isTauri) return;
    try { this.google = await googleStatus(); } catch { this.google = null; }
    if (this.google) void this.refreshCloudBackups();
  }

  async refreshCloudBackups() {
    if (!isTauri || !this.google) return;
    try {
      this.cloudBackups = await cloudBackupList();
    } catch (e) {
      // the list is refreshed whenever Settings opens; a failure is told once, not on every look
      if (/sign-in has expired/i.test(errText(e))) this.google = null;
      this.notify(errText(e), "error", { id: "cloud-list-failed" });
    }
  }

  async signInGoogle() {
    if (!isTauri) { this.say(t("Signing in works in the desktop app.")); return; }
    this.googleBusy = true;
    try {
      this.google = await googleSignIn();
      this.notify(t("Signed in to Google as {account}.", { account: this.google.email || t("your account") }), "success");
      await this.refreshCloudBackups();
      const newest = this.cloudBackups[0];
      if (newest) {
        // probably a new computer, or a reinstall: offer to take the saved settings
        const when = i18n.dateTime(newest.at * 1000);
        if (await confirmDialog(t("Your Google Drive has a settings backup from {when}.", { when }) + "\n" + t("Restore it? Your current settings are backed up first."), t("Restore from Google Drive"), t("Restore"), t("Keep mine"))) {
          await this.restoreCloudBackup(newest, false);
        }
      } else {
        await this.cloudBackUpNow("signin", false);
      }
    } catch (e) {
      this.googleFailed(e);
    } finally {
      this.googleBusy = false;
    }
  }

  async signOutGoogle() {
    try { await googleSignOut(); } catch { /* signed out locally anyway */ }
    this.google = null;
    this.cloudBackups = [];
    this.say(t("Signed out of Google. Backups already in your Drive are kept."));
  }

  setAutoCloud(v: boolean) { this.autoCloud = v; save("autoCloud", v); }

  private cloudRunning = false;

  async cloudBackUpNow(label = "manual", withCovers = false, announce = true) {
    if (!this.google || this.cloudRunning) return;
    this.cloudRunning = true;
    try {
      await this.flush(false); // writing the settings must not start another automatic backup
      await cloudBackupCreate(this.uiBundle(withCovers), label);
      save("cloudHash", this.settingsFingerprint());
      save("cloudAt", Date.now());
      if (announce) this.say(t("Backed up to Google Drive"), "success");
      await this.refreshCloudBackups();
    } catch (e) {
      // an automatic backup that fails (no network, Google down) is reported once, quietly, not on every attempt
      if (announce) this.googleFailed(e);
      else {
        if (/sign-in has expired/i.test(errText(e))) this.google = null;
        this.notify(errText(e), "error", { id: "cloud-auto-failed" });
      }
    } finally {
      this.cloudRunning = false;
    }
  }

  async deleteCloudBackup(b: CloudBackup) {
    try { await cloudBackupDelete(b.id); await this.refreshCloudBackups(); } catch (e) { this.googleFailed(e); }
  }

  async restoreCloudBackup(b: CloudBackup, ask = true) {
    const when = i18n.dateTime(b.at * 1000);
    if (ask) {
      const msg = t("Go back to the settings saved on {when}?", { when }) + "\n" + t("Your current settings are backed up first, so you can undo this. Mods and game files are not touched.");
      if (!(await confirmDialog(msg, t("Restore backup"), t("Restore"), t("Cancel")))) return;
    }
    try {
      await this.flush();
      const r = await cloudBackupRestore(b.id, this.uiBundle(false));
      this.applyUiBundle(r.ui);
      this.notify(t("Settings restored from Google Drive ({when}).", { when }), "success");
      location.reload();
    } catch (e) {
      this.googleFailed(e);
    }
  }

  private settingsFingerprint() {
    return JSON.stringify({ s: this.currentSettings(), ui: this.uiBundle(false) });
  }

  /** After the settings changed: keep Drive up to date, at most every half hour and only when something is different. */
  private maybeAutoCloudBackup() {
    if (!this.google || !this.autoCloud || this.googleBusy) return;
    // the last attempt counts, successful or not: an unreachable Google is not asked again for half an hour
    if (Date.now() - Math.max(load("cloudAt", 0), load("cloudTry", 0)) < 30 * 60 * 1000) return;
    if (load("cloudHash", "") === this.settingsFingerprint()) return;
    save("cloudTry", Date.now());
    void this.cloudBackUpNow("auto", false, false);
  }

  async refreshBackups() {
    if (!isTauri) return;
    try { this.backups = await backupList(); } catch { /* the list is only a convenience */ }
  }

  async backUpNow(includeCovers: boolean) {
    if (!isTauri) { this.say(t("Backups work in the desktop app.")); return; }
    try {
      await this.flush();
      await backupCreate(this.uiBundle(includeCovers));
      this.say(t("Backup saved"), "success");
      await this.refreshBackups();
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  async deleteBackup(b: Backup) {
    try { await backupDelete(b.name); await this.refreshBackups(); } catch (e) { this.say(errText(e), "error"); }
  }

  /** Go back to a backup (asks first). The current settings are backed up before they are replaced. */
  async restoreBackup(b: Backup) {
    const when = i18n.dateTime(b.at * 1000);
    const msg = t("Go back to the settings saved on {when}?", { when }) + "\n" + t("Your current settings are backed up first, so you can undo this. Mods and game files are not touched.");
    if (!(await confirmDialog(msg, t("Restore backup"), t("Restore"), t("Cancel")))) return;
    try {
      await this.flush();
      const r = await backupRestore(b.name, this.uiBundle(false));
      this.applyUiBundle(r.ui);
      this.notify(t("Settings restored from the backup of {when}.", { when }), "success");
      location.reload();
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  async importConfiguration() {
    if (!isTauri) { this.say(t("Importing works in the desktop app.")); return; }
    try {
      const path = await pickConfigFile();
      if (!path) return;
      const msg = t("Replace your current settings with the ones in this file?") + "\n" + t("Your current settings are saved as a backup first. Mods and game files are not touched.");
      if (!(await confirmDialog(msg, t("Import configuration"), t("Import"), t("Cancel")))) return;
      await this.flush();
      const r = await importConfig(path, this.uiBundle(false));
      this.applyUiBundle(r.ui);
      const names = r.games.map(gameName).join(", ") || t("look only");
      this.notify(t("Configuration imported ({games}).", { games: names }), "success");
      location.reload(); // every screen re-reads its settings from the files
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  /** Launch options kept in the settings file but edited in the game options window. */
  get gameParams() { return this.rest.gameParams; }
  get hideLauncher() { return this.rest.hideLauncherWhilePlaying; }
  get extraPrograms() { return this.rest.extraPrograms ?? []; }
  setLaunchExtras(gameParams: string, hideLauncherWhilePlaying: boolean, extraPrograms: { path: string; onlyWindowed: boolean }[] = this.extraPrograms) {
    this.rest = { ...this.rest, gameParams, hideLauncherWhilePlaying, extraPrograms };
    this.persist();
  }

  toggleSidebar() { this.sidebarCollapsed = !this.sidebarCollapsed; save("sidebarCollapsed", this.sidebarCollapsed); }

  /** Set a custom cover for a mod; its dominant hue becomes the mod's accent. */
  setCover(id: string, cover: string, hue: number) {
    const item = this.items.find((i) => i.id === id);
    if (!item) return;
    item.cover = cover; item.hue = hue;
    this.customCovers[id] = { cover, hue };
    try {
      localStorage.setItem(storeKey("customCovers"), JSON.stringify(this.customCovers));
    } catch {
      this.say(t("Cover applied, but it is too large to remember after restart"));
    }
    this.applyTheme();
  }

  async resetCover(id: string) {
    delete this.customCovers[id];
    save("customCovers", this.customCovers);
    if (isTauri) {
      await this.loadCatalog(); // brings back the catalog's own art and hue
    } else {
      const item = this.items.find((i) => i.id === id);
      const original = mockItems.find((i) => i.id === id);
      if (item && original) { item.cover = original.cover; item.hue = original.hue; }
    }
    this.applyTheme();
  }

  applyTheme() {
    document.documentElement.dataset.theme = this.theme;
    const hue = this.accentFromMod ? this.activeMod.hue : 262;
    document.documentElement.style.setProperty("--accent-h", String(hue));
  }

  setTheme(t: Theme) { this.theme = t; save("theme", t); this.applyTheme(); }
  toggleTheme() { this.setTheme(this.theme === "dark" ? "light" : "dark"); }

  setPref<K extends "windowed" | "quickStart" | "cameraHeight" | "accentFromMod" | "coverBackdrop">(k: K, v: this[K]) {
    this[k] = v;
    if (k === "accentFromMod" || k === "coverBackdrop") save(k, v);
    else this.persist();
    this.applyTheme();
  }

  async selectMod(id: string) {
    this.activeModId = id;
    this.applyTheme();
    this.persist();
    await this.ensureExtras(id);
  }

  setChosenVersion(modId: string, v: string) {
    this.chosenVersions[modId] = v;
    this.persist();
  }

  /** Show a short message and keep it on the Notifications page. */
  say(msg: string, level: NoticeLevel = "info") {
    this.notify(msg, level);
    this.toast = msg;
    setTimeout(() => { if (this.toast === msg) this.toast = null; }, 3600);
  }

  /** Keep a message on the Notifications page without a pop-up. */
  notify(text: string, level: NoticeLevel = "info", extra: { id?: string; title?: string; link?: string | null; at?: number } = {}) {
    const id = extra.id ?? `n${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
    if (this.notices.some((n) => n.id === id)) return;
    const at = extra.at ?? Date.now();
    // the same message twice within a few seconds (a retry loop, a double click) is one entry
    if (!extra.id && this.notices[0]?.text === text && at - this.notices[0].at < 5000) return;
    const unreadNow = this.view !== "notifications";
    this.notices = [{ id, at, level, text, title: extra.title, link: extra.link ?? null, read: !unreadNow }, ...this.notices].sort((a, b) => b.at - a.at).slice(0, 200);
    this.saveNotices();
  }

  private saveNotices() { save("notices", this.notices); }
  markAllRead() {
    if (!this.unread) return;
    this.notices = this.notices.map((n) => (n.read ? n : { ...n, read: true }));
    this.saveNotices();
  }
  removeNotice(id: string) { this.notices = this.notices.filter((n) => n.id !== id); this.saveNotices(); }
  clearNotices() { this.notices = []; this.saveNotices(); }

  /** Messages from the project that this person has not seen yet. */
  async checkAnnouncements() {
    if (!isTauri) return;
    try {
      for (const a of await loadAnnouncements()) {
        const when = Date.parse(a.date);
        this.notify(a.text, "news", { id: `news:${a.id}`, title: a.title || undefined, link: a.link, at: Number.isNaN(when) ? Date.now() : when });
      }
    } catch { /* best effort */ }
  }

  toggleChild(item: Item) {
    if (!item.versions.some((v) => v.installed)) { this.install(item); return; }
    const turnOn = !this.enabled.includes(item.id);
    if (turnOn && (item.kind === "patch" || item.kind === "exe")) {
      // only one patch (and one game exe) can be active at a time
      for (const sib of this.children) {
        if (sib.kind === item.kind && sib.id !== item.id) sib.selected = false;
      }
      this.enabled = this.enabled.filter((id) => !this.children.some((c) => c.id === id && c.kind === item.kind));
    }
    item.selected = turnOn;
    this.enabled = turnOn ? [...this.enabled, item.id] : this.enabled.filter((id) => id !== item.id);
    this.persist();
  }

  /**
   * "Install" / "Update" on a catalog entry. The launcher never downloads a mod by itself: it opens the mod's own page
   * in the built-in browser so the download comes from the people who published it.
   */
  async install(item: Item, version?: string) {
    if (isTauri) {
      if (this.installProgress[item.id]) return;
      // a version other than the newest, when the catalog lists it (with its own page and download)
      const other = version && version !== item.latest ? item.versions.find((v) => v.v === version) : undefined;
      const wanted = other?.v ?? item.latest;
      const direct = other ? other.direct : item.direct;
      const page = (other ? other.page : null) || item.links?.moddb || null;
      if (direct) {
        // the author published a direct link: no page to visit
        const parent = item.kind === "mod" ? null : (item.parent ?? this.activeModId);
        this.directStarted[item.id] = item;
        const ok = await this.add({ source: { type: "link", url: direct.url, pageUrl: page }, name: item.name, version: wanted, kind: item.kind, parent });
        if (!ok) delete this.directStarted[item.id];
        return;
      }
      this.openAdd(item.kind, item.kind === "mod" ? null : (item.parent ?? this.activeModId), {
        name: item.name,
        pageUrl: page,
        update: item.versions.some((v) => v.installed),
        fixedName: true,
        expectedVersion: wanted,
      });
      return;
    }
    if (item.progress !== undefined) return;
    item.progress = 0;
    item.phase = "downloading";
    const tick = setInterval(() => {
      item.progress = Math.min(1, (item.progress ?? 0) + 0.04 + Math.random() * 0.06);
      if (item.progress >= 1) {
        clearInterval(tick);
        item.progress = undefined;
        const latest = item.versions.find((v) => v.v === item.latest);
        if (latest) latest.installed = true; else item.versions.unshift({ v: item.latest, installed: true });
        this.say(t("{name} {version} installed", { name: item.name, version: item.latest }));
      }
    }, 140);
  }

  async cancel(item: Item) {
    if (!isTauri) { item.progress = undefined; return; }
    try { await cancelInstall(item.id); } catch (e) { this.say(errText(e), "error"); }
  }

  /** Remove one installed version from disk (asks first). */
  /** Give an installed version another name. Returns false when it could not be renamed (the reason is shown). */
  async renameVersion(item: Item, version: string, newName: string): Promise<boolean> {
    const name = newName.trim();
    if (!name || name === version) return true;
    if (!isTauri) {
      const v = item.versions.find((x) => x.v === version);
      if (v) v.v = name;
      if (this.chosenVersions[item.id] === version) this.chosenVersions[item.id] = name;
      return true;
    }
    try {
      const final = await renameVersion(item.id, version, name);
      if (this.chosenVersions[item.id] === version) this.chosenVersions[item.id] = final;
      this.persist();
      this.say(t("{name} {version} renamed to {to}", { name: item.name, version, to: final }));
      await this.refreshLibrary();
      return true;
    } catch (e) {
      this.say(errText(e), "error");
      return false;
    }
  }

  async uninstall(item: Item, version: string) {
    const msg = t("Remove {name} {version} from your computer?", { name: item.name, version }) + "\n" + t("Its files are deleted from GenLauncher's folder; your game files are not touched.");
    if (!(await confirmDialog(msg, t("Remove {name}", { name: item.name })))) return;
    if (!isTauri) {
      const v = item.versions.find((x) => x.v === version);
      if (v) v.installed = false;
      return;
    }
    try {
      await uninstallVersion(item.id, version);
      if (item.versions.filter((v) => v.installed && v.v !== version).length === 0 && this.enabled.includes(item.id)) {
        // nothing left of it to switch on
        this.enabled = this.enabled.filter((id) => id !== item.id);
        this.persist();
      }
      this.say(t("{name} {version} removed", { name: item.name, version }));
      await this.refreshLibrary();
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  /** Mods the user already agreed to start although the catalog says they are no longer maintained. */
  private deprecatedOk: string[] = load("deprecatedOk", []);

  async play(target: LaunchTarget = "game") {
    if (this.launch !== "idle") return;
    const mod = this.activeMod;
    if (mod.deprecated && !this.deprecatedOk.includes(mod.id)) {
      const msg = t("{name} is no longer maintained. It may not work with the current game or with newer mods.", { name: mod.name }) + "\n" + t("Start it anyway?");
      if (!(await confirmDialog(msg, t("Old mod"), t("Play anyway"), t("Cancel")))) return;
      this.deprecatedOk = [...this.deprecatedOk, mod.id];
      save("deprecatedOk", this.deprecatedOk);
    }
    if (this.activeModId) { this.lastPlayed = { ...this.lastPlayed, [this.activeModId]: Date.now() }; save("lastPlayed", this.lastPlayed); }
    if (isTauri) {
      this.launch = "preparing";
      try {
        await this.flush();
        await launchGame(target); // returns once the game process is up
        this.launch = "running";
      } catch (e) {
        this.launch = "idle";
        this.say(errText(e), "error");
      }
      return;
    }
    this.launch = "preparing";
    await new Promise((r) => setTimeout(r, 1400));
    this.launch = "running";
    this.say(t("Game started (simulated)"));
    await new Promise((r) => setTimeout(r, 3000));
    this.launch = "idle";
    this.say(t("Game closed, files restored"));
  }
}

export const app = new AppState();

// checks in the browser preview can open the Add dialog as if a file had been dropped (never in a production build)
if (import.meta.env.DEV && typeof window !== "undefined") {
  window.addEventListener("genlauncher-test-drop", () => {
    const file = localStorage.getItem("test-drop");
    if (file) app.openAdd("mod", null, { name: "", pageUrl: null, update: false, file, askType: true });
  });
}
