import {
  agentRuns,
  agentsNextRuns,
  deleteAgent,
  exportAgents,
  importAgents,
  type ImportReport,
  listAgents,
  onAgentRun,
  runAgent,
  runningAgents,
  saveAgent,
  stopAgent,
  type AgentSpec,
  type RunRecord,
  type RunTrigger,
} from "$lib/core/agents-client";

/** The sidebar's width: a default, and how far it can be dragged. */
export const SIDEBAR_WIDTH = { initial: 264, min: 200, max: 560 };
const WIDTH_KEY = "termo.agents.sidebarWidth";

export function clampSidebarWidth(width: number): number {
  return Math.round(Math.min(SIDEBAR_WIDTH.max, Math.max(SIDEBAR_WIDTH.min, width)));
}

/** Only a convenience: a browser storage that's missing or throws just means
 * the default width. */
function loadWidth(): number {
  try {
    const saved = Number(localStorage.getItem(WIDTH_KEY));
    return saved ? clampSidebarWidth(saved) : SIDEBAR_WIDTH.initial;
  } catch {
    return SIDEBAR_WIDTH.initial;
  }
}

/** The user's agents and what each is doing, kept by the backend
 * (src-tauri/src/agents). Runs report through the `agent-run` event, so the
 * state here follows runs started from anywhere — the sidebar, the wizard's
 * test, the scheduler. */
class AgentsStore {
  agents = $state<AgentSpec[]>([]);
  /** Each agent's current run, or its latest one. */
  lastRun = $state<Record<string, RunRecord>>({});
  /** When each scheduled agent runs next on its own. */
  nextRun = $state<Record<string, number>>({});
  /** The left sidebar. */
  open = $state(false);
  /** Its width in px, as the user last dragged it. */
  width = $state(loadWidth());
  /** Ticks every 30 s while the sidebar is open, for "hace 5 min" labels. */
  now = $state(Date.now());
  private started = false;
  private timer: ReturnType<typeof setInterval> | null = null;

  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    await onAgentRun((run) => {
      this.lastRun = { ...this.lastRun, [run.agentId]: run };
      // A run moves the next one.
      if (run.status !== "running") void this.refreshNextRuns();
    });
    this.agents = await listAgents().catch(() => []);
    const latest: Record<string, RunRecord> = {};
    await Promise.all(
      this.agents.map(async (agent) => {
        const [last] = await agentRuns(agent.id).catch(() => []);
        if (last) latest[agent.id] = last;
      }),
    );
    // A run marked "running" on disk but not running now was cut short by
    // the app closing; the live list is the truth.
    const live = await runningAgents().catch(() => []);
    for (const run of live) latest[run.agentId] = run;
    this.lastRun = { ...latest, ...this.lastRun };
    await this.refreshNextRuns();
  }

  async refreshNextRuns(): Promise<void> {
    this.nextRun = await agentsNextRuns().catch(() => this.nextRun);
  }

  isRunning(agentId: string): boolean {
    return this.lastRun[agentId]?.status === "running";
  }

  setWidth(width: number, persist = false): void {
    this.width = clampSidebarWidth(width);
    if (!persist) return;
    try {
      localStorage.setItem(WIDTH_KEY, String(this.width));
    } catch {
      // Not remembered next time; nothing else depends on it.
    }
  }

  toggle(): void {
    this.setOpen(!this.open);
  }

  setOpen(open: boolean): void {
    this.open = open;
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    if (open) {
      this.now = Date.now();
      this.timer = setInterval(() => {
        this.now = Date.now();
        void this.refreshNextRuns();
      }, 30_000);
    }
  }

  async save(spec: AgentSpec): Promise<void> {
    this.agents = await saveAgent($state.snapshot(spec));
    void this.refreshNextRuns();
  }

  async importFrom(paths: string[]): Promise<ImportReport> {
    const [agents, report] = await importAgents(paths);
    this.agents = agents;
    void this.refreshNextRuns();
    return report;
  }

  exportTo(ids: string[], path: string): Promise<void> {
    return exportAgents(ids, path);
  }

  async remove(id: string): Promise<void> {
    this.agents = await deleteAgent(id);
    const { [id]: _, ...rest } = this.lastRun;
    this.lastRun = rest;
  }

  async setEnabled(id: string, enabled: boolean): Promise<void> {
    const agent = this.agents.find((candidate) => candidate.id === id);
    if (agent) await this.save({ ...agent, enabled });
  }

  /** Starts a run; rejects with the backend's (translated) reason. */
  run(spec: AgentSpec, trigger: RunTrigger = "manual"): Promise<RunRecord> {
    return runAgent($state.snapshot(spec), trigger);
  }

  stop(agentId: string): Promise<void> {
    return stopAgent(agentId);
  }
}

export const agentsStore = new AgentsStore();
