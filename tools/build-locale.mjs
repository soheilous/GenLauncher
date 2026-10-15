// Turns a translator's numbered lines into a language file. Handy when translating many texts at once:
//
//   node tools/check-i18n.mjs --keys > keys.json          (the texts, in order)
//   node tools/build-locale.mjs fa fa.txt keys.json       (fa.txt: one "number|translation" line per text)
//
// Add --site for the website's languages (made with: node tools/check-i18n.mjs --site --keys).
//
// Plural texts take their forms as "one:text¦few:text¦other:text". Existing entries in the language file are kept unless
// the lines give a new translation for the same text. Lines starting with # and empty lines are ignored.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const site = process.argv.includes("--site");
const [code, file, keysFile] = process.argv.slice(2).filter((a) => a !== "--site");
if (!code || !file || !keysFile) {
  console.error("usage: node tools/build-locale.mjs <code> <lines.txt> <keys.json>");
  process.exit(2);
}
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const target = site ? path.join(root, "website", "src", "lib", "locales", `${code}.ts`) : path.join(root, "src", "lib", "i18n", "locales", `${code}.ts`);
const keys = JSON.parse(fs.readFileSync(keysFile, "utf8"));

// what the language file already says
const old = fs.existsSync(target) ? fs.readFileSync(target, "utf8") : "";
const s = old.indexOf("= {"), e = old.lastIndexOf("};");
const have = s >= 0 && e >= 0 ? new Function(`return ${old.slice(s + 2, e + 1)}`)() : {};

const seen = new Set();
let added = 0;
for (const [n, raw] of fs.readFileSync(file, "utf8").split(/\r?\n/).entries()) {
  const line = raw.trim();
  if (!line || line.startsWith("#")) continue;
  const bar = line.indexOf("|");
  const idx = Number(line.slice(0, bar));
  if (bar < 1 || !Number.isInteger(idx) || idx < 0 || idx >= keys.length) throw new Error(`line ${n + 1}: not "number|text": ${line.slice(0, 60)}`);
  if (seen.has(idx)) throw new Error(`line ${n + 1}: ${idx} is given twice`);
  seen.add(idx);
  const text = line.slice(bar + 1).trim();
  if (!text) continue;
  have[keys[idx]] = /^(zero|one|two|few|many|other):/.test(text) ? Object.fromEntries(text.split("¦").map((p) => { const i = p.indexOf(":"); return [p.slice(0, i).trim(), p.slice(i + 1).trim()]; })) : text;
  added++;
}
const body = Object.keys(have).sort().map((k) => `  ${JSON.stringify(k)}: ${JSON.stringify(have[k])},`).join("\n");
fs.writeFileSync(target, `import type { Messages } from "${site ? "../i18n" : "../index.svelte"}";\n\nconst messages: Messages = {\n${body}\n};\nexport default messages;\n`);
console.log(`${code}: ${added} lines read, ${Object.keys(have).length} texts in the file (${keys.length - Object.keys(have).filter((k) => keys.includes(k)).length} still missing)`);
