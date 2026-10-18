export type Game = "zh" | "generals";
export type ModKind = "mod" | "patch" | "addon" | "exe";

export interface Version {
  v: string;
  installed: boolean;
  /** The web page this version was downloaded from, when it was added from one (for a version not installed: the catalog's page for it). */
  page?: string | null;
  /** A picture of this very version, when the catalog has one. */
  image?: string | null;
  /** The author's direct download of this very version, when there is one. */
  direct?: { url: string; via?: string | null; size?: number | null } | null;
  date?: string | null;
}

/** A named combination to start with one click: a mod, its version, the patches / add-ons / executables switched on, and two options. */
export interface Preset {
  id: string;
  name: string;
  game: Game;
  mod: string;
  version?: string;
  /** Ids of the mod's patches, add-ons and executables. */
  enabled: string[];
  windowed: boolean;
  quickStart: boolean;
}

export interface Links {
  moddb?: string | null;
  discord?: string | null;
  news?: string | null;
  network?: string | null;
  support?: string | null;
}

/** Mirrors `catalog::Item` in the Rust core. */
export interface Item {
  id: string;
  kind: ModKind;
  name: string;
  summary: string;
  hue: number; // accent hue derived from art
  cover?: string | null; // image URL or data URL; drives card art, hero and window backdrop
  versions: Version[];
  latest: string;
  parent?: string | null; // mod id for patches/addons/exes
  dependsOn?: string | null; // e.g. an addon that needs a specific patch
  links?: Links;
  /** A direct download the author published (shown in the catalog): installs with one click, no page to visit. */
  direct?: { url: string; via?: string | null; size?: number | null } | null;
  deprecated?: boolean;
  // UI-only state
  progress?: number; // 0..1 while installing (0 while the size is still unknown)
  bytes?: number; // bytes received so far while the total size is unknown
  phase?: string; // preparing, downloading, verifying, extracting, finishing
  selected?: boolean;
  sizeMb?: number;
}

export interface GameInstall {
  dir: string;
  game: Game;
}

/** Mirrors `store::Settings`. */
export interface Settings {
  windowed: boolean;
  quickStart: boolean;
  cameraHeight: number;
  gameParams: string;
  checkModFiles: boolean;
  askBeforeCheck: boolean;
  hideLauncherWhilePlaying: boolean;
  gentoolAutoUpdate: boolean;
  autoDeleteOldVersions: boolean;
  moddedExe: boolean;
  useVulkan: boolean;
  /** Programs started together with the game. */
  extraPrograms: { path: string; onlyWindowed: boolean }[];
  activeMod: string | null;
  enabled: string[];
  chosenVersions: Record<string, string>;
}

/** GenLauncher 1.x data found in the game folder. */
export interface LegacySummary {
  mods: number;
  bytes: number;
  deletions: number;
  notes: string[];
}

export interface Recovered {
  opsUndone: number;
  warnings: string[];
}

export interface Bootstrap {
  recovered?: Recovered | null;
  legacy?: LegacySummary | null;
  installs: GameInstall[];
  active: GameInstall | null;
  settings: Settings;
}

export interface CatalogResponse {
  items: Item[];
  online: boolean;
  error: string | null;
}

export type NoticeLevel = "info" | "success" | "error" | "news";

/** One entry of the Notifications page. */
export interface Notice {
  id: string;
  at: number; // ms since epoch
  level: NoticeLevel;
  text: string;
  title?: string;
  link?: string | null;
  read: boolean;
}

/** A message from the project (catalog/announcements.json in the catalog repository). */
export interface Announcement {
  id: string;
  date: string;
  title: string;
  text: string;
  link?: string | null;
}
