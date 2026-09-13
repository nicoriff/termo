import { convertFileSrc, invoke } from "@tauri-apps/api/core";

let cachedList: Promise<string[]> | null = null;

/** Monospace-category Google Fonts — the only sensible choice for a terminal,
 * since xterm.js assumes fixed-width character cells. */
export function listMonospaceGoogleFonts(): Promise<string[]> {
  if (!cachedList) {
    cachedList = invoke<string[]>("list_google_monospace_fonts").catch((err) => {
      cachedList = null;
      throw err;
    });
  }
  return cachedList;
}

/** Downloads the font's woff2 file and saves it under the app's config
 * directory so it keeps working offline after the first selection, then
 * injects an @font-face rule pointing at that local file. */
export async function installGoogleFont(family: string): Promise<string> {
  const localPath = await invoke<string>("install_google_font", { family });
  injectFontFace(family, localPath);
  return localPath;
}

let previewStylesheetInjected = false;

/** Loads a lightweight preview stylesheet straight from Google (not through
 * Rust/local install) so the font picker can render every option in its own
 * typeface. The browser only actually fetches the woff2 for families whose
 * name text is rendered on screen, so this stays cheap even for the full
 * monospace list. Picking a font still triggers the real local install. */
export function injectPreviewStylesheet(families: string[]): void {
  if (previewStylesheetInjected || families.length === 0) return;
  previewStylesheetInjected = true;

  const params = families
    .map((family) => `family=${encodeURIComponent(family).replace(/%20/g, "+")}`)
    .join("&");
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = `https://fonts.googleapis.com/css2?${params}&display=swap`;
  document.head.appendChild(link);
}

function fontFaceStyleId(family: string): string {
  return `google-font-${family.replace(/[^a-zA-Z0-9]/g, "_")}`;
}

/** Re-applies an already-downloaded font's @font-face from its local path —
 * used on startup to restore a previously installed font without any network
 * access. */
export function injectFontFace(family: string, localPath: string): void {
  const styleId = fontFaceStyleId(family);
  if (document.getElementById(styleId)) return;

  const assetUrl = convertFileSrc(localPath);
  // Google Fonts installs are woff2; the Nerd Font install is a ttf.
  const format = localPath.toLowerCase().endsWith(".ttf") ? "truetype" : "woff2";
  const style = document.createElement("style");
  style.id = styleId;
  style.textContent = `@font-face { font-family: "${family}"; src: url("${assetUrl}") format("${format}"); font-display: swap; }`;
  document.head.appendChild(style);
}
