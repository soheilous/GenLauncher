import {
  addItem, backend, cloudBackupCreate, cloudBackupDelete, cloudBackupList, cloudBackupRestore, googleSignIn, googleSignOut, googleStatus, backupCreate, backupDelete, backupList, backupRestore, cancelInstall, exportConfig, onFileDrop, importConfig, loadAnnouncements, pickConfigFile, pickConfigSavePath, confirmDialog, isTauri, launchGame, migrateLegacy, onCloseBlocked, onInstallFinished,
  openInBrowser, openItemFolder,
  onInstallProgress, onLaunchFinished, pickFolder, uninstallVersion, type InstallFinished, type InstallProgress,
  type AddRequest, type Backup, type CloudBackup, type LaunchTarget,
} from "./backend";
import { slugify } from "./guess";
import { mockItems } from "./mock";
import type { Bootstrap, Game, GameInstall, Item, LegacySummary, ModKind, Notice, NoticeLevel, Settings } from "./types";

/** What "Install" on a catalog entry hands to the Add dialog. */
export interface AddPreset {
  name: string;
  /** Where to get it (opened in the built-in browser), if the catalog knows. */
  pageUrl: string | null;
  /** Another version of something already installed: no "same name" confirmation needed. */
  update: boolean;
  /** A file or folder already chosen (dropped onto the window). */
  file?: string;
}

