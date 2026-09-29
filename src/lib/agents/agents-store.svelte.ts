import {
  agentRuns,
  agentsNextRuns,
  createProject,
  deleteAgent,
  deleteProject,
  exportAgents,
  exportProject,
  importAgents,
  importProject,
  type ImportReport,
  listAgents,
  listProjects,
  moveAgent,
  onAgentRun,
  restoreProject,
  runAgent,
  runningAgents,
  runProject,
  saveAgent,
  saveProject,
  setProjectEnabled,
  stopAgent,
  stopProject,
  type AgentSpec,
  type Project,
  type RunRecord,
  type RunTrigger,
} from "$lib/core/agents-client";

/** The sidebar's width: a default, and how far it can be dragged. */
export const SIDEBAR_WIDTH = { initial: 264, min: 200, max: 560 };
const WIDTH_KEY = "termo.agents.sidebarWidth";
const COLLAPSED_KEY = "termo.agents.collapsedProjects";

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

function loadCollapsed(): string[] {
  try {
    const saved = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return Array.isArray(saved) ? saved.filter((id) => typeof id === "string") : [];
  } catch {
    return [];
  }
}

/** The user's agents and what each is doing, kept by the backend
 * (src-tauri/src/agents). Runs report through the `agent-run` event, so the
 * state here follows runs started from anywhere — the sidebar, the wizard's
 * test, the scheduler. */
class AgentsStore {
  agents = $state<AgentSpec[]>([]);
  projects = $state<Project[]>([]);
  /** Projects folded shut in the sidebar (remembered per computer). */
  collapsed = $state<string[]>(loadCollapsed());
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
    this.projects = await listProjects().catch(() => []);
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

  project(id: string): Project | undefined {
    return this.projects.find((project) => project.id === id);
  }

  agentsOf(projectId: string): AgentSpec[] {
    return this.agents.filter((agent) => agent.projectId === projectId);
  }

  toggleCollapsed(projectId: string): void {
    this.collapsed = this.collapsed.includes(projectId)
      ? this.collapsed.filter((id) => id !== projectId)
      : [...this.collapsed, projectId];
    try {
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify(this.collapsed));
    } catch {
      // Only a convenience.
    }
  }

  async saveProject(project: Project): Promise<void> {
    this.projects = await saveProject($state.snapshot(project));
  }

  /** Saves a reviewed project with its agents (from a template or AI). */
  async createProject(project: Project, agents: AgentSpec[]): Promise<void> {
    [this.projects, this.agents] = await createProject($state.snapshot(project), $state.snapshot(agents));
    void this.refreshNextRuns();
  }

  async removeProject(id: string, deleteAgents: boolean): Promise<void> {
    [this.projects, this.agents] = await deleteProject(id, deleteAgents);
    void this.refreshNextRuns();
  }

  async moveAgent(agentId: string, projectId: string): Promise<void> {
    this.agents = await moveAgent(agentId, projectId);
    this.projects = await listProjects().catch(() => this.projects);
  }

  async setProjectEnabled(id: string, enabled: boolean): Promise<void> {
    this.agents = await setProjectEnabled(id, enabled);
    void this.refreshNextRuns();
  }

  runProject(id: string, rehearsal: boolean): Promise<RunRecord> {
    return runProject(id, rehearsal);
  }

  stopProject(id: string): Promise<void> {
    return stopProject(id);
  }

  exportProject(id: string, path: string): Promise<void> {
    return exportProject(id, path);
  }

  async importProject(path: string): Promise<ImportReport> {
    const [projects, agents, report] = await importProject(path);
    this.projects = projects;
    this.agents = agents;
    return report;
  }

  async restoreProject(id: string, at: number): Promise<void> {
    [this.projects, this.agents] = await restoreProject(id, at);
    void this.refreshNextRuns();
  }

  async remove(id: string): Promise<void> {
    this.agents = await deleteAgent(id);
    this.projects = await listProjects().catch(() => this.projects);
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
