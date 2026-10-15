import type { Item } from "./types";

// Placeholder data. Replaced by the Rust manifest reader later.
export const mockItems: Item[] = [
  { id: "rotr", kind: "mod", name: "Rise of the Reds", hue: 8, cover: "/covers/rotr.svg", latest: "1.87", sizeMb: 2400,
    summary: "Total conversion adding Russia and the Chinese Federation with a new campaign-scale roster.",
    versions: [{ v: "1.87", installed: true }, { v: "1.86", installed: true }],
    links: { moddb: "https://www.moddb.com", discord: "https://discord.gg" } },
  { id: "contra", kind: "mod", name: "Contra", hue: 150, cover: "/covers/contra.svg", latest: "009 Final", sizeMb: 1900,
    summary: "Large community overhaul with rebalanced factions, new generals and tech trees.",
    versions: [{ v: "009 Final", installed: false }, { v: "009 Beta 3", installed: true }] },
  { id: "shockwave", kind: "mod", name: "Shockwave", hue: 205, cover: "/covers/shockwave.svg", latest: "1.201", sizeMb: 1300,
    summary: "Adds new units, abilities and a refined general's powers system to Zero Hour.",
    versions: [{ v: "1.201", installed: true }] },
  { id: "eod", kind: "mod", name: "The End of Days", hue: 32, latest: "0.97.5", sizeMb: 1100,
    summary: "Futuristic setting with three new armies and a dark storyline.",
    versions: [{ v: "0.97.5", installed: false }] },
  { id: "evo", kind: "mod", name: "Evolution", hue: 300, latest: "2.4", sizeMb: 800,
    summary: "Graphical and gameplay modernisation focused on competitive balance.",
    versions: [{ v: "2.4", installed: false }] },
  { id: "original-game", kind: "mod", name: "Original Game", hue: 220, latest: "1.04", sizeMb: 0,
    summary: "Unmodified Zero Hour. Patches and addons still apply.",
    versions: [{ v: "1.04", installed: true }] },

  { id: "rotr-p1", kind: "patch", parent: "rotr", name: "Balance Patch", hue: 8, latest: "1.87.2", versions: [{ v: "1.87.2", installed: true }], summary: "Hotfixes and balance." },
  { id: "rotr-p2", kind: "patch", parent: "rotr", name: "Legacy 1.85 Rules", hue: 8, latest: "1.0", versions: [{ v: "1.0", installed: false }], summary: "Restores 1.85 balance." },
  { id: "rotr-a1", kind: "addon", parent: "rotr", name: "HD Textures", hue: 8, latest: "3.1", versions: [{ v: "3.1", installed: true }], summary: "High resolution unit textures." },
  { id: "rotr-a2", kind: "addon", parent: "rotr", name: "Cinematic Music Pack", hue: 8, latest: "1.2", versions: [{ v: "1.2", installed: false }], summary: "Extra soundtrack." },
  { id: "rotr-e1", kind: "exe", parent: "rotr", name: "Modded EXE (camera unlock)", hue: 8, latest: "2.0", versions: [{ v: "2.0", installed: true }], summary: "Bypasses the LAN camera-height limit." },

  { id: "contra-p1", kind: "patch", parent: "contra", name: "Hotfix 3", hue: 150, latest: "3", versions: [{ v: "3", installed: true }], summary: "Stability fixes." },
  { id: "contra-a1", kind: "addon", parent: "contra", name: "Extra Maps", hue: 150, latest: "1.0", versions: [{ v: "1.0", installed: true }], summary: "40 community maps." },

  { id: "og-a1", kind: "addon", parent: "original-game", name: "Widescreen Fix", hue: 220, latest: "1.1", versions: [{ v: "1.1", installed: true }], summary: "Proper UI scaling above 1080p." },
  { id: "og-e1", kind: "exe", parent: "original-game", name: "Modded EXE (camera unlock)", hue: 220, latest: "2.0", versions: [{ v: "2.0", installed: true }], summary: "Bypasses the LAN camera-height limit." },
];
