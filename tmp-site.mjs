import fs from "node:fs";
let s = fs.readFileSync("tools/build-site.mjs", "utf8");
const a = "s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, u) => `<a href=\"${u.replace(/^(PRIVACY|TERMS)\.md$/, (m, n) => n.toLowerCase() + \".html\")}\">${t}</a>`);";
if (!s.includes(a)) throw new Error("anchor");
s = s.replace(a, `s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, u) => \`<a href="\${link(u)}">\${t}</a>\`);`);
s = s.replace("function inline(s) {", `/** Links between the pages stay on the site; links to other files in the repository go to GitHub. */
function link(u) {
  if (/^https?:/.test(u)) return u;
  const page = /^(PRIVACY|TERMS)\.md$/.exec(u);
  return page ? page[1].toLowerCase() + ".html" : "https://github.com/soheilous/GenLauncher/tree/main/" + u;
}

function inline(s) {`);
fs.writeFileSync("tools/build-site.mjs", s);
