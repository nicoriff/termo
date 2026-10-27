import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { TabGroup } from "$lib/tab-groups/tab-groups";

/** The tab groups kept by the backend (see src-tauri/src/tab_groups.rs). */
export function listTabGroups(): Promise<TabGroup[]> {
  return invoke("list_tab_groups");
}

/** Adds or replaces one group; returns the full list after saving. */
export function saveTabGroup(group: TabGroup): Promise<TabGroup[]> {
  return invoke("save_tab_group", { group });
}

/** Returns the remaining groups. */
export function deleteTabGroup(id: string): Promise<TabGroup[]> {
  return invoke("delete_tab_group", { id });
}

/** Every window hears about every change, its own included. */
export function onTabGroups(handler: (groups: TabGroup[]) => void): Promise<UnlistenFn> {
  return listen<TabGroup[]>("tab-groups", ({ payload }) => handler(payload));
}
