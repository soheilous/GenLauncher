import { invoke } from "@tauri-apps/api/core";
import type { Announcement, Bootstrap, CatalogResponse, Item, Settings } from "./types";

/** False in a plain browser (vite dev, headless QA); the UI then runs on mock data. */
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const backend = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),
  selectInstall: (dir: string) => invoke<Bootstrap>("select_install", { dir }),
  catalogLoad: () => invoke<CatalogResponse>("catalog_load"),
  /** Rebuild the library from disk with the catalog already in memory (no network). */
  catalogReload: () => invoke<Item[]>("catalog_reload"),
  loadExtras: (modId: string) => invoke<Item[]>("load_extras", { modId }),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
};

export async function pickFolder(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false, title: "Select your Generals / Zero Hour folder" });
  return typeof picked === "string" ? picked : null;
}

export interface LaunchFinished {
  error: string | null;
  warnings: string[];
  playedSecs: number;
}

export type LaunchTarget = "game" | "worldbuilder";

/** Resolves once the game process has started; the session ends with a `launch-finished` event. */
export const launchGame = (target: LaunchTarget) => invoke<void>("launch_game", { target });

export async function onLaunchFinished(cb: (r: LaunchFinished) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<LaunchFinished>("launch-finished", (e) => cb(e.payload));
}

export async function onCloseBlocked(cb: () => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen("close-blocked", () => cb());
}

export interface InstallProgress {
  id: string;
  phase: "preparing" | "downloading" | "verifying" | "extracting" | "finishing";
  done: number;
  total: number;
}

export interface InstallFinished {
  id: string;
  ok: boolean;
  cancelled: boolean;
  error: string | null;
  version: string | null;
}

export const cancelInstall = (id: string) => invoke<void>("cancel_install", { id });
export const uninstallVersion = (id: string, version: string) => invoke<void>("uninstall_version", { id, version });

export async function onInstallProgress(cb: (p: InstallProgress) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<InstallProgress>("install-progress", (e) => cb(e.payload));
}

export async function onInstallFinished(cb: (r: InstallFinished) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<InstallFinished>("install-finished", (e) => cb(e.payload));
}

export async function confirmDialog(message: string, title = "GenLauncher", okLabel = "Remove", cancelLabel = "Keep"): Promise<boolean> {
  if (!isTauri) return confirm(message);
  const { ask } = await import("@tauri-apps/plugin-dialog");
  return ask(message, { title, kind: "warning", okLabel, cancelLabel });
}

export const loadAnnouncements = () => invoke<Announcement[]>("announcements_load");

export const migrateLegacy = () => invoke<{ stepsDone: number }>("migrate_legacy");

// ---- adding things the user picked or linked themselves ----

export interface AddRequest {
  source:
    | { type: "file"; path: string; pageUrl?: string | null }
    | { type: "link"; url: string; pageUrl?: string | null }
    /** Whatever the built-in browser is downloading (or has downloaded); installed once complete. */
    | { type: "browser"; pageUrl?: string | null };
  name: string;
  version: string;
  kind: "mod" | "patch" | "addon" | "exe";
  /** The mod a patch / addon / executable belongs to. */
  parent?: string | null;
}

/** Starts an install; resolves with the new item's id. Progress arrives as install events. */
export const addItem = (request: AddRequest) => invoke<string>("add_item", { request });

export async function pickArchiveFile(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({
    multiple: false,
    directory: false,
    title: "Choose the downloaded mod file",
    filters: [{ name: "Mod archive", extensions: ["zip", "7z", "rar", "big", "gib"] }, { name: "All files", extensions: ["*"] }],
  });
  return typeof picked === "string" ? picked : null;
}

export async function pickModFolder(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ multiple: false, directory: true, title: "Choose the folder with the mod's files" });
  return typeof picked === "string" ? picked : null;
}

export async function openInBrowser(url: string) {
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

// ---- the launcher's own browser window ----

export type BrowserEvent =
  | { state: "started"; fileName: string }
  | { state: "progress"; fileName: string; bytes: number }
  | { state: "finished"; fileName: string; path: string }
  | { state: "failed"; message: string }
  | { state: "stalled"; message: string }
  | { state: "closed" };

/** Opens `url` in a launcher window; the user clicks a download there and `browser-event`s follow. */
export const openBuiltInBrowser = (url: string) => invoke<void>("open_browser", { url });
export const closeBuiltInBrowser = () => invoke<void>("close_browser");

export async function onBrowserEvent(cb: (e: BrowserEvent) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<BrowserEvent>("browser-event", (e) => cb(e.payload));
}

/** Show an item's installed files (the chosen version) in the system file manager. */
export const openItemFolder = (id: string, version?: string | null) => invoke<void>("open_item_folder", { id, version: version ?? null });

// ---- configuration files ----

export const exportConfig = (path: string, ui: unknown) => invoke<string[]>("config_export", { path, ui });
export const importConfig = (path: string, currentUi: unknown) => invoke<{ games: string[]; ui: unknown }>("config_import", { path, currentUi });

export interface Backup { name: string; at: number; label: string; bytes: number }
export const backupCreate = (ui: unknown) => invoke<Backup>("backup_create", { ui });
export const backupList = () => invoke<Backup[]>("backup_list");
export const backupDelete = (name: string) => invoke<void>("backup_delete", { name });
export const backupRestore = (name: string, currentUi: unknown) => invoke<{ games: string[]; ui: unknown }>("backup_restore", { name, currentUi });

export async function pickConfigSavePath(): Promise<string | null> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ title: "Export configuration", defaultPath: "genlauncher-config.json", filters: [{ name: "GenLauncher configuration", extensions: ["json"] }] })) ?? null;
}

export async function pickConfigFile(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ title: "Import configuration", multiple: false, directory: false, filters: [{ name: "GenLauncher configuration", extensions: ["json"] }] });
  return typeof picked === "string" ? picked : null;
}

// ---- the game's own options (Options.ini) ----

export interface Graphics {
  width: number; height: number;
  /** 0 = high, 1 = medium, 2 = low */
  textureReduction: number;
  maxParticleCount: number;
  shadowVolumes: boolean; shadowDecals: boolean; buildingOcclusion: boolean; showTrees: boolean; cloudMap: boolean; lightMap: boolean;
  softWaterEdge: boolean; heatEffects: boolean; extraAnimations: boolean; dynamicLod: boolean; alternateMouse: boolean;
}

export const optionsLoad = () => invoke<{ path: string; exists: boolean; graphics: Graphics }>("options_load");
export const optionsSave = (graphics: Graphics) => invoke<void>("options_save", { graphics });

// ---- Google sign-in and Drive backups ----

export interface CloudBackup { id: string; name: string; at: number; label: string; bytes: number }
export const googleStatus = () => invoke<{ email: string } | null>("google_status");
export const googleSignIn = () => invoke<{ email: string }>("google_sign_in");
export const googleSignOut = () => invoke<void>("google_sign_out");
export const cloudBackupCreate = (ui: unknown, label?: string) => invoke<CloudBackup>("cloud_backup_create", { ui, label });
export const cloudBackupList = () => invoke<CloudBackup[]>("cloud_backup_list");
export const cloudBackupDelete = (id: string) => invoke<void>("cloud_backup_delete", { id });
export const cloudBackupRestore = (id: string, currentUi: unknown) => invoke<{ games: string[]; ui: unknown }>("cloud_backup_restore", { id, currentUi });
