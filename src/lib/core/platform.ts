/**
 * Read from the webview rather than @tauri-apps/plugin-os so it stays a
 * synchronous constant: the key handlers that consume it run on every
 * keystroke, and an async lookup would mean either awaiting mid-handler or
 * caching the result by hand. `navigator.platform` is formally deprecated
 * but still reported by every webview Tauri targets, and the userAgent
 * fallback covers a future where it isn't.
 */
export const isMac = /mac/i.test(navigator.platform || navigator.userAgent);
export const isWindows = /win/i.test(navigator.platform || navigator.userAgent);
export const isLinux = !isMac && !isWindows;
