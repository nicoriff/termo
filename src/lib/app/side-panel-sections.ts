import type { Component } from "svelte";
import type { MessageKey } from "$lib/i18n/i18n.svelte";
import type { SidePanelId } from "./side-panel-layout";
import * as surfaces from "./lazy-surfaces";

/** What the side panel needs of each section: its component, and the names
 * its folded header and its close button show. */
export interface SidePanelSection {
  load: () => Promise<{ default: Component }>;
  title: MessageKey;
  close: MessageKey;
}

export const SIDE_PANEL_SECTIONS: Record<SidePanelId, SidePanelSection> = {
  explorer: { load: surfaces.loadExplorerPanel, title: "explorer.toggle", close: "explorer.close" },
  git: { load: surfaces.loadGitPanel, title: "git.title", close: "git.close" },
  harness: { load: surfaces.loadHarnessPanel, title: "harness.title", close: "harness.close" },
  tabGroups: { load: surfaces.loadTabGroupsPanel, title: "tabGroups.title", close: "tabGroups.closePanel" },
  kanban: { load: surfaces.loadKanbanSidebar, title: "kanban.title", close: "kanban.closePanel" },
  agents: { load: surfaces.loadAgentsSidebar, title: "agents.title", close: "agents.close" },
};
