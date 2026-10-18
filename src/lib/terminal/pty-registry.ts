// Tab (SessionTab) id -> PTY id. A TerminalInstance only learns its PTY id
// after the shell finishes spawning, while the rest of the UI (workspace
// store, status bar) identifies consoles by their tab id.
const ptyIdByTab = new Map<string, string>();

export function registerPty(tabId: string, ptyId: string): void {
  ptyIdByTab.set(tabId, ptyId);
}

export function unregisterPty(tabId: string): void {
  ptyIdByTab.delete(tabId);
}

export function getPtyId(tabId: string): string | undefined {
  return ptyIdByTab.get(tabId);
}

/** Every tab's PTY id, by tab id. */
export function ptyIdsByTab(): Record<string, string> {
  return Object.fromEntries(ptyIdByTab);
}

export function getTabId(ptyId: string): string | undefined {
  for (const [tabId, id] of ptyIdByTab) {
    if (id === ptyId) return tabId;
  }
  return undefined;
}
