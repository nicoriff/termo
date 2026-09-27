// Copies the app version from package.json — its single source, which
// tauri.conf.json also reads — into the Rust crate (Cargo.toml and the
// crate's own entry in Cargo.lock). Run by `npm version` (see the "version"
// script), so a bump lands in all of them in the same commit.
import fs from "node:fs";

const root = new URL("..", import.meta.url);
const read = (path) => fs.readFileSync(new URL(path, root), "utf8");
const write = (path, text) => fs.writeFileSync(new URL(path, root), text);

const { version } = JSON.parse(read("package.json"));

function replaceOnce(path, pattern, replacement) {
  const text = read(path);
  if (!pattern.test(text)) throw new Error(`${path}: no version line matched ${pattern}`);
  write(path, text.replace(pattern, replacement));
}

replaceOnce("src-tauri/Cargo.toml", /^(\[package\][^[]*?\nversion = ")[^"]*(")/m, `$1${version}$2`);
replaceOnce("src-tauri/Cargo.lock", /(\nname = "termo"\r?\nversion = ")[^"]*(")/, `$1${version}$2`);

console.log(`version ${version} → Cargo.toml, Cargo.lock`);
