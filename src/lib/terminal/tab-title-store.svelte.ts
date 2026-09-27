import { aiStatusStore } from "../ai-mode/ai-status-store.svelte";
import { TOOL_DISPLAY_NAMES } from "../ai-mode/tool-names";
import { appConfigStore } from "../core/app-config.svelte";
import type { SessionTab } from "../layout/split-tree";
import {
  applyProcessChange,
  applyProgramTitle,
  consoleKind,
  consoleLabel,
  EMPTY_TAB_TITLE,
  processDisplayName,
  resolveTabTitle,
  type TabTitleState,
} from "./tab-title";

/** Keyed by tab (SessionTab) id, like aiStatusStore, so a title follows its
 * tab when it's dragged to another tile. */
class TabTitleStore {
  byTab = $state<Record<string, TabTitleState>>({});

  setProgramTitle(tabId: string, raw: string): void {
    this.update(tabId, (state) => applyProgramTitle(state, raw, Date.now()));
  }

  setForegroundProcess(tabId: string, path: string | null, shellPath?: string): void {
    this.update(tabId, (state) => applyProcessChange(state, processDisplayName(path, shellPath), Date.now()));
  }

  /** With nothing more specific from the program, a tab running an agent (as
   * Modo IA sees it) is named after the agent rather than the shell it
   * happens to run in. The store clears the session when the agent ends, so
   * it falls back to the console's own title on its own. */
  titleFor(tabId: string, fallback: string): string {
    const tool = appConfigStore.aiModeEnabled ? aiStatusStore.byId[tabId]?.tool : null;
    if (!tool) return resolveTabTitle(this.byTab[tabId], fallback);
    return resolveTabTitle(this.byTab[tabId], TOOL_DISPLAY_NAMES[tool], [tool, TOOL_DISPLAY_NAMES[tool]]);
  }

  /** The agent Modo IA sees running in the tab, if any. */
  private toolIn(tabId: string) {
    return appConfigStore.aiModeEnabled ? (aiStatusStore.byId[tabId]?.tool ?? null) : null;
  }

  /** Which icon the tab gets: the agent running in it, else its shell. */
  kindFor(session: SessionTab): string {
    return this.toolIn(session.id) ?? consoleKind(session.shellPath);
  }

  /** The console's type, spelled out (the status bar, the tab's tooltip):
   * the agent running in it, else the shell's own name. */
  typeFor(session: SessionTab): string {
    const tool = this.toolIn(session.id);
    return tool ? TOOL_DISPLAY_NAMES[tool] : consoleLabel(session.title);
  }

  remove(tabId: string): void {
    if (!(tabId in this.byTab)) return;
    const next = { ...this.byTab };
    delete next[tabId];
    this.byTab = next;
  }

  private update(tabId: string, change: (state: TabTitleState) => TabTitleState): void {
    const prev = this.byTab[tabId] ?? EMPTY_TAB_TITLE;
    const next = change(prev);
    if (next !== prev) this.byTab = { ...this.byTab, [tabId]: next };
  }
}

export const tabTitleStore = new TabTitleStore();
