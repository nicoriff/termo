/**
 * Path handling for the agent-launch folder browser: walking up a level,
 * splitting a path into breadcrumbs, and jumping by typed letters. Paths are
 * whatever the backend returns — `E:\Proyectos` on Windows, `/home/x`
 * elsewhere — so both shapes are handled.
 *
 * Pure — no runes, no Tauri — so it can be unit-tested in plain Node, same
 * approach as `terminal/osc7.ts`.
 */

const DRIVE_ROOT = /^[A-Za-z]:\\?$/;

function isWindowsPath(path: string): boolean {
  return /^[A-Za-z]:/.test(path);
}

function trimTrailingSeparator(path: string): string {
  if (path === "/" || DRIVE_ROOT.test(path)) return path;
  return path.replace(/[\\/]+$/, "");
}

/** The folder one level up, or null at a drive or filesystem root — the
 * level above those is the list of places to start from. */
export function parentPath(path: string): string | null {
  const trimmed = trimTrailingSeparator(path);
  if (trimmed === "/" || DRIVE_ROOT.test(trimmed)) return null;
  if (isWindowsPath(trimmed)) {
    const cut = trimmed.lastIndexOf("\\");
    // `E:\Proyectos` goes up to `E:\`, keeping the root's backslash.
    return cut <= 2 ? `${trimmed.slice(0, 2)}\\` : trimmed.slice(0, cut);
  }
  const cut = trimmed.lastIndexOf("/");
  return cut <= 0 ? "/" : trimmed.slice(0, cut);
}

/** Whether two paths name the same folder. Windows paths ignore case and
 * separator style; the rest compare exactly. */
export function samePath(a: string, b: string): boolean {
  const normalize = (path: string) => {
    const trimmed = trimTrailingSeparator(path);
    return isWindowsPath(trimmed) ? trimmed.replace(/\//g, "\\").replace(/\\$/, "").toLowerCase() : trimmed;
  };
  return normalize(a) === normalize(b);
}

export interface Crumb {
  label: string;
  path: string;
}

/** Each folder from the root down to `path`, for the breadcrumb bar. */
export function breadcrumbs(path: string): Crumb[] {
  const crumbs: Crumb[] = [];
  for (let current: string | null = path; current !== null; current = parentPath(current)) {
    const trimmed = trimTrailingSeparator(current);
    const label =
      trimmed === "/" ? "/" : DRIVE_ROOT.test(trimmed) ? trimmed.slice(0, 2) : trimmed.split(/[\\/]/).pop()!;
    crumbs.unshift({ label, path: current });
  }
  return crumbs;
}

/**
 * Where typing `query` lands in `names`: the first name starting with it,
 * searching forward from `from` and wrapping. A single letter starts after
 * `from`, so pressing the same letter again steps through every name that
 * starts with it; a longer query keeps refining in place.
 */
export function typeAheadIndex(names: readonly string[], query: string, from: number): number | null {
  if (!query || names.length === 0) return null;
  const needle = query.toLowerCase();
  const start = needle.length === 1 ? from + 1 : Math.max(from, 0);
  for (let step = 0; step < names.length; step++) {
    const index = (start + step) % names.length;
    if (names[index].toLowerCase().startsWith(needle)) return index;
  }
  return null;
}
