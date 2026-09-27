import { invoke } from "@tauri-apps/api/core";

/** See src-tauri/src/commands/explorer_commands.rs. */
export interface ExplorerRoot {
  path: string;
  isRepo: boolean;
}

export interface ExplorerEntry {
  name: string;
  path: string;
  isDir: boolean;
}

export type GitMark = "modified" | "added" | "untracked" | "deleted" | "renamed" | "conflict";

export function explorerRoot(path: string): Promise<ExplorerRoot> {
  return invoke("explorer_root", { path });
}

export function listEntries(path: string): Promise<ExplorerEntry[]> {
  return invoke("list_entries", { path });
}

/** Changed files by absolute path; empty outside a repository. */
export function gitStatus(root: string): Promise<Record<string, GitMark>> {
  return invoke("git_status", { root });
}

/** Rejects with a message ready to show (too large, not text…). */
export function readTextFile(path: string): Promise<string> {
  return invoke("read_text_file", { path });
}

export function writeTextFile(path: string, content: string): Promise<void> {
  return invoke("write_text_file", { path, content });
}
