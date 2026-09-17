/**
 * Where each console currently is, as the shell itself reports it via OSC 7
 * (see osc7.ts and src-tauri/src/pty/cwd_report.rs). Keyed by tab (SessionTab)
 * id, like the other per-console stores.
 *
 * SessionTab.cwd is only the folder a console was *opened* in, so this is what
 * saving a layout should record: a console the user cd'd elsewhere must come
 * back where it is now.
 */
class TabCwdStore {
  byTab = $state<Record<string, string>>({});

  set(tabId: string, cwd: string): void {
    if (this.byTab[tabId] === cwd) return;
    this.byTab = { ...this.byTab, [tabId]: cwd };
  }

  remove(tabId: string): void {
    if (!(tabId in this.byTab)) return;
    const next = { ...this.byTab };
    delete next[tabId];
    this.byTab = next;
  }
}

export const tabCwd = new TabCwdStore();
