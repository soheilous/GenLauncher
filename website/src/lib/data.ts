import index from "./generated/index.json";
import covers from "./generated/covers.json";
import releaseInfo from "./generated/release.json";

export type GameKey = "zh" | "generals";

export interface SiteMod {
  name: string;
  game: GameKey;
  version?: string;
  /** The page the mod is published on. */
  page?: string;
  image?: string;
  discord?: string;
  deprecated: boolean;
  patches: string[];
  addons: string[];
  /** A stable hue for the placeholder art when there is no image. */
  hue: number;
}

export const GAMES: Record<GameKey, string> = { zh: "Zero Hour", generals: "Generals" };

const hueOf = (s: string) => [...s].reduce((h, c) => (h * 33 + c.charCodeAt(0)) % 360, 7);

interface RawEntry { name: string; version?: string; page?: string; image?: string; discord?: string; deprecated?: boolean }
interface RawMod extends RawEntry { patches?: RawEntry[]; addons?: RawEntry[] }

function read(game: GameKey, mods: RawMod[]): SiteMod[] {
  return mods.map((m) => ({
    name: m.name,
    game,
    version: m.version,
    page: m.page,
    // the copy kept on this site, never the original address
    image: m.image ? (covers as Record<string, string>)[m.image] : undefined,
    discord: m.discord,
    deprecated: !!m.deprecated,
    patches: (m.patches ?? []).map((p) => p.name),
    addons: (m.addons ?? []).map((a) => a.name),
    hue: hueOf(m.name),
  }));
}

export const mods: SiteMod[] = [
  ...read("zh", (index.zh.mods ?? []) as RawMod[]),
  ...read("generals", (index.generals.mods ?? []) as RawMod[]),
];

export const stats = {
  mods: mods.length,
  patches: mods.reduce((n, m) => n + m.patches.length, 0),
  addons: mods.reduce((n, m) => n + m.addons.length, 0),
  updated: (index.zh.updated as string) ?? "",
};

/** The mods with the most to offer, for the home page. */
export const featured = mods
  .filter((m) => m.image && !m.deprecated && m.game === "zh")
  .sort((a, b) => b.patches.length + b.addons.length - (a.patches.length + a.addons.length))
  .slice(0, 8);

export const REPO = "https://github.com/soheilous/GenLauncher";
export const CATALOG = "https://github.com/soheilous/generals-mod-catalog";

/** The newest published release, when the build could read it from GitHub. */
export const release = releaseInfo as { tag: string; version: string; prerelease: boolean; page: string; download: string; size: number } | null;
