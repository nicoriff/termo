// What the updater reads (plugins.updater.endpoints in tauri.conf.json):
// `latest.json`, with each platform's signed build. Built once, from every
// build's `.sig`, after all of them are up — the builds run side by side,
// and each rewriting a shared latest.json could drop another's platform.

/**
 * The updater's platform key for a signature file, by the name the bundler
 * (and tauri-action) gives the file it signs; null for anything that isn't
 * an update artifact (an MSI, a .deb, a .dmg).
 */
export function platformOf(sigName) {
  const name = sigName.replace(/\.sig$/, "");
  if (name.endsWith("-setup.exe")) return "windows-x86_64";
  if (name.endsWith(".AppImage")) return /aarch64|arm64/i.test(name) ? "linux-aarch64" : "linux-x86_64";
  if (name.endsWith(".app.tar.gz")) {
    if (/aarch64|arm64/i.test(name)) return "darwin-aarch64";
    if (/x64|x86_64|intel/i.test(name)) return "darwin-x86_64";
  }
  return null;
}

/**
 * latest.json for a release. `signatures`: [{ name, signature }] for each
 * `.sig` asset; `download(name)`: the public URL of the file it signs.
 * Throws on two files claiming one platform, so a bad release never ships
 * a manifest that points at the wrong build.
 */
export function buildManifest({ version, notes, pubDate, signatures, download }) {
  const platforms = {};
  for (const { name, signature } of signatures) {
    const platform = platformOf(name);
    if (!platform) continue;
    if (platforms[platform]) throw new Error(`two update files for ${platform}: ${name}`);
    platforms[platform] = { signature: signature.trim(), url: download(name.replace(/\.sig$/, "")) };
  }
  if (Object.keys(platforms).length === 0) throw new Error("no update files in the release");
  return { version, notes, pub_date: pubDate, platforms };
}
