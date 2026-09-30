// The body of a GitHub release: the version's entry in
// src/lib/app/changelog.json, in both languages, each after a marker the app
// splits on (see src/lib/app/release-notes.ts) and GitHub's page doesn't
// show. Run by the release workflow; fails when the version has no entry, so
// a release never goes out without saying what it brings.
//
//   node scripts/release-notes.mjs 0.8.0 > notes.md
import fs from "node:fs";

const version = (process.argv[2] ?? "").replace(/^v/, "");
const changelog = JSON.parse(fs.readFileSync(new URL("../src/lib/app/changelog.json", import.meta.url), "utf8"));
const entry = changelog.find((item) => item.version === version);

if (!entry || !entry.es?.length || !entry.en?.length) {
  console.error(`changelog.json has no entry in both languages for version ${version || "(none given)"}`);
  process.exit(1);
}

const list = (items) => items.map((item) => `- ${item}`).join("\n");
process.stdout.write(`<!-- lang:es -->\n${list(entry.es)}\n\n<!-- lang:en -->\n${list(entry.en)}\n`);
