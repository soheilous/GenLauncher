// Builds the launcher's interface as the interactive demo for the website: website/static/demo/.
//
//   node tools/build-demo.mjs        (or: npm run build:demo)
//
// It is the same interface as the app, built with sample data and a few demo-only touches (a banner, a message
// interface the page drives it with, storage names of its own). The result is committed, so the website builds
// without the desktop app's tooling. Run this again whenever the interface changes.

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const out = path.join(root, "website", "static", "demo");
if (!out.endsWith(path.join("static", "demo"))) throw new Error("unexpected output folder: " + out);

fs.rmSync(out, { recursive: true, force: true }); // only the generated demo folder
const r = spawnSync("npx", ["vite", "build"], {
  cwd: root,
  stdio: "inherit",
  shell: true,
  env: { ...process.env, GL_BASE: "/demo", GL_OUT: out, VITE_DEMO: "1" },
});
if (r.status !== 0) process.exit(r.status ?? 1);
if (!fs.existsSync(path.join(out, "index.html"))) throw new Error("the demo was not built: no index.html");

let bytes = 0;
const walk = (d) => { for (const e of fs.readdirSync(d, { withFileTypes: true })) e.isDirectory() ? walk(path.join(d, e.name)) : (bytes += fs.statSync(path.join(d, e.name)).size); };
walk(out);
console.log(`demo built: ${path.relative(root, out)} (${(bytes / 1024).toFixed(0)} KB)`);
