// Copies what the website shows from the rest of the repository into src/lib/generated/:
//   - the mod index (../index/*.json)
//   - the privacy policy and terms of service (../PRIVACY.md, ../TERMS.md), turned into HTML
// If the files outside website/ are not available (a build that only has this folder), they are fetched from GitHub.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(here, "..", "..");
const out = path.join(here, "..", "src", "lib", "generated");
const RAW = "https://raw.githubusercontent.com/soheilous/GenLauncher/main/";
fs.mkdirSync(out, { recursive: true });

async function read(rel) {
  const local = path.join(root, rel);
  if (fs.existsSync(local)) return fs.readFileSync(local, "utf8");
  const r = await fetch(RAW + rel);
  if (!r.ok) throw new Error(`${rel}: neither in the repository nor on GitHub (${r.status})`);
  return r.text();
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

const [zh, generals, privacy, terms] = await Promise.all([read("index/zero-hour.json"), read("index/generals.json"), read("PRIVACY.md"), read("TERMS.md")]);
fs.writeFileSync(path.join(out, "index.json"), JSON.stringify({ zh: JSON.parse(zh), generals: JSON.parse(generals) }));
fs.writeFileSync(path.join(out, "legal.json"), JSON.stringify({ privacy: markdown(privacy), terms: markdown(terms) }));
console.log("synced the mod index and the legal pages");
