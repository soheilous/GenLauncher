import {
  addItem, backend, cancelInstall, confirmDialog, installItem, isTauri, launchGame, migrateLegacy, onCloseBlocked, onInstallFinished,
  openInBrowser, openItemFolder,
  onInstallProgress, onLaunchFinished, pickFolder, uninstallVersion, type InstallFinished, type InstallProgress,
  type AddRequest, type LaunchTarget,
} from "./backend";
import { slugify } from "./guess";
import { mockItems } from "./mock";
import type { Bootstrap, Game, GameInstall, Item, LegacySummary, ModKind, Settings } from "./types";

export type View = "library" | "discover" | "downloads";
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
  useVulkan: false, activeMod: null, enabled: [], chosenVersions: {},
};

function load<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : (JSON.parse(raw) as T);
  } catch {
    return fallback;
  }
}
function save(key: string, value: unknown) {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* storage unavailable */ }
}
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
  toast = $state<string | null>(null);
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
  // Original Game is always listed first in the library
  installedMods = $derived(
    this.filteredMods
      .filter((m) => m.versions.some((v) => v.installed))
      .sort((a, b) => Number(b.id === "original-game") - Number(a.id === "original-game"))
  );
  availableMods = $derived(this.filteredMods.filter((m) => !m.versions.some((v) => v.installed)));

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
      this.say(`Could not start: ${errText(e)}`);
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
        const played = m >= 1 ? `${m} min` : `${r.playedSecs} s`;
        this.say(`Game closed after ${played}. Game folder restored${r.warnings.length ? ` (${r.warnings.length} notes in the log)` : ""}.`);
      }
    });
    void onCloseBlocked(() => this.say("The game is still running. Close it first so your game folder can be restored."));
    void onInstallProgress((p) => {
      this.installProgress[p.id] = p;
      this.applyProgress();
    });
    void onInstallFinished((r) => void this.installFinished(r));
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
    return ({ preparing: "Preparing", downloading: "Downloading", verifying: "Checking files", extracting: "Extracting", finishing: "Finishing" } as Record<string, string>)[item.phase ?? ""] ?? "Working";
  }

  private applyProgress() {
    for (const item of this.items) {
      const p = this.installProgress[item.id];
      item.progress = p ? (p.total > 0 ? Math.min(1, p.done / p.total) : 0) : undefined;
      item.phase = p?.phase;
      item.bytes = p && p.total === 0 && p.done > 0 ? p.done : undefined;
    }
  }

  private async installFinished(r: InstallFinished) {
    delete this.installProgress[r.id];
    delete this.pending[r.id];
    this.applyProgress();
    const name = this.items.find((i) => i.id === r.id)?.name ?? "Item";
    if (!r.ok) {
      // a placeholder shown for something the user added (it has no versions yet) must not linger
      this.items = this.items.filter((i) => !(i.id === r.id && i.versions.length === 0));
    }
    if (r.cancelled) {
      this.say(`${name}: installation cancelled. Progress is kept if you try again.`);
      return;
    }
    if (!r.ok) {
      this.say(`${name}: ${r.error ?? "installation failed"}`);
      return;
    }
    this.say(`${name} ${r.version ?? ""} installed`.trim());
    await this.refreshLibrary();
  }

  // --- "Add mod": things the user picked or linked themselves ---
  addDialog = $state<{ kind: ModKind; parent: string | null } | null>(null);

  openAdd(kind: ModKind = "mod", parent: string | null = null) {
    this.addDialog = { kind, parent };
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
      this.say(`${name} ${version} added (preview)`);
      return true;
    }
    try {
      const id = await addItem(req);
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
      this.say(errText(e));
      return false;
    }
  }

  /** Open a web page in the user's own browser. */
  async openPage(url: string) {
    try {
      if (isTauri) await openInBrowser(url);
      else window.open(url, "_blank");
    } catch {
      this.say("Could not open your browser. The address is: " + url);
    }
  }

  /** Show the installed files of `item` (the given version) in the file manager. */
  async openFolder(item: Item, version?: string) {
    if (!isTauri) {
      this.say("Opening folders works in the desktop app.");
      return;
    }
    try {
      await openItemFolder(item.id, version);
    } catch (e) {
      this.say(errText(e));
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
        ? "The last session did not finish cleanly. Your game folder has been restored."
        : `Could not fully restore the game folder: ${r.warnings[0] ?? "unknown problem"}`);
    }
    this.phase = "loading";
    if (b.legacy) await this.offerMigration(b.legacy); // the library is scanned fresh right after
    await this.loadCatalog();
  }

  /** Old GenLauncher 1.x data found: explain exactly what will happen, then do it only on a yes. */
  private async offerMigration(l: LegacySummary): Promise<boolean> {
    const gb = (l.bytes / 1e9).toFixed(1);
    const msg =
      `Data from the old GenLauncher was found in this game folder.\n\n` +
      `• ${l.mods} installed mods (${gb} GB) will be MOVED into the new library. They are kept, not re-downloaded.\n` +
      `• ${l.deletions} leftover files and folders of the old launcher (its program, settings and caches) will be deleted.\n` +
      `• Your game files, GenTool and Steam files are not touched.` +
      (l.notes.length ? `\n\nLeft alone:\n${l.notes.map((n) => "• " + n).join("\n")}` : "");
    if (!(await confirmDialog(msg, "Move to the new library", "Move and clean up", "Not now"))) return false;
    try {
      await migrateLegacy();
      this.say("Your library was moved and the old launcher's files were removed.");
      return true;
    } catch (e) {
      this.say(errText(e));
      return false;
    }
  }

  async loadCatalog() {
    try {
      const res = await backend.catalogLoad();
      this.items = res.items;
      this.online = res.online;
      this.catalogError = res.error;
      if (!this.mods.some((m) => m.id === this.activeModId)) {
        this.activeModId = (this.mods.find((m) => m.versions.some((v) => v.installed)) ?? this.mods[0])?.id ?? "";
      }
      this.applyCovers();
      this.applyProgress();
      this.phase = "ready";
      this.applyTheme();
      await this.ensureExtras(this.activeModId);
    } catch (e) {
      this.say(`Could not load the library: ${errText(e)}`);
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
      this.say(`Could not load add-ons: ${errText(e)}`);
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
      this.say(errText(e));
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

  /** Write settings now (a launch must see exactly what is on screen). */
  async flush() {
    clearTimeout(this.saveTimer);
    const s = this.currentSettings();
    if (!isTauri) { save("settings", s); return; }
    try {
      await backend.saveSettings(s);
    } catch (e) {
      this.say(`Could not save settings: ${errText(e)}`);
      throw e;
    }
  }

  toggleSidebar() { this.sidebarCollapsed = !this.sidebarCollapsed; save("sidebarCollapsed", this.sidebarCollapsed); }

  /** Set a custom cover for a mod; its dominant hue becomes the mod's accent. */
  setCover(id: string, cover: string, hue: number) {
    const item = this.items.find((i) => i.id === id);
    if (!item) return;
    item.cover = cover; item.hue = hue;
    this.customCovers[id] = { cover, hue };
    try {
      localStorage.setItem("customCovers", JSON.stringify(this.customCovers));
    } catch {
      this.say("Cover applied, but it is too large to remember after restart");
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

  say(msg: string) {
    this.toast = msg;
    setTimeout(() => { if (this.toast === msg) this.toast = null; }, 3600);
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

  async install(item: Item) {
    if (isTauri) {
      if (this.installProgress[item.id]) return;
      // show it as started right away; the core's progress events take over
      this.installProgress[item.id] = { id: item.id, phase: "preparing", done: 0, total: 0 };
      this.applyProgress();
      try {
        await installItem(item.id);
      } catch (e) {
        delete this.installProgress[item.id];
        this.applyProgress();
        this.say(errText(e));
      }
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
        this.say(`${item.name} ${item.latest} installed`);
      }
    }, 140);
  }

  async cancel(item: Item) {
    if (!isTauri) { item.progress = undefined; return; }
    try { await cancelInstall(item.id); } catch (e) { this.say(errText(e)); }
  }

  /** Remove one installed version from disk (asks first). */
  async uninstall(item: Item, version: string) {
    const msg = `Remove ${item.name} ${version} from your computer?\nIts files in the GLM folder are deleted; your game files are not touched.`;
    if (!(await confirmDialog(msg, `Remove ${item.name}`))) return;
    if (!isTauri) {
      const v = item.versions.find((x) => x.v === version);
      if (v) v.installed = false;
      return;
    }
    try {
      await uninstallVersion(item.id, version);
      this.say(`${item.name} ${version} removed`);
      await this.refreshLibrary();
    } catch (e) {
      this.say(errText(e));
    }
  }

  async play(target: LaunchTarget = "game") {
    if (this.launch !== "idle") return;
    if (isTauri) {
      this.launch = "preparing";
      try {
        await this.flush();
        await launchGame(target); // returns once the game process is up
        this.launch = "running";
      } catch (e) {
        this.launch = "idle";
        this.say(errText(e));
      }
      return;
    }
    this.launch = "preparing";
    await new Promise((r) => setTimeout(r, 1400));
    this.launch = "running";
    this.say("Game started (simulated)");
    await new Promise((r) => setTimeout(r, 3000));
    this.launch = "idle";
    this.say("Game closed, files restored");
  }
}

export const app = new AppState();
