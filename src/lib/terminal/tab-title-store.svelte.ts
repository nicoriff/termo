import { aiStatusStore } from "../ai-mode/ai-status-store.svelte";
import { TOOL_DISPLAY_NAMES } from "../ai-mode/tool-names";
import { appConfigStore } from "../core/app-config.svelte";
import {
  applyProcessChange,
  applyProgramTitle,
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

  /** While Modo IA has an agent session in this tab, the brackets name the
   * agent ("[Claude Code] fixing the parser") rather than the shell it
   * happens to run in — which is the thing you're actually looking for when
   * scanning tabs. The store clears the session when the agent ends, so it
   * falls back to the console's own title on its own. */
  titleFor(tabId: string, fallback: string): string {
    const tool = appConfigStore.aiModeEnabled ? aiStatusStore.byId[tabId]?.tool : null;
    if (!tool) return resolveTabTitle(this.byTab[tabId], fallback);
    return resolveTabTitle(this.byTab[tabId], TOOL_DISPLAY_NAMES[tool], [tool, TOOL_DISPLAY_NAMES[tool]]);
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
