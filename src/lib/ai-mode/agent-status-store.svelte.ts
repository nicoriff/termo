/** One of the plan's rate-limit windows. `windowMinutes` is what names it,
 * since the agents don't report the same windows: Claude has a 5-hour and a
 * weekly one, Codex a primary and an optional secondary whose length depends
 * on the plan (a free one is a single 30-day window). */
export interface PlanWindow {
  percentage: number;
  windowMinutes: number | null;
  resetsAt: number | null;
}

/** Plan usage for each console, as reported by the `agent-status` event —
 * from Claude Code's status line, or read back from Codex's rollout (see
 * src-tauri/src/pty/status_line.rs and codex_usage.rs). Everything is
 * optional: no agent reports usage before its first response, Claude's
 * windows need a Pro/Max subscription, and Codex reports no cost. */
export interface AgentStatus {
  model: string | null;
  contextUsedPercentage: number | null;
  costUsd: number | null;
  planWindows: PlanWindow[];
  spendLimitPercentage: number | null;
  /** What the context holds and the model's whole window, in tokens. */
  contextTokens?: number | null;
  contextWindowTokens?: number | null;
  /** Claude only: what the session changed and how long it has run. */
  linesAdded?: number | null;
  linesRemoved?: number | null;
  durationMs?: number | null;
  apiDurationMs?: number | null;
  agentVersion?: string | null;
  outputStyle?: string | null;
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
    return (this.byTab[tabId]?.planWindows.length ?? 0) > 0;
  }
}

export const agentStatusStore = new AgentStatusStore();
