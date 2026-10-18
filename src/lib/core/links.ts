import { invoke } from "@tauri-apps/api/core";

/** A browser installed on this computer (see src-tauri/src/browser.rs). */
export interface Browser {
  name: string;
  /** What `link_browser` holds: the program, `.app` or `.desktop` entry. */
  path: string;
}

/** Opens a link with the browser chosen in Preferences → General, or the
 * system's default one. Only web and mail links open. Everything in Termo
 * that opens a link goes through here. */
export function openLink(url: string): Promise<void> {
  return invoke("open_link", { url });
}

export function listBrowsers(): Promise<Browser[]> {
  return invoke("list_browsers");
}

/** A web view opens `window.open` in a window of its own, not the user's
 * browser: anything that still calls it (a library's default) goes to
 * openLink instead. */
export function routeWindowOpen(): void {
  window.open = (url?: string | URL) => {
    const href = url ? String(url) : "";
    if (/^(https?:|mailto:)/i.test(href)) void openLink(href).catch(() => {});
    return null;
  };
}
