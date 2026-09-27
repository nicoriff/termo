/**
 * The explorer's tree logic: which rows are showing given the folders that
 * are open, and which folders contain changes git reported. Pure — no runes,
 * no Tauri — so it can be unit-tested in plain Node.
 */

export interface TreeEntry {
  name: string;
  path: string;
  isDir: boolean;
}

export interface TreeRow extends TreeEntry {
  depth: number;
}

/** Every row showing: the root's entries, and inside each open folder its
 * own, as far down as folders are open and their contents loaded. */
export function visibleRows(
  root: string,
  children: ReadonlyMap<string, readonly TreeEntry[]>,
  expanded: ReadonlySet<string>,
): TreeRow[] {
  const rows: TreeRow[] = [];
  const walk = (dir: string, depth: number) => {
    for (const entry of children.get(dir) ?? []) {
      rows.push({ ...entry, depth });
      if (entry.isDir && expanded.has(entry.path)) walk(entry.path, depth + 1);
    }
  };
  walk(root, 0);
  return rows;
}

/** Whether `folder` holds any of the changed paths, however deep. */
export function folderHasChanges(folder: string, changed: Iterable<string>): boolean {
  const prefix = folder.replace(/[\\/]+$/, "");
  for (const path of changed) {
    if (path.length > prefix.length && path.startsWith(prefix) && /[\\/]/.test(path[prefix.length])) return true;
  }
  return false;
}
