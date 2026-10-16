// Builds the update feed (latest.json, the format Tauri's updater uses) from `gh release view --json ...` output.
//
//   node tools/make-update-feed.mjs release.json > latest.json
//
// The setup program must have a matching .sig file among the release's assets (made when the release is built with
// TAURI_SIGNING_PRIVATE_KEY set). Installed copies verify that signature against the public key in tauri.conf.json.

import fs from "node:fs";

const rel = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const setup = rel.assets.find((a) => /-setup\.exe$/.test(a.name));
const sig = rel.assets.find((a) => a.name === `${setup?.name}.sig`);
if (!setup || !sig) {
  console.error(`The release ${rel.tagName} has no setup program with a .sig file next to it.`);
  process.exit(1);
}
const signature = (await (await fetch(sig.url)).text()).trim();
if (!signature) throw new Error("The signature file is empty");
const body = (rel.body || "").trim();
console.log(
  JSON.stringify(
    {
      version: rel.tagName.replace(/^v/, ""),
      notes: body.length > 600 ? body.slice(0, 600) + "…" : body,
      pub_date: rel.publishedAt,
      platforms: { "windows-x86_64": { signature, url: setup.url } },
    },
    null,
    2,
  ),
);
