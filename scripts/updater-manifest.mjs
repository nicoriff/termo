// What the updater reads (plugins.updater.endpoints in tauri.conf.json):
// `latest.json`, with each platform's signed build. Built once, from every
// build's `.sig`, after all of them are up — the builds run side by side,
// and each rewriting a shared latest.json could drop another's platform.

/** The architecture in a bundle's file name, as the updater names it. */
function arch(name, armPattern = /aarch64|arm64/i) {
  return armPattern.test(name) ? "aarch64" : "x86_64";
}

/**
 * The updater's platform keys for a signature file, by the name the bundler
 * (and tauri-action) gives the file it signs; empty for anything that isn't
 * an update artifact (a .dmg).
 *
 * The updater looks for `{os}-{arch}-{installer}` first — the installer
 * Termo was installed with — and only then for plain `{os}-{arch}`. So each
 * installer gets its own key: a Termo installed from the .deb must update
 * from the .deb (with dpkg), never from the AppImage, which it can't
 * install. The plain keys point at what an install of unknown origin can
 * always take: the AppImage, the NSIS setup and the .app.
 */
export function platformsOf(sigName) {
  const name = sigName.replace(/\.sig$/, "");
  if (name.endsWith("-setup.exe")) return ["windows-x86_64", "windows-x86_64-nsis"];
  if (name.endsWith(".msi")) return ["windows-x86_64-msi"];
  if (name.endsWith(".AppImage")) {
    const platform = `linux-${arch(name)}`;
    return [platform, `${platform}-appimage`];
  }
  if (name.endsWith(".deb")) return [`linux-${arch(name, /aarch64|arm64/i)}-deb`];
  if (name.endsWith(".rpm")) return [`linux-${arch(name)}-rpm`];
  if (name.endsWith(".app.tar.gz")) {
    let platform = null;
    if (/aarch64|arm64/i.test(name)) platform = "darwin-aarch64";
    else if (/x64|x86_64|intel/i.test(name)) platform = "darwin-x86_64";
    return platform ? [platform, `${platform}-app`] : [];
  }
  return [];
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
    for (const platform of platformsOf(name)) {
      if (platforms[platform]) throw new Error(`two update files for ${platform}: ${name}`);
      platforms[platform] = { signature: signature.trim(), url: download(name.replace(/\.sig$/, "")) };
    }
  }
  if (Object.keys(platforms).length === 0) throw new Error("no update files in the release");
  return { version, notes, pub_date: pubDate, platforms };
}
