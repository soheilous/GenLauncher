// Checks the mod index (index/*.json): its structure, and (unless --offline) that the pages and images it points to
// still exist. Needs Node 18+, no packages.
//
//   node tools/check-index.mjs            structure + links
//   node tools/check-index.mjs --offline  structure only (used for pull requests)
//
// Exit code 1 when the structure is wrong or a link is certainly dead (404, 410, unknown host). Sites that block
// scripts (ModDB answers 403 to anything that is not a browser) are reported as "unverified", not as failures.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const dir = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "index");
const offline = process.argv.includes("--offline");
const problems = [];
const links = new Map(); // url -> where it is used

const fail = (where, msg) => problems.push(`${where}: ${msg}`);

function checkUrl(where, field, value) {
  if (value === undefined || value === null) return;
  if (typeof value !== "string" || !/^https?:\/\/[^\s]+$/.test(value)) return fail(where, `${field} is not a web address: ${JSON.stringify(value)}`);
  if (/\.(zip|7z|rar|exe|msi)(\?|$)/i.test(new URL(value).pathname + new URL(value).search)) {
    fail(where, `${field} points at a file, the index lists pages only: ${value}`);
  }
  if (/insave\.ovh|s3host|:9000/i.test(value)) fail(where, `${field} points at the old mod storage: ${value}`);
  if (!links.has(value)) links.set(value, []);
  links.get(value).push(`${where} ${field}`);
}

function checkEntry(where, e, { needsName = true } = {}) {
  if (needsName && (typeof e.name !== "string" || !e.name.trim())) fail(where, "has no name");
  for (const f of ["page", "image", "discord", "news", "support", "network"]) {
    if (f === "network" && typeof e[f] === "string" && !/^https?:/.test(e[f])) continue; // free text in old data
    checkUrl(where, f, e[f]);
  }
  checkUrl(where, "colors.backgroundImage", e.colors?.backgroundImage);
  for (const k of Object.keys(e)) {
    if (/download|s3|secret|key|password|token/i.test(k)) fail(where, `field "${k}" must not be in the index`);
  }
}

for (const file of fs.readdirSync(dir).filter((f) => f.endsWith(".json") && f !== "announcements.json")) {
  let idx;
  try {
    idx = JSON.parse(fs.readFileSync(path.join(dir, file), "utf8"));
  } catch (e) {
    fail(file, `is not valid JSON (${e.message})`);
    continue;
  }
  if (idx.format !== 1) fail(file, `format should be 1, it is ${idx.format}`);
  if (!["zh", "generals"].includes(idx.game)) fail(file, `game should be "zh" or "generals"`);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(idx.updated ?? "")) fail(file, "updated should be YYYY-MM-DD");
  const names = new Set();
  for (const m of idx.mods ?? []) {
    const where = `${file} > ${m.name}`;
    checkEntry(where, m);
    const key = String(m.name).toLowerCase().replace(/[^a-z0-9]+/g, "-");
    if (names.has(key)) fail(where, "another mod has the same name");
    names.add(key);
    for (const kind of ["patches", "addons"]) {
      const seen = new Set();
      for (const x of m[kind] ?? []) {
        checkEntry(`${where} > ${x.name}`, x);
        if (seen.has(String(x.name).toLowerCase())) fail(`${where} > ${x.name}`, `listed twice under ${kind}`);
        seen.add(String(x.name).toLowerCase());
      }
    }
  }
  for (const kind of ["patches", "addons"]) for (const x of idx.originalGame?.[kind] ?? []) checkEntry(`${file} > original game > ${x.name}`, x);
  for (const x of idx.executables ?? []) {
    checkEntry(`${file} > ${x.name}`, x);
    if (x.for && !(idx.mods ?? []).some((m) => m.name.toLowerCase() === String(x.for).toLowerCase()) && x.for.toLowerCase() !== "original game") {
      fail(`${file} > ${x.name}`, `"for" names a mod that is not in the list: ${x.for}`);
    }
  }
}

const dead = [];
const unverified = [];
if (!offline) {
  const queue = [...links.keys()];
  const hard = /ENOTFOUND|certificate|ECONNREFUSED/i;
  async function probe(url) {
    for (const method of ["HEAD", "GET"]) {
      try {
        const r = await fetch(url, { method, redirect: "follow", signal: AbortSignal.timeout(20000), headers: { "user-agent": "GenLauncher-index-check (+https://github.com/soheilous/GenLauncher)" } });
        if (r.ok) return;
        if ([404, 410].includes(r.status)) return dead.push(`${url} -> ${r.status} (${links.get(url)[0]})`);
        if (method === "GET") return unverified.push(`${url} -> ${r.status}`);
      } catch (e) {
        const why = `${e.cause?.code ?? e.name}: ${e.cause?.message ?? e.message}`;
        if (method === "GET") return (hard.test(why) ? dead : unverified).push(`${url} -> ${why} (${links.get(url)[0]})`);
      }
    }
  }
  await Promise.all(Array.from({ length: 8 }, async () => { for (let u; (u = queue.shift()); ) await probe(u); }));
}

console.log(`${links.size} links${offline ? " (not fetched: --offline)" : ""}`);
if (unverified.length) console.log(`\nCould not be verified (often sites that block scripts), ${unverified.length}:\n  ` + unverified.join("\n  "));
if (dead.length) console.log(`\nDEAD, ${dead.length}:\n  ` + dead.join("\n  "));
if (problems.length) console.log(`\nPROBLEMS, ${problems.length}:\n  ` + problems.join("\n  "));
if (!dead.length && !problems.length) console.log("\nThe index looks fine.");
process.exit(dead.length || problems.length ? 1 : 0);
