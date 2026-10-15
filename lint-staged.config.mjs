// Runs on `git commit` for the files that are staged (see .husky/pre-commit).
//
// The interactive demo on the website (website/static/demo) is built from the interface's source, so a commit that
// changes the interface also has to carry the rebuilt demo. The tasks ignore the file names on purpose: they rebuild
// when needed and stage the result.
export default {
  "{src,static}/**": () => ["node tools/build-demo.mjs --if-stale", "git add website/static/demo"],
};
