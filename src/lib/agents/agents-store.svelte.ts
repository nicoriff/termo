import {
  agentRuns,
  deleteAgent,
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

/** The user's agents and what each is doing, kept by the backend
 * (src-tauri/src/agents). Runs report through the `agent-run` event, so the
 * state here follows runs started from anywhere — the sidebar, the wizard's
 * test, the scheduler. */
class AgentsStore {
  agents = $state<AgentSpec[]>([]);
  /** Each agent's current run, or its latest one. */
  lastRun = $state<Record<string, RunRecord>>({});
  /** The left sidebar. */
  open = $state(false);
  /** Ticks every 30 s while the sidebar is open, for "hace 5 min" labels. */
  now = $state(Date.now());
  private started = false;
  private timer: ReturnType<typeof setInterval> | null = null;

  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    await onAgentRun((run) => {
      this.lastRun = { ...this.lastRun, [run.agentId]: run };
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
  }

  isRunning(agentId: string): boolean {
    return this.lastRun[agentId]?.status === "running";
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
      this.timer = setInterval(() => (this.now = Date.now()), 30_000);
    }
  }

  async save(spec: AgentSpec): Promise<void> {
    this.agents = await saveAgent($state.snapshot(spec));
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
