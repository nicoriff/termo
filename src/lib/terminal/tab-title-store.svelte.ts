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

  titleFor(tabId: string, fallback: string): string {
    return resolveTabTitle(this.byTab[tabId], fallback);
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
