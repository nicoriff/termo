import { deleteLayout, listLayouts, saveLayout, type SavedLayout } from "$lib/core/layouts-client";
import { resumeInTab } from "$lib/ai-mode/restore-session";
import { tabAgentSessions } from "$lib/ai-mode/tab-sessions.svelte";
import { tabCwd } from "$lib/terminal/tab-cwd.svelte";
import { hydrateLayout, snapshotLayout } from "./layout-snapshot";
import { workspaceStore } from "./workspace-store.svelte";

/** Saved console layouts, kept by the backend (src-tauri/src/layouts.rs). Only
 * the main window reads or writes them, so there's no cross-window event to
 * listen to: every mutation returns the new list. */
class LayoutsStore {
  layouts = $state<SavedLayout[]>([]);

  async load(): Promise<void> {
    this.layouts = await listLayouts().catch(() => []);
  }

  /**
   * Saves the tree as it is right now. An existing name is overwritten.
   * With `withSessions`, each console also records the agent conversation it
   * has open, so restoring reopens the conversations and not just the shells.
   */
  async save(name: string, withSessions = false): Promise<void> {
    const root = workspaceStore.root;
    if (!root) return;
    const snapshot = snapshotLayout(root, {
      agentByTab: tabAgentSessions.byTab,
      cwdByTab: tabCwd.byTab,
      withSessions,
    });
    this.layouts = await saveLayout(name, snapshot).catch(() => this.layouts);
  }

  async remove(name: string): Promise<void> {
    this.layouts = await deleteLayout(name).catch(() => this.layouts);
  }

  /** Replaces everything on screen: the consoles that were open are closed as
   * their tiles go away, and the layout's own consoles start fresh — each one
   * resuming its conversation if the layout was saved with sessions. */
  apply(layout: SavedLayout): void {
    const { root, resumes } = hydrateLayout(layout.root);
    workspaceStore.replaceRoot(root);
    for (const resume of resumes) {
      void resumeInTab(resume.tabId, resume.tool, resume.agentSessionId);
    }
  }
}

export const layoutsStore = new LayoutsStore();
