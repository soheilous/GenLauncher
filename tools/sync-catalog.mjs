// Refreshes the snapshot of the mod catalog that is built into the launcher (src-tauri/bundled/), so a release works
// offline and on first start. Run it before a release:
//
//   node tools/sync-catalog.mjs
//
// It copies catalog/zero-hour.json and catalog/generals.json from a sibling checkout of generals-mod-catalog when there is
// one (../generals-mod-catalog), otherwise from GitHub.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const sibling = path.join(root, "..", "generals-mod-catalog", "catalog");
const RAW = "https://raw.githubusercontent.com/soheilous/generals-mod-catalog/main/catalog/";
const out = path.join(root, "src-tauri", "bundled");
fs.mkdirSync(out, { recursive: true });

for (const file of ["zero-hour.json", "generals.json"]) {
  const local = path.join(sibling, file);
  const text = fs.existsSync(local) ? fs.readFileSync(local, "utf8") : await (async () => {
    const r = await fetch(RAW + file);
    if (!r.ok) throw new Error(`${file}: ${r.status}`);
    return r.text();
  })();
  const parsed = JSON.parse(text); // refuse to bundle something that is not valid
  if (!Array.isArray(parsed.mods) || !parsed.mods.length) throw new Error(file + " has no mods");
  fs.writeFileSync(path.join(out, file), text);
  console.log(`${file}: ${parsed.mods.length} mods (updated ${parsed.updated})`);
}
