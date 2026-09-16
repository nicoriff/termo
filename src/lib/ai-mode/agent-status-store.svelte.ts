/** Claude Code status-line data for each console, as reported by the
 * `agent-status` event (see src-tauri/src/pty/status_line.rs). Every field is
 * optional: `rate_limits` only exists for claude.ai Pro/Max sessions and only
 * after the first API response, and the windows disappear when they reset. */
export interface AgentStatus {
  model: string | null;
  contextUsedPercentage: number | null;
  costUsd: number | null;
  fiveHourPercentage: number | null;
  fiveHourResetsAt: number | null;
  sevenDayPercentage: number | null;
  sevenDayResetsAt: number | null;
  spendLimitPercentage: number | null;
}

/** Keyed by tab (SessionTab) id, like aiStatusStore and tabTitleStore. */
class AgentStatusStore {
  byTab = $state<Record<string, AgentStatus>>({});

  set(tabId: string, status: AgentStatus): void {
    this.byTab = { ...this.byTab, [tabId]: status };
  }

  remove(tabId: string): void {
    if (!(tabId in this.byTab)) return;
    const next = { ...this.byTab };
    delete next[tabId];
    this.byTab = next;
  }

  /** True when there's a plan-usage window worth showing for this tab. */
  hasPlanUsage(tabId: string | null): boolean {
    if (!tabId) return false;
    const status = this.byTab[tabId];
    return !!status && (status.fiveHourPercentage !== null || status.sevenDayPercentage !== null);
  }
}

export const agentStatusStore = new AgentStatusStore();