export type View = "library" | "discover" | "downloads" | "notifications";
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
      this.say(`Could not start: ${errText(e)}`, "error");
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
    void onFileDrop((e) => {
      if (e.type === "enter" || e.type === "over") this.dragging = this.phase === "ready" && !this.addDialog && !this.optionsOpen;
      else this.dragging = false;
      if (e.type === "drop" && e.paths.length && this.phase === "ready" && !this.addDialog && !this.optionsOpen) this.addDropped(e.paths);
    });
  }

  /** True while something is being dragged over the window. */
  dragging = $state(false);

  /** A file or folder dropped onto the window: open the Add dialog with it chosen. */
  private addDropped(paths: string[]) {
    const first = paths[0];
    if (/\.(png|jpe?g|webp|gif|bmp|ico)$/i.test(first)) {
      this.say("That is an image. To change a mod's cover, use the Change cover button on its panel.");
      return;
    }
    if (paths.length > 1) this.say("One at a time: adding the first of the dropped files.");
    this.view = "library";
    this.openAdd("mod", null, { name: "", pageUrl: null, update: false, file: first });
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
      this.say(`${name}: ${r.error ?? "installation failed"}`, "error");
      return;
    }
    this.say(`${name} ${r.version ?? ""} installed`.trim(), "success");
    await this.refreshLibrary();
  }

  // --- "Add mod": things the user picked or linked themselves ---
  addDialog = $state<{ kind: ModKind; parent: string | null; preset?: AddPreset } | null>(null);

  /** `preset`: something the launcher already knows about (a Discover entry): its name and the page to fetch it from. */
  openAdd(kind: ModKind = "mod", parent: string | null = null, preset?: AddPreset) {
    this.addDialog = { kind, parent, preset };
  }
  /** Reopen the page an installed version came from, ready to add a newer version of it. */
  checkForUpdate(item: Item, pageUrl: string) {
    this.openAdd("mod", null, { name: item.name, pageUrl, update: true });
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
      if (!res.online) this.notify(`Could not reach the mod index, showing the saved copy. ${res.error ?? ""}`.trim(), "error", { id: "index-offline" });
      if (!this.mods.some((m) => m.id === this.activeModId)) {
        this.activeModId = (this.mods.find((m) => m.versions.some((v) => v.installed)) ?? this.mods[0])?.id ?? "";
      }
      this.applyCovers();
      this.applyProgress();
      this.phase = "ready";
      this.applyTheme();
      void this.checkAnnouncements();
      void this.refreshGoogle();
      await this.ensureExtras(this.activeModId);
    } catch (e) {
      this.say(`Could not load the library: ${errText(e)}`, "error");
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
      this.say(`Could not load add-ons: ${errText(e)}`, "error");
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
      this.maybeAutoCloudBackup();
    } catch (e) {
      this.say(`Could not save settings: ${errText(e)}`, "error");
      throw e;
    }
  }

  // --- configuration files ---

  /** The look-and-feel part of a configuration file. */
  private uiBundle(includeCovers: boolean) {
    return {
      theme: this.theme, sidebarCollapsed: this.sidebarCollapsed, accentFromMod: this.accentFromMod, coverBackdrop: this.coverBackdrop,
      ...(includeCovers ? { customCovers: this.customCovers } : {}),
    };
  }

  /** Take over what a configuration file says about the look, ignoring anything of the wrong type. */
  private applyUiBundle(ui: unknown) {
    if (!ui || typeof ui !== "object") return;
    const u = ui as Record<string, unknown>;
    if (u.theme === "dark" || u.theme === "light") save("theme", u.theme);
    for (const k of ["sidebarCollapsed", "accentFromMod", "coverBackdrop"]) if (typeof u[k] === "boolean") save(k, u[k]);
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
    if (!isTauri) { this.say("Exporting works in the desktop app."); return; }
    try {
      const path = await pickConfigSavePath();
      if (!path) return;
      await this.flush();
      const games = await exportConfig(path, this.uiBundle(includeCovers));
      this.say(`Configuration exported (${games.map((g) => (g === "zh" ? "Zero Hour" : "Generals")).join(", ") || "look only"})`, "success");
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  /** Replace this launcher's settings with those from a file (asks first), then restart the screen. */
  backups = $state<Backup[]>([]);

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
    try { this.cloudBackups = await cloudBackupList(); } catch (e) { this.googleFailed(e); }
  }

  async signInGoogle() {
    if (!isTauri) { this.say("Signing in works in the desktop app."); return; }
    this.googleBusy = true;
    try {
      this.google = await googleSignIn();
      this.notify(`Signed in to Google as ${this.google.email || "your account"}.`, "success");
      await this.refreshCloudBackups();
      const newest = this.cloudBackups[0];
      if (newest) {
        // probably a new computer, or a reinstall: offer to take the saved settings
        const when = new Date(newest.at * 1000).toLocaleString();
        if (await confirmDialog(`Your Google Drive has a settings backup from ${when}.\nRestore it? Your current settings are backed up first.`, "Restore from Google Drive", "Restore", "Keep mine")) {
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
    this.say("Signed out of Google. Backups already in your Drive are kept.");
  }

  setAutoCloud(v: boolean) { this.autoCloud = v; save("autoCloud", v); }

  async cloudBackUpNow(label = "manual", withCovers = false, announce = true) {
    if (!this.google) return;
    try {
      await this.flush();
      await cloudBackupCreate(this.uiBundle(withCovers), label);
      save("cloudHash", this.settingsFingerprint());
      save("cloudAt", Date.now());
      if (announce) this.say("Backed up to Google Drive", "success");
      await this.refreshCloudBackups();
    } catch (e) {
      this.googleFailed(e);
    }
  }

  async deleteCloudBackup(b: CloudBackup) {
    try { await cloudBackupDelete(b.id); await this.refreshCloudBackups(); } catch (e) { this.googleFailed(e); }
  }

  async restoreCloudBackup(b: CloudBackup, ask = true) {
    const when = new Date(b.at * 1000).toLocaleString();
    if (ask) {
      const msg = `Go back to the settings saved on ${when}?\nYour current settings are backed up first, so you can undo this. Mods and game files are not touched.`;
      if (!(await confirmDialog(msg, "Restore backup", "Restore", "Cancel"))) return;
    }
    try {
      await this.flush();
      const r = await cloudBackupRestore(b.id, this.uiBundle(false));
      this.applyUiBundle(r.ui);
      this.notify(`Settings restored from Google Drive (${when}).`, "success");
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
    if (Date.now() - load("cloudAt", 0) < 30 * 60 * 1000) return;
    if (load("cloudHash", "") === this.settingsFingerprint()) return;
    void this.cloudBackUpNow("auto", false, false);
  }

  async refreshBackups() {
    if (!isTauri) return;
    try { this.backups = await backupList(); } catch { /* the list is only a convenience */ }
  }

  async backUpNow(includeCovers: boolean) {
    if (!isTauri) { this.say("Backups work in the desktop app."); return; }
    try {
      await this.flush();
      await backupCreate(this.uiBundle(includeCovers));
      this.say("Backup saved", "success");
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
    const when = new Date(b.at * 1000).toLocaleString();
    const msg = `Go back to the settings saved on ${when}?\nYour current settings are backed up first, so you can undo this. Mods and game files are not touched.`;
    if (!(await confirmDialog(msg, "Restore backup", "Restore", "Cancel"))) return;
    try {
      await this.flush();
      const r = await backupRestore(b.name, this.uiBundle(false));
      this.applyUiBundle(r.ui);
      this.notify(`Settings restored from the backup of ${when}.`, "success");
      location.reload();
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  async importConfiguration() {
    if (!isTauri) { this.say("Importing works in the desktop app."); return; }
    try {
      const path = await pickConfigFile();
      if (!path) return;
      const msg = "Replace your current settings with the ones in this file?\nYour current settings are saved as a backup first. Mods and game files are not touched.";
      if (!(await confirmDialog(msg, "Import configuration", "Import", "Cancel"))) return;
      await this.flush();
      const r = await importConfig(path, this.uiBundle(false));
      this.applyUiBundle(r.ui);
      const names = r.games.map((g) => (g === "zh" ? "Zero Hour" : "Generals")).join(", ") || "look only";
      this.notify(`Configuration imported (${names}).`, "success");
      location.reload(); // every screen re-reads its settings from the files
    } catch (e) {
      this.say(errText(e), "error");
    }
  }

  /** Launch options kept in the settings file but edited in the game options window. */
  get gameParams() { return this.rest.gameParams; }
  get hideLauncher() { return this.rest.hideLauncherWhilePlaying; }
  setLaunchExtras(gameParams: string, hideLauncherWhilePlaying: boolean) {
    this.rest = { ...this.rest, gameParams, hideLauncherWhilePlaying };
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
  async install(item: Item) {
    if (isTauri) {
      if (this.installProgress[item.id]) return;
      this.openAdd(item.kind, item.kind === "mod" ? null : (item.parent ?? this.activeModId), {
        name: item.name,
        pageUrl: item.links?.moddb || null,
        update: item.versions.some((v) => v.installed),
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
        this.say(`${item.name} ${item.latest} installed`);
      }
    }, 140);
  }

  async cancel(item: Item) {
    if (!isTauri) { item.progress = undefined; return; }
    try { await cancelInstall(item.id); } catch (e) { this.say(errText(e), "error"); }
  }

  /** Remove one installed version from disk (asks first). */
  async uninstall(item: Item, version: string) {
    const msg = `Remove ${item.name} ${version} from your computer?\nIts files are deleted from GenLauncher's folder; your game files are not touched.`;
    if (!(await confirmDialog(msg, `Remove ${item.name}`))) return;
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
      this.say(`${item.name} ${version} removed`);
      await this.refreshLibrary();
    } catch (e) {
      this.say(errText(e), "error");
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
        this.say(errText(e), "error");
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
