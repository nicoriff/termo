import { untrack } from "svelte";

/**
 * Each console's size in columns and rows, as xterm has it, for the status
 * bar. Keyed by tab (SessionTab) id, like the other per-console stores.
 */
export interface ConsoleSize {
  cols: number;
  rows: number;
}

class TabSizeStore {
  byTab = $state<Record<string, ConsoleSize>>({});

  set(tabId: string, cols: number, rows: number): void {
    // Untracked: it's called from inside a console's theme effect (through
    // resize), which would otherwise rerun whenever any console's size
    // changed — and with two consoles fitting at once, never stop.
    untrack(() => {
      const before = this.byTab[tabId];
      if (before?.cols === cols && before.rows === rows) return;
      this.byTab = { ...this.byTab, [tabId]: { cols, rows } };
    });
  }

  remove(tabId: string): void {
    if (!(tabId in this.byTab)) return;
    const next = { ...this.byTab };
    delete next[tabId];
    this.byTab = next;
  }
}

export const tabSize = new TabSizeStore();
