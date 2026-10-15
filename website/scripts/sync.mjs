// Copies what the website shows from the rest of the repository into src/lib/generated/:
//   - the mod catalog (github.com/soheilous/generals-mod-catalog; the snapshot in ../src-tauri/bundled/ if GitHub is unreachable)
//   - the privacy policy and terms of service (../PRIVACY.md, ../TERMS.md), turned into HTML
// If the files outside website/ are not available (a build that only has this folder), they are fetched from GitHub.

import fs from "node:fs";
import crypto from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(here, "..", "..");
const out = path.join(here, "..", "src", "lib", "generated");
const RAW = "https://raw.githubusercontent.com/soheilous/GenLauncher/main/";
const CATALOG = "https://raw.githubusercontent.com/soheilous/generals-mod-catalog/main/catalog/";
fs.mkdirSync(out, { recursive: true });

async function read(rel) {
  const local = path.join(root, rel);
  if (fs.existsSync(local)) return fs.readFileSync(local, "utf8");
  const r = await fetch(RAW + rel);
  if (!r.ok) throw new Error(`${rel}: neither in the repository nor on GitHub (${r.status})`);
  return r.text();
}

/** The latest catalog file from its repository; the snapshot shipped with the launcher when that cannot be reached. */
async function readCatalog(file) {
  try {
    const r = await fetch(CATALOG + file, { signal: AbortSignal.timeout(20000) });
    if (r.ok) return await r.text();
    console.warn(`catalog ${file}: HTTP ${r.status}, using the bundled snapshot`);
  } catch (e) {
    console.warn(`catalog ${file}: ${e.message}, using the bundled snapshot`);
  }
  return read("src-tauri/bundled/" + file);
}

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Links between the pages stay on the site; links to other files in the repository go to GitHub. */
function link(u) {
  if (/^https?:/.test(u)) return u;
  const page = /^(PRIVACY|TERMS)\.md$/.exec(u);
  return page ? "/" + page[1].toLowerCase() : "https://github.com/soheilous/GenLauncher/tree/main/" + u;
}

function inline(s) {
  s = esc(s);
  s = s.replace(/&lt;(https?:\/\/[^\s&]+)&gt;/g, '<a href="$1">$1</a>');
  s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, u) => `<a href="${link(u)}">${t}</a>`);
  s = s.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>").replace(/\*([^*]+)\*/g, "<em>$1</em>");
  return s.replace(/`([^`]+)`/g, "<code>$1</code>");
}

/** The little bit of markdown those pages use: headings, paragraphs, bullet lists, bold, italic, code, links. */
function markdown(md) {
  const html = [];
  let para = [];
  let list = false;
  const flush = () => { if (para.length) html.push(`<p>${inline(para.join(" "))}</p>`); para = []; };
  const close = () => { if (list) html.push("</ul>"); list = false; };
  for (const line of md.split(/\r?\n/)) {
    const h = /^(#{1,3})\s+(.*)$/.exec(line);
    if (h) { flush(); close(); html.push(`<h${h[1].length}>${inline(h[2])}</h${h[1].length}>`); continue; }
    const li = /^-\s+(.*)$/.exec(line);
    if (li) { flush(); if (!list) html.push("<ul>"); list = true; html.push(`<li>${inline(li[1])}</li>`); continue; }
    if (list && /^\s+\S/.test(line)) { html[html.length - 1] = html[html.length - 1].replace(/<\/li>$/, ` ${inline(line.trim())}</li>`); continue; }
    if (!line.trim()) { flush(); close(); continue; }
    para.push(line.trim());
  }
  flush(); close();
  return html.join("\n");
}

const [zh, generals, privacy, terms] = await Promise.all([readCatalog("zero-hour.json"), readCatalog("generals.json"), read("PRIVACY.md"), read("TERMS.md")]);
/**
 * Mod cover images are kept on this site, so a card does not depend on imgur or Discord staying up.
 * Each image is downloaded once into static/covers/ (and committed); an existing file is never fetched again, and an
 * address that no longer works simply has no cover.
 */
const coversDir = path.join(here, "..", "static", "covers");
fs.mkdirSync(coversDir, { recursive: true });
const EXT = { "image/png": ".png", "image/jpeg": ".jpg", "image/webp": ".webp", "image/gif": ".gif", "image/svg+xml": ".svg" };
const index = { zh: JSON.parse(zh), generals: JSON.parse(generals) };
const urls = [...new Set([...index.zh.mods, ...index.generals.mods].map((m) => m.image).filter((u) => typeof u === "string" && /^https?:/.test(u)))];
const covers = {};
const existing = fs.readdirSync(coversDir);
await Promise.all(urls.map(async (url) => {
  const key = crypto.createHash("sha1").update(url).digest("hex").slice(0, 12);
  const have = existing.find((f) => f.startsWith(key + "."));
  if (have) { covers[url] = "/covers/" + have; return; }
  try {
    const r = await fetch(url, { signal: AbortSignal.timeout(15000), headers: { "user-agent": "GenLauncher-site-build (+https://github.com/soheilous/GenLauncher)" } });
    const ext = EXT[(r.headers.get("content-type") ?? "").split(";")[0].trim().toLowerCase()];
    if (!r.ok || !ext) throw new Error(r.ok ? "not an image" : "HTTP " + r.status);
    const bytes = Buffer.from(await r.arrayBuffer());
    if (bytes.length > 3 * 1024 * 1024) throw new Error("larger than 3 MB");
    fs.writeFileSync(path.join(coversDir, key + ext), bytes);
    covers[url] = "/covers/" + key + ext;
  } catch (e) {
    console.warn(`no cover for ${url}: ${e.message}`);
  }
}));
fs.writeFileSync(path.join(out, "covers.json"), JSON.stringify(covers));

fs.writeFileSync(path.join(out, "index.json"), JSON.stringify(index));
fs.writeFileSync(path.join(out, "legal.json"), JSON.stringify({ privacy: markdown(privacy), terms: markdown(terms) }));
console.log(`synced the mod index (${Object.keys(covers).length} of ${urls.length} covers) and the legal pages`);
