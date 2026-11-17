/**
 * Paths to Markdown files in what a console prints — an agent saying it
 * wrote `docs/plan.md`, a `Write(C:\repo\NOTES.md)` — so Ctrl+click opens
 * them in the Markdown viewer (see TerminalInstance's link provider).
 *
 * Pure — no runes, no xterm — so it can be unit-tested in plain Node.
 */

export interface MarkdownPathMatch {
  /** Where the path starts and ends in the line, as string indices. */
  start: number;
  end: number;
  /** As printed, without what trails it (`:12`, a closing quote). */
  path: string;
}

/** A run of path characters ending in a Markdown extension, not followed by
 * more of a name. Spaces aren't path characters here: a console line is
 * full of them, and a path with spaces is rare in what agents print. */
const MARKDOWN_PATH = /[\w.\-~/\\:@+%]*\.(?:md|markdown|mdx)(?![\w.\-/\\])/gi;

export function findMarkdownPaths(line: string): MarkdownPathMatch[] {
  const found: MarkdownPathMatch[] = [];
  for (const match of line.matchAll(MARKDOWN_PATH)) {
    let path = match[0];
    let start = match.index;
    // A web address is the web-links addon's, not a file.
    const before = line.slice(Math.max(0, start - 3), start);
    if (/^[a-z][\w+.-]*:\/\//i.test(path) || before.endsWith("://")) continue;
    // A drive letter is the only colon a path keeps: "at:docs/x.md".
    const colon = path.lastIndexOf(":");
    if (colon === 0 || colon > 1) {
      start += colon + 1;
      path = path.slice(colon + 1);
    }
    if (!/[\w-]\.(?:md|markdown|mdx)$/i.test(path)) continue;
    found.push({ start, end: start + path.length, path });
  }
  return found;
}

/**
 * The absolute path `printed` names, read from `cwd` when it's relative.
 * Null when it can't be told, or when it's a network path: opening
 * `\\server\share\x.md` would have this computer reach a server named by
 * whatever the console printed (and Windows hand it the user's credentials).
 */
export function resolveMarkdownPath(printed: string, cwd: string | undefined, windows: boolean): string | null {
  const sep = windows ? "\\" : "/";
  let path = windows ? printed.replace(/\//g, "\\") : printed;
  if (/^[\\/]{2}/.test(path)) return null;
  const absolute = windows ? /^[a-z]:\\/i.test(path) : path.startsWith("/");
  if (windows && /^[\\/]/.test(path) && !absolute) return null;
  if (!absolute) {
    if (!cwd) return null;
    if (/^[\\/]{2}/.test(cwd)) return null;
    if (path.startsWith("~")) return null;
    const base = windows ? cwd.replace(/\//g, "\\") : cwd;
    path = base.replace(/[\\/]+$/, "") + sep + path;
  }
  return normalize(path, sep);
}

/** Folds `.` and `..` away, so the same file opened twice is one tab. */
function normalize(path: string, sep: string): string {
  const parts = path.split(sep);
  const head = parts.shift() ?? "";
  const kept: string[] = [];
  for (const part of parts) {
    if (part === "" || part === ".") continue;
    if (part === "..") kept.pop();
    else kept.push(part);
  }
  return [head, ...kept].join(sep);
}
