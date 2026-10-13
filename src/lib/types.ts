export type Game = "zh" | "generals";
export type ModKind = "mod" | "patch" | "addon" | "exe";

export interface Version {
  v: string;
  installed: boolean;
  /** The web page this version was downloaded from, when it was added from one. */
  page?: string | null;
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
  launcherVersion: string | null;
  error: string | null;
}
