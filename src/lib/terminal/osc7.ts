/**
 * OSC 7 — how a shell tells its terminal which directory it is in. The shell
 * prints `ESC ] 7 ; file://<host>/<path> BEL` on each prompt (see
 * src-tauri/src/pty/cwd_report.rs); xterm hands us everything after the `7;`.
 *
 * Pure — no runes, no xterm — so it can be unit-tested in plain Node, same
 * approach as `commands.ts` and `core/shortcuts.ts`.
 */

/** The directory, or null if this isn't a usable file:// location. */
export function parseOsc7(payload: string): string | null {
  const text = payload.trim();
  if (!text.toLowerCase().startsWith("file://")) return null;

  // Everything after the authority: file:///c:/x and file://host/c:/x both
  // leave "/c:/x". The host is ignored — a remote one isn't a path we can open.
  const afterScheme = text.slice("file://".length);
  const start = afterScheme.indexOf("/");
  if (start === -1) return null;

  let path = afterScheme.slice(start);
  try {
    path = decodeURIComponent(path);
  } catch {
    // A stray % that isn't an escape: better the raw path than nothing.
  }
  if (path === "") return null;

  // "/C:/Users/x" is a Windows path wearing URL clothes; "/home/x" is not.
  return /^\/[A-Za-z]:\//.test(path) ? path.slice(1).replace(/\//g, "\\") : path;
}
