/** What the Edit menu and the editing shortcuts need from a live console.
 * Kept to this narrow surface (rather than handing out the TerminalInstance)
 * so nothing else starts reaching into xterm from across the app. */
export interface TerminalHandle {
  hasSelection(): boolean;
  selection(): string;
  write(text: string): void;
  selectAll(): void;
  clear(): void;
  focus(): void;
}

/** Keyed by tab (SessionTab) id, like the other per-console stores. */
const byTab = new Map<string, TerminalHandle>();

export function registerTerminal(tabId: string, handle: TerminalHandle): void {
  byTab.set(tabId, handle);
}

export function unregisterTerminal(tabId: string): void {
  byTab.delete(tabId);
}

export function getTerminal(tabId: string): TerminalHandle | undefined {
  return byTab.get(tabId);
}
