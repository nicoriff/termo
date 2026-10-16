import { invoke } from "@tauri-apps/api/core";
import type { LayoutNode } from "$lib/layout/layout-snapshot";

/** A console layout saved under a name (see src-tauri/src/layouts.rs). */
export interface SavedLayout {
  name: string;
  /** Unix seconds. */
  savedAt: number;
  root: LayoutNode;
}

export function listLayouts(): Promise<SavedLayout[]> {
  return invoke("list_layouts");
}

/** Returns the full list after saving. */
export function saveLayout(name: string, root: LayoutNode): Promise<SavedLayout[]> {
  return invoke("save_layout", { name, root });
}

/** Returns the remaining layouts. */
export function deleteLayout(name: string): Promise<SavedLayout[]> {
  return invoke("delete_layout", { name });
}

/** Records what this window has open, for consoles that outlive it (see
 * layouts::save_window_session); null forgets it. */
export function saveWindowSession(root: LayoutNode | null): Promise<void> {
  return invoke("save_window_session", { root });
}

/** What this window had open the last time it recorded it. */
export function windowSession(): Promise<LayoutNode | null> {
  return invoke("window_session");
}
