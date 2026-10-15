// Builds the small project website (docs/) from the markdown pages in the repository, for GitHub Pages:
// https://soheilous.github.io/GenLauncher/   (Settings > Pages > "Deploy from a branch" > main, /docs)
//
//   node tools/build-site.mjs
//
// The pages are plain HTML, no scripts. Edit PRIVACY.md / TERMS.md and run this again.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const out = path.join(root, "docs");
fs.mkdirSync(out, { recursive: true });

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** The little bit of markdown these pages use: headings, paragraphs, bullet lists, **bold**, *italic*, links. */
/** Links between the pages stay on the site; links to other files in the repository go to GitHub. */
function link(u) {
  if (/^https?:/.test(u)) return u;
  const page = /^(PRIVACY|TERMS)\.md$/.exec(u);
  return page ? page[1].toLowerCase() + ".html" : "https://github.com/soheilous/GenLauncher/tree/main/" + u;
}

function inline(s) {
  s = esc(s);
  s = s.replace(/&lt;(https?:\/\/[^\s&]+)&gt;/g, '<a href="$1">$1</a>');
  s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, u) => `<a href="${link(u)}">${t}</a>`);
  s = s.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>").replace(/\*([^*]+)\*/g, "<em>$1</em>");
  return s;
}

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

const CSS = `
:root{color-scheme:light dark;--bg:#fff;--fg:#1d1a24;--dim:#625d70;--accent:#e8502a;--card:#f5f3f8}
@media (prefers-color-scheme:dark){:root{--bg:#15131c;--fg:#eeeaf5;--dim:#a29cb2;--accent:#ff7a4d;--card:#201d2b}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.65 system-ui,-apple-system,"Segoe UI",sans-serif}
header,main,footer{max-width:46rem;margin:0 auto;padding:1.2rem 1.25rem}
header{display:flex;align-items:center;gap:.8rem;padding-top:1.6rem}header img{width:44px;height:44px;border-radius:12px}
header a{color:var(--fg);text-decoration:none;font-weight:700;font-size:1.15rem}nav{margin-left:auto;display:flex;gap:1.1rem}
nav a{font-weight:500;font-size:.95rem;color:var(--dim)}a{color:var(--accent)}
h1{font-size:2rem;line-height:1.2;margin:1.2rem 0 .6rem}h2{font-size:1.25rem;margin:1.8rem 0 .4rem}p,li{color:var(--fg)}li{margin:.3rem 0}
.hero{padding:2.2rem 0 .5rem}.hero p{font-size:1.15rem;color:var(--dim);max-width:36rem}
.cards{display:grid;gap:1rem;grid-template-columns:repeat(auto-fit,minmax(14rem,1fr));margin:1.6rem 0}
.card{background:var(--card);border-radius:14px;padding:1rem 1.15rem}.card h3{margin:.1rem 0 .3rem;font-size:1.02rem}.card p{margin:0;color:var(--dim);font-size:.95rem}
.btn{display:inline-block;background:var(--accent);color:#fff;text-decoration:none;font-weight:600;padding:.65rem 1.15rem;border-radius:999px;margin:.4rem .5rem .4rem 0}
.btn.alt{background:var(--card);color:var(--fg)}footer{color:var(--dim);font-size:.9rem;border-top:1px solid var(--card);margin-top:2rem}
`;

const shell = (title, body, description) => `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>${esc(title)}</title><meta name="description" content="${esc(description)}">
<link rel="icon" href="logo.png"><style>${CSS}</style></head>
<body><header><img src="logo.png" alt=""><a href="./">GenLauncher</a>
<nav><a href="privacy.html">Privacy</a><a href="terms.html">Terms</a><a href="https://github.com/soheilous/GenLauncher">GitHub</a></nav></header>
<main>${body}</main>
<footer>GenLauncher is a free, open-source fan project. It is not affiliated with Electronic Arts, the authors of any mod, or Google.
<em>Command &amp; Conquer</em> is a trademark of Electronic Arts Inc.</footer></body></html>
`;

const DESC = "A modern mod launcher for Command & Conquer: Generals and Zero Hour.";

fs.writeFileSync(path.join(out, "index.html"), shell("GenLauncher", `
<section class="hero"><h1>A modern mod launcher for<br>Command &amp; Conquer: Generals and Zero Hour</h1>
<p>Install, switch between and start mods, patches, add-ons and alternate executables, then press Play. Your game folder is
put back the way it was when the game closes, even after a crash.</p>
<a class="btn" href="https://github.com/soheilous/GenLauncher">Get it on GitHub</a><a class="btn alt" href="privacy.html">Privacy</a><a class="btn alt" href="terms.html">Terms</a></section>
<div class="cards">
<div class="card"><h3>Mods from their authors</h3><p>Nothing is mirrored: the launcher opens a mod's own page (ModDB, Nexus Mods, an author's site) and you download it from there.</p></div>
<div class="card"><h3>Safe by design</h3><p>Changes to your game folder are journaled and undone when the game exits, or at the next start after a crash.</p></div>
<div class="card"><h3>Your settings, anywhere</h3><p>Export them to a file, or keep versioned backups in a hidden folder of your own Google Drive. Optional.</p></div>
</div>`, DESC), "utf8");

for (const [file, name, desc] of [["PRIVACY.md", "privacy", "How GenLauncher handles data."], ["TERMS.md", "terms", "The terms for using GenLauncher."]]) {
  const md = fs.readFileSync(path.join(root, file), "utf8");
  const title = /^#\s+(.*)$/m.exec(md)?.[1] ?? name;
  fs.writeFileSync(path.join(out, `${name}.html`), shell(`${title} · GenLauncher`, markdown(md), desc), "utf8");
}
fs.copyFileSync(path.join(root, "branding", "logo-512.png"), path.join(out, "logo.png"));
fs.writeFileSync(path.join(out, ".nojekyll"), "");
console.log("built", fs.readdirSync(out).join(", "));
