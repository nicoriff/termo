import type { ToolId } from "./pattern-table";

/** The agent conversation a console currently has open. */
export interface TabAgentSession {
  tool: ToolId;
  /** The agent's own session id — what its resume command takes. */
  agentSessionId: string;
  /** Where the agent was last working. The tab's own `cwd` is only the folder
   * it was *created* in, so this is what a restored console should open in. */
  cwd?: string;
}

/**
 * Which agent conversation is running in which tab, keyed by tab (SessionTab)
 * id like aiStatusStore and agentStatusStore.
 *
 * Both channels report it (hooks for either agent, Claude's status line), and
 * it is what lets a saved layout reopen not just the consoles but the
 * conversations that were in them. A console with no agent — or one whose
 * agent Termini can't identify, i.e. without hooks installed — simply never
 * appears here.
 */
class TabAgentSessions {
  byTab = $state<Record<string, TabAgentSession>>({});

  set(tabId: string, tool: ToolId, agentSessionId: string, cwd?: string): void {
    const current = this.byTab[tabId];
    // An event without a folder doesn't erase the one already known.
    const next: TabAgentSession = { tool, agentSessionId, cwd: cwd || current?.cwd };
    if (current?.tool === next.tool && current.agentSessionId === next.agentSessionId && current.cwd === next.cwd) {
      return;
    }
    this.byTab = { ...this.byTab, [tabId]: next };
  }

  remove(tabId: string): void {
    if (!(tabId in this.byTab)) return;
    const next = { ...this.byTab };
    delete next[tabId];
    this.byTab = next;
  }

  get count(): number {
    return Object.keys(this.byTab).length;
  }
}

export const tabAgentSessions = new TabAgentSessions();
