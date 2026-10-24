/**
 * The console the status bar is about — the tab in focus — and what's known
 * of it, shared by the items in ./items so each doesn't work it out again.
 */
import { agentStatusStore } from "$lib/ai-mode/agent-status-store.svelte";
import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
import { isConsoleTab } from "$lib/layout/split-tree";
import { sessionFolder, workspaceStore } from "$lib/layout/workspace-store.svelte";

class ActiveConsole {
  readonly session = $derived(workspaceStore.focusedSession);
  readonly tabId = $derived(this.session?.id ?? null);
  /** The active tab's console, when it is one (not a file or an agent's runs). */
  readonly console = $derived(this.session && isConsoleTab(this.session) ? this.session : null);
  /** Which agent Modo IA detected in it, by hooks or by reading the screen.
   * Null when there's none (or Modo IA is off, which is what stops both
   * sources from reporting). */
  readonly tool = $derived(this.tabId ? (aiStatusStore.byId[this.tabId]?.tool ?? null) : null);
  readonly aiEntry = $derived(this.tabId ? (aiStatusStore.byId[this.tabId] ?? null) : null);
  /** What its agent last reported: usage, context, cost and the rest. */
  readonly agent = $derived(this.tabId ? (agentStatusStore.byTab[this.tabId] ?? null) : null);
  readonly folder = $derived(this.session ? (sessionFolder(this.session) ?? null) : null);
}

export const activeConsole = new ActiveConsole();
