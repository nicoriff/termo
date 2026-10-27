/** Tabs whose console is about to be put to sleep (see tab-saver.svelte.ts):
 * as their <Terminal> unmounts, it lets go of the console instead of ending
 * it. Keyed by tab (SessionTab) id, like the other per-console stores. */
const releasing = new Set<string>();

export function markReleasing(tabId: string): void {
  releasing.add(tabId);
}

/** Whether the tab's console is to be let go of; asking clears it. */
export function takeReleasing(tabId: string): boolean {
  return releasing.delete(tabId);
}

/** What a discarded console last showed, for its new one to show first. */
const prefaces = new Map<string, string>();

export function setPreface(tabId: string, text: string): void {
  prefaces.set(tabId, text);
}

/** The text to show above the tab's new console, once; asking clears it. */
export function takePreface(tabId: string): string | undefined {
  const text = prefaces.get(tabId);
  prefaces.delete(tabId);
  return text;
}
