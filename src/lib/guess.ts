// Suggestions for the "Add mod" form. They only prefill the fields; the user always has the last word.

const ARCHIVE_EXT = /\.(zip|7z|rar|big|gib|tar|gz|exe)$/i;

export interface Guess {
  name: string;
  version: string;
}

function tidy(s: string): string {
  return s
    .replace(/[_+]+/g, " ")
    .replace(/\(\s*\)|\[\s*\]/g, " ") // brackets that held only the version
    .replace(/\s+/g, " ")
    .replace(/^[\s\-–—.()[\]]+|[\s\-–—.()[\]]+$/g, "");
}

/** `Rise_of_the_Reds_v1.87.7z` -> { name: "Rise of the Reds", version: "1.87" } */
export function guessFromFileName(raw: string): Guess {
  let s = raw.split(/[\\/]/).pop() ?? raw;
  s = s.split("?")[0];
  try {
    s = decodeURIComponent(s);
  } catch {
    /* keep the raw text */
  }
  s = s.replace(ARCHIVE_EXT, "");
  s = tidy(s);

  // 1.2, 1.2.3, v1.2.3 (dots make it unambiguous), then a lone "v12"
  const dotted = /(?:^|[\s\-_(])[vV]?(\d+(?:\.\d+)+)(?=$|[\s\-_)a-zA-Z])/.exec(s);
  const lone = /(?:^|[\s\-_(])[vV](\d+)(?=$|[\s\-_)])/.exec(s);
  const hit = dotted ?? lone;
  if (!hit) return { name: s || "New mod", version: "1.0" };

  const version = hit[1];
  const name = tidy(s.slice(0, hit.index) + " " + s.slice(hit.index + hit[0].length));
  return { name: name || s, version };
}

/** `https://www.moddb.com/mods/zero-hour-reborn-omega` -> "Zero Hour Reborn Omega" */
export function guessNameFromPage(url: string): string {
  try {
    const parts = new URL(url).pathname.split("/").filter(Boolean);
    const i = parts.indexOf("mods");
    const slug = i >= 0 ? parts[i + 1] : parts[parts.length - 1];
    if (!slug) return "";
    return slug.replace(/[-_]+/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()).trim();
  } catch {
    return "";
  }
}

export function isModDbPage(url: string): boolean {
  try {
    const u = new URL(url.trim());
    return /(^|\.)moddb\.com$/i.test(u.hostname) && !/\.(zip|7z|rar)$/i.test(u.pathname);
  } catch {
    return false;
  }
}

export function isWebAddress(url: string): boolean {
  try {
    return ["http:", "https:"].includes(new URL(url.trim()).protocol);
  } catch {
    return false;
  }
}

/** Mirrors `browser::looks_like_direct_file` in the Rust core: a clear file link needs no browsing. */
export function looksLikeDirectFile(url: string): boolean {
  let u: URL;
  try {
    u = new URL(url.trim());
  } catch {
    return false;
  }
  if (!["http:", "https:"].includes(u.protocol)) return false;
  const host = u.hostname.toLowerCase();
  const path = u.pathname.toLowerCase();
  const archive = [".zip", ".7z", ".rar", ".big", ".gib"].some((e) => path.endsWith(e));
  if (host.endsWith("moddb.com") && !archive) return false; // pages and session-bound mirror redirects: browse them
  const share =
    (host === "onedrive.live.com" && path.startsWith("/download")) ||
    (host.endsWith("dropbox.com") && u.search.includes("dl=")) ||
    (host.endsWith("github.com") && path.includes("/releases/download/")) ||
    host === "objects.githubusercontent.com";
  return archive || share;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

/** The same id the Rust core derives from a name (`catalog::slug`): letters and digits, lowercased, joined by `-`. */
export function slugify(s: string): string {
  return s.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-+|-+$/g, "");
}
