// Builds the launcher's interface as the interactive demo for the website: website/static/demo/.
//
//   node tools/build-demo.mjs             always build            (npm run build:demo)
//   node tools/build-demo.mjs --if-stale  build only if the interface changed since the last build
//   node tools/build-demo.mjs --check     exit 1 if the committed demo is older than the interface (CI)
//
// It is the same interface as the app, built with sample data and a few demo-only touches (a banner, a message
// interface the page drives it with, storage names of its own). The result is committed, so the website builds
// without the desktop app's tooling. A hash of the interface's source is stored next to it (source-hash.txt), which is
// how "older than the interface" is known; a pre-commit hook (lint-staged) keeps it current.

import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const out = path.join(root, "website", "static", "demo");
const stampFile = path.join(out, "source-hash.txt");
if (!out.endsWith(path.join("static", "demo"))) throw new Error("unexpected output folder: " + out);

/** What the demo is made of: the interface source, its static files, the build config and the locked dependencies. */
function sourceHash() {
  const files = [];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) { if (e.name !== "node_modules") walk(p); } else files.push(p);
    }
  };
  walk(path.join(root, "src"));
  walk(path.join(root, "static"));
  for (const f of ["svelte.config.js", "vite.config.js", "package-lock.json"]) files.push(path.join(root, f));
  const h = crypto.createHash("sha256");
  for (const f of files.map((f) => path.relative(root, f).split(path.sep).join("/")).sort()) {
    // line endings differ between checkouts (Windows / Linux): they must not change the hash
    h.update(f + "\0").update(fs.readFileSync(path.join(root, f), "utf8").replace(/\r\n/g, "\n")).update("\0");
  }
  return h.digest("hex");
}

const current = fs.existsSync(stampFile) ? fs.readFileSync(stampFile, "utf8").trim() : "";
const now = sourceHash();
const upToDate = current === now && fs.existsSync(path.join(out, "index.html"));

if (process.argv.includes("--check")) {
  if (upToDate) { console.log("the demo is up to date"); process.exit(0); }
  console.error("The interactive demo (website/static/demo) is older than the interface. Run `npm run build:demo` and commit the result.");
  process.exit(1);
}
if (process.argv.includes("--if-stale") && upToDate) {
  console.log("the demo is up to date");
  process.exit(0);
}

fs.rmSync(out, { recursive: true, force: true }); // only the generated demo folder
const r = spawnSync("npx", ["vite", "build"], {
  cwd: root,
  stdio: "inherit",
  shell: true,
  env: { ...process.env, GL_BASE: "/demo", GL_OUT: out, VITE_DEMO: "1" },
});
if (r.status !== 0) process.exit(r.status ?? 1);
if (!fs.existsSync(path.join(out, "index.html"))) throw new Error("the demo was not built: no index.html");
fs.writeFileSync(stampFile, now + "\n");

let bytes = 0;
const size = (d) => { for (const e of fs.readdirSync(d, { withFileTypes: true })) e.isDirectory() ? size(path.join(d, e.name)) : (bytes += fs.statSync(path.join(d, e.name)).size); };
size(out);
console.log(`demo built: ${path.relative(root, out)} (${(bytes / 1024).toFixed(0)} KB)`);
