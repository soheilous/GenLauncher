// Checks the translations. The code keeps the English text as the key (t("Install"), tn(n, "{n} item", "{n} items"),
// N("Label")); src/lib/i18n/locales/<code>.ts maps it to a language.
//
//   node tools/check-i18n.mjs                list what every language lacks
//   node tools/check-i18n.mjs --keys         print every text that needs translating (JSON)
//   node tools/check-i18n.mjs --missing fa   print what fa still lacks, as a JSON object ready to fill in
//
// Exit code 1 when a translation is broken: a text the code no longer has (stale), or {values} that do not match the
// English text. A language that simply lacks a text is allowed (it falls back to English) and only reported.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const srcDir = path.join(root, "src");
const localesDir = path.join(srcDir, "lib", "i18n", "locales");

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { if (!p.includes(path.join("lib", "i18n"))) walk(p, out); }
    else if (/\.(svelte|ts)$/.test(e.name)) out.push(p);
  }
  return out;
}

/** A JavaScript string literal as the string it stands for. */
const literal = (src) => new Function(`return ${src}`)();
const STR = String.raw`("(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'|\`(?:\\.|[^\`\\])*\`)`;

/** Every text the code asks to translate, with where it is. */
function extract() {
  const keys = new Map();
  const add = (text, file) => { if (!keys.has(text)) keys.set(text, new Set()); keys.get(text).add(path.relative(root, file)); };
  const t = new RegExp(String.raw`(?<![\w$.])(?:t|N)\(\s*${STR}`, "g");
  const tn = new RegExp(String.raw`(?<![\w$.])tn\(\s*[^,()]+(?:\([^()]*\))?[^,()]*,\s*${STR}\s*,\s*${STR}`, "g");
  for (const file of walk(srcDir)) {
    const text = fs.readFileSync(file, "utf8");
    for (const m of text.matchAll(t)) add(literal(m[1]), file);
    for (const m of text.matchAll(tn)) add(literal(m[2]), file); // the plural key is the "other" form
  }
  return keys;
}

function loadLocale(code) {
  const src = fs.readFileSync(path.join(localesDir, `${code}.ts`), "utf8");
  const start = src.indexOf("= {");
  const end = src.lastIndexOf("};");
  if (start < 0 || end < 0) throw new Error(`${code}.ts: expected "const messages: Messages = { ... };"`);
  return new Function(`return ${src.slice(start + 2, end + 1)}`)();
}

const vars = (s) => [...new Set([...String(s).matchAll(/\{(\w+)\}/g)].map((m) => m[1]))].sort().join(",");

const keys = extract();
const arg = process.argv[2];
if (arg === "--keys") {
  console.log(JSON.stringify([...keys.keys()].sort(), null, 2));
  process.exit(0);
}

const codes = fs.readdirSync(localesDir).filter((f) => f.endsWith(".ts")).map((f) => f.slice(0, -3)).sort();
if (arg === "--missing") {
  const code = process.argv[3];
  const have = loadLocale(code);
  const out = {};
  for (const k of [...keys.keys()].sort()) if (!(k in have)) out[k] = "";
  console.log(JSON.stringify(out, null, 2));
  process.exit(0);
}

let broken = 0;
console.log(`${keys.size} texts in the code\n`);
for (const code of codes) {
  const have = loadLocale(code);
  const missing = [...keys.keys()].filter((k) => !(k in have));
  const stale = Object.keys(have).filter((k) => !keys.has(k));
  const bad = [];
  for (const [k, v] of Object.entries(have)) {
    if (!keys.has(k)) continue;
    const forms = typeof v === "string" ? [v] : Object.values(v);
    // a plural text may drop {n} in a form (one: "one item"), but must not use values the English text does not have
    for (const f of forms) {
      const used = vars(f).split(",").filter(Boolean);
      const allowed = new Set(vars(k).split(",").filter(Boolean).concat(typeof v === "string" ? [] : ["n"]));
      if (typeof v === "string" ? vars(f) !== vars(k) : used.some((u) => !allowed.has(u))) bad.push(`${JSON.stringify(k)} -> ${JSON.stringify(f)}`);
    }
  }
  const pct = Math.round(((keys.size - missing.length) / keys.size) * 100);
  console.log(`${code.padEnd(3)} ${String(pct).padStart(3)}%  missing ${missing.length}, stale ${stale.length}, wrong values ${bad.length}`);
  for (const k of stale) console.log(`   stale: ${JSON.stringify(k)}`);
  for (const b of bad) console.log(`   values: ${b}`);
  broken += stale.length + bad.length;
}
process.exit(broken ? 1 : 0);
