// The release workflow's last step: once every build has uploaded its files
// to the draft release, writes latest.json from their signatures (see
// updater-manifest.mjs), uploads it, and publishes the release — the moment
// the updater starts seeing the new version.
//
// Env: GH_TOKEN (contents:write on the releases repo), RELEASES_REPO
// ("owner/name"), RELEASE_ID.
import { buildManifest } from "./updater-manifest.mjs";

const { GH_TOKEN, RELEASES_REPO, RELEASE_ID } = process.env;
if (!GH_TOKEN || !RELEASES_REPO || !RELEASE_ID) {
  console.error("GH_TOKEN, RELEASES_REPO and RELEASE_ID are required");
  process.exit(1);
}

const api = `https://api.github.com/repos/${RELEASES_REPO}`;
const headers = { Authorization: `Bearer ${GH_TOKEN}`, "X-GitHub-Api-Version": "2022-11-28" };

async function call(url, init = {}) {
  const response = await fetch(url, { ...init, headers: { ...headers, ...init.headers } });
  if (!response.ok) throw new Error(`${init.method ?? "GET"} ${url}: ${response.status} ${await response.text()}`);
  return response;
}

const release = await (await call(`${api}/releases/${RELEASE_ID}`)).json();
const assets = await (await call(`${api}/releases/${RELEASE_ID}/assets?per_page=100`)).json();

const signatures = [];
for (const asset of assets.filter((item) => item.name.endsWith(".sig"))) {
  const file = await call(asset.url, { headers: { Accept: "application/octet-stream" } });
  signatures.push({ name: asset.name, signature: await file.text() });
}

const manifest = buildManifest({
  version: release.tag_name.replace(/^v/, ""),
  notes: release.body ?? "",
  pubDate: new Date().toISOString(),
  signatures,
  // Where the file will be once the release is published (a draft's own
  // download URLs change when it is).
  download: (name) =>
    `https://github.com/${RELEASES_REPO}/releases/download/${release.tag_name}/${encodeURIComponent(name)}`,
});
console.log(`latest.json: ${Object.keys(manifest.platforms).join(", ")}`);

const stale = assets.find((item) => item.name === "latest.json");
if (stale) await call(`${api}/releases/assets/${stale.id}`, { method: "DELETE" });
const uploadUrl = release.upload_url.replace(/\{.*\}$/, "");
await call(`${uploadUrl}?name=latest.json`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(manifest, null, 2),
});

await call(`${api}/releases/${RELEASE_ID}`, {
  method: "PATCH",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ draft: false, make_latest: "true" }),
});
console.log(`published ${release.tag_name}`);
