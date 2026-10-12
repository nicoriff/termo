/**
 * What a repository's changes come down to, for the panel's sections, the
 * status bar's item and the button's badge. Pure — no runes, no Tauri — so it
 * can be unit-tested in plain Node; anything shown comes back as a catalog
 * key for the component to translate.
 */

import type { GitMark } from "../core/explorer-client";
import type { MessageKey } from "../i18n/translate";

export interface ChangedFile {
  path: string;
  staged: GitMark | null;
  unstaged: GitMark | null;
  conflict: boolean;
}

export interface ChangeCounts {
  /** In the index, ready to commit. */
  staged: number;
  /** Tracked files changed on disk and not staged. */
  changed: number;
  untracked: number;
  conflicts: number;
  /** Files with anything at all. */
  total: number;
}

export function countChanges(files: readonly ChangedFile[]): ChangeCounts {
  const counts: ChangeCounts = { staged: 0, changed: 0, untracked: 0, conflicts: 0, total: files.length };
  for (const file of files) {
    if (file.conflict) counts.conflicts++;
    else {
      if (file.staged) counts.staged++;
      if (file.unstaged === "untracked") counts.untracked++;
      else if (file.unstaged) counts.changed++;
    }
  }
  return counts;
}

export type ChangeGroup = "conflicts" | "staged" | "changes" | "untracked";
export const CHANGE_GROUPS: ChangeGroup[] = ["conflicts", "staged", "changes", "untracked"];

/** The panel's sections. A file staged and then changed again shows in both
 * "staged" and "changes": they're two different sets of changes. */
export function groupChanges<T extends ChangedFile>(files: readonly T[]): Record<ChangeGroup, T[]> {
  const groups: Record<ChangeGroup, T[]> = { conflicts: [], staged: [], changes: [], untracked: [] };
  for (const file of files) {
    if (file.conflict) {
      groups.conflicts.push(file);
      continue;
    }
    if (file.staged) groups.staged.push(file);
    if (file.unstaged === "untracked") groups.untracked.push(file);
    else if (file.unstaged) groups.changes.push(file);
  }
  return groups;
}

/** The mark a file shows in a section: what's staged there, what's on disk
 * everywhere else. */
export function markIn(file: ChangedFile, group: ChangeGroup): GitMark {
  if (group === "conflicts") return "conflict";
  return (group === "staged" ? file.staged : file.unstaged) ?? "modified";
}

export const GROUP_LABEL: Record<ChangeGroup, MessageKey> = {
  conflicts: "git.changes.conflicts",
  staged: "git.changes.staged",
  changes: "git.changes.changes",
  untracked: "git.changes.untracked",
};

/** `+2 ~3 ?1 !1`: staged, changed, untracked, in conflict — only what there
 * is, in that order. Empty when the tree is clean. */
export function changesLabel(counts: ChangeCounts): string {
  const parts: string[] = [];
  if (counts.staged) parts.push(`+${counts.staged}`);
  if (counts.changed) parts.push(`~${counts.changed}`);
  if (counts.untracked) parts.push(`?${counts.untracked}`);
  if (counts.conflicts) parts.push(`!${counts.conflicts}`);
  return parts.join(" ");
}

/** `↑2 ↓1`: commits to push and to pull. Empty when level. */
export function syncLabel(ahead: number, behind: number): string {
  const parts: string[] = [];
  if (ahead) parts.push(`↑${ahead}`);
  if (behind) parts.push(`↓${behind}`);
  return parts.join(" ");
}

/** A path as the panel shows it: the name, and the folder it's in apart so
 * it can be dimmed. */
export function splitPath(path: string): { name: string; folder: string } {
  const cut = path.lastIndexOf("/");
  return cut === -1 ? { name: path, folder: "" } : { name: path.slice(cut + 1), folder: path.slice(0, cut) };
}

/** What a comma- or space-separated box of names holds, without repeats. */
export function parseNames(text: string): string[] {
  return [...new Set(text.split(/[,\n]/).map((name) => name.trim().replace(/^@/, "")).filter(Boolean))];
}

/** What changed between two lists of names: what to add and what to remove. */
export function diffNames(before: readonly string[], after: readonly string[]): { add: string[]; remove: string[] } {
  return {
    add: after.filter((name) => !before.includes(name)),
    remove: before.filter((name) => !after.includes(name)),
  };
}
