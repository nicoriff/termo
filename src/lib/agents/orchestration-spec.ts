import type { AgentEngine, RunEvent } from "$lib/core/agents-client";
import type {
  Catalog,
  CatalogModel,
  Orchestration,
  OrchestrationStatus,
  Plan,
  PlanChange,
  PlannedAgent,
  PlannedTask,
  Price,
  RevisionHold,
  TaskStatus,
} from "$lib/core/orchestration-client";
import type { MessageKey } from "$lib/i18n/translate";
import { withoutEchoes, type AgentTone } from "./agent-spec";

/** Room over the estimate for the budget a plan starts with (the same the
 * backend gives a draft: see planner.rs). */
const BUDGET_MARGIN = 1.3;
/** An estimate is a guess about tokens: shown as a range around it. */
const RANGE = { low: 0.6, high: 1.6 };

export const ENGINE_NAMES: Record<AgentEngine, string> = {
  claude: "Claude Code",
  codex: "Codex",
  gemini: "Gemini CLI",
  copilot: "Copilot CLI",
};

export const STATUS_KEYS: Record<OrchestrationStatus, MessageKey> = {
  draft: "agents.orchestration.status.draft",
  running: "agents.orchestration.status.running",
  awaitingApproval: "agents.orchestration.status.awaitingApproval",
  paused: "agents.orchestration.status.paused",
  done: "agents.orchestration.status.done",
  failed: "agents.orchestration.status.failed",
  stopped: "agents.orchestration.status.stopped",
};

export const TASK_STATUS_KEYS: Record<TaskStatus, MessageKey> = {
  pending: "agents.orchestration.task.pending",
  running: "agents.orchestration.task.running",
  done: "agents.orchestration.task.done",
  failed: "agents.orchestration.task.failed",
  dropped: "agents.orchestration.task.dropped",
};

export const HOLD_KEYS: Record<Exclude<RevisionHold, "none">, MessageKey> = {
  budget: "agents.orchestration.hold.budget",
  autonomy: "agents.orchestration.hold.autonomy",
  limit: "agents.orchestration.hold.limit",
};

export function statusTone(status: OrchestrationStatus): AgentTone {
  if (status === "running") return "running";
  if (status === "done") return "success";
  if (status === "failed") return "error";
  if (status === "awaitingApproval" || status === "paused") return "paused";
  return "idle";
}

export function taskTone(status: TaskStatus): AgentTone {
  if (status === "running") return "running";
  if (status === "done") return "success";
  if (status === "failed") return "error";
  return "idle";
}

export function modelsOf(catalog: Catalog, engine: AgentEngine): CatalogModel[] {
  return catalog.find((entry) => entry.engine === engine)?.models ?? [];
}

/** The price of a model on this computer's catalog; null when it has none
 * (the engine's default, Copilot, a model the catalog doesn't list). */
export function priceOf(catalog: Catalog, engine: AgentEngine, model: string): Price | null {
  return modelsOf(catalog, engine).find((option) => option.id === model)?.price ?? null;
}

export function agentOf(plan: Plan, task: PlannedTask): PlannedAgent | undefined {
  return plan.agents.find((agent) => agent.key === task.agent);
}

/** What one run of the task is expected to cost in USD; null without a price. */
export function taskEstimate(plan: Plan, task: PlannedTask, catalog: Catalog): number | null {
  const agent = agentOf(plan, task);
  const price = agent ? priceOf(catalog, agent.engine, agent.model) : null;
  if (!price) return null;
  return (task.tokensIn * price.input + task.tokensOut * price.output) / 1_000_000;
}

export interface Totals {
  tasks: number;
  tokensIn: number;
  tokensOut: number;
  /** The estimate for what has a price. */
  usd: number;
  /** Tasks whose model has no known price: not in `usd`. */
  unpriced: number;
}

function totalsOf(plan: Plan, tasks: PlannedTask[], catalog: Catalog): Totals {
  const totals: Totals = { tasks: 0, tokensIn: 0, tokensOut: 0, usd: 0, unpriced: 0 };
  for (const task of tasks) {
    if (task.status === "dropped") continue;
    totals.tasks += 1;
    totals.tokensIn += task.tokensIn;
    totals.tokensOut += task.tokensOut;
    const estimate = taskEstimate(plan, task, catalog);
    if (estimate === null) totals.unpriced += 1;
    else totals.usd += estimate;
  }
  return totals;
}

/** What the plan was estimated at, before anything ran. */
export function planTotals(plan: Plan, catalog: Catalog): Totals {
  return totalsOf(plan, plan.tasks, catalog);
}

export function agentTotals(plan: Plan, agentKey: string, catalog: Catalog): Totals {
  return totalsOf(
    plan,
    plan.tasks.filter((task) => task.agent === agentKey),
    catalog,
  );
}

/** What its runs are known to have cost so far. */
export function spentUsd(plan: Plan): number {
  return plan.tasks.reduce((sum, task) => sum + (task.costUsd ?? 0), 0);
}

/** What the whole plan is expected to cost, start to end: what's known for
 * what ran, the estimate for the rest (the backend's rule: see plan.rs). */
export function projectedUsd(plan: Plan, catalog: Catalog): number {
  let total = 0;
  for (const task of plan.tasks) {
    const estimate = taskEstimate(plan, task, catalog) ?? 0;
    if (task.status === "dropped") total += task.costUsd ?? 0;
    else if (task.status === "done" && task.costUsd !== null) total += task.costUsd;
    else total += (task.costUsd ?? 0) + estimate;
  }
  return total;
}

/** The budget a plan starts with; 0 (no limit) when nothing has a price. */
export function defaultBudget(plan: Plan, catalog: Catalog): number {
  return Math.round(planTotals(plan, catalog).usd * BUDGET_MARGIN * 100) / 100;
}

export function costRange(usd: number): { low: number; high: number } {
  return { low: usd * RANGE.low, high: usd * RANGE.high };
}

export function formatUsd(usd: number): string {
  const digits = usd > 0 && usd < 0.1 ? 3 : 2;
  return `US$ ${usd.toFixed(digits)}`;
}

/** 1 250 000 → "1.3M", 42 000 → "42k". */
export function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
  if (tokens >= 1_000) return `${Math.round(tokens / 1_000)}k`;
  return String(tokens);
}

/** The plan with one agent moved to another engine and model. */
export function withModel(plan: Plan, agentKey: string, engine: AgentEngine, model: string): Plan {
  return {
    ...plan,
    agents: plan.agents.map((agent) => (agent.key === agentKey ? { ...agent, engine, model } : agent)),
  };
}

export function withInstructions(plan: Plan, agentKey: string, instructions: string): Plan {
  return {
    ...plan,
    agents: plan.agents.map((agent) => (agent.key === agentKey ? { ...agent, instructions } : agent)),
  };
}

/** The request of one task: what it asks for and when it's done. */
export function withTaskRequest(plan: Plan, taskKey: string, description: string, doneWhen: string): Plan {
  return {
    ...plan,
    tasks: plan.tasks.map((task) => (task.key === taskKey ? { ...task, description, doneWhen } : task)),
  };
}

/** How the tasks ended up: for the final summary. */
export function taskCounts(plan: Plan): { total: number; done: number; failed: number; dropped: number; pending: number } {
  const count = (status: PlannedTask["status"]) => plan.tasks.filter((task) => task.status === status).length;
  return {
    total: plan.tasks.length,
    done: count("done"),
    failed: count("failed"),
    dropped: count("dropped"),
    pending: count("pending") + count("running"),
  };
}

/** Whether it has ended in a way there's something to sum up. */
export function hasEnded(status: OrchestrationStatus): boolean {
  return status === "done" || status === "failed" || status === "stopped" || status === "paused";
}

export interface ConsoleLine {
  source: string;
  event: RunEvent;
}

/** Several runs' steps as one console: in the order they happened, each
 * run's own order kept on a tie, a step heard twice (read from disk and
 * heard live) once, and a result's text not repeated as the text before it. */
export function mergeRunLogs(logs: { source: string; events: RunEvent[] }[]): ConsoleLine[] {
  const lines = logs.flatMap(({ source, events }) => {
    const seen = new Set<string>();
    const unique = events.filter((event) => {
      const key = `${event.at}|${event.kind}|${event.tool ?? ""}|${event.text}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    return withoutEchoes(unique).map((event) => ({ source, event }));
  });
  return lines.sort((a, b) => a.event.at - b.event.at);
}

/** A key not used by any of `taken`: the base, or the base with a number. */
export function uniqueKey(base: string, taken: string[]): string {
  if (!taken.includes(base)) return base;
  let n = 2;
  while (taken.includes(`${base}-${n}`)) n += 1;
  return `${base}-${n}`;
}

/** Whether any of the agent's tasks has run: then it can't leave the plan. */
export function agentHasRun(plan: Plan, agentKey: string): boolean {
  return plan.tasks.some((task) => task.agent === agentKey && !isTaskEditable(task));
}

/** A new agent on `engine`'s default model, that only looks until told more. */
export function addAgent(plan: Plan, engine: AgentEngine, name: string): { plan: Plan; key: string } {
  const key = uniqueKey("agent", plan.agents.map((agent) => agent.key));
  const agent: PlannedAgent = {
    key,
    name,
    icon: "🤖",
    specialty: "",
    engine,
    model: "",
    whyModel: "",
    autonomy: "read",
    web: false,
    instructions: "",
    agentId: "",
  };
  return { plan: { ...plan, agents: [...plan.agents, agent] }, key };
}

/** The agent and its tasks, and every mention of those tasks. */
export function removeAgent(plan: Plan, agentKey: string): Plan {
  const gone = new Set(plan.tasks.filter((task) => task.agent === agentKey).map((task) => task.key));
  return {
    agents: plan.agents.filter((agent) => agent.key !== agentKey),
    tasks: plan.tasks
      .filter((task) => !gone.has(task.key))
      .map((task) => ({ ...task, dependsOn: task.dependsOn.filter((key) => !gone.has(key)) })),
  };
}

export function withAgentFields(
  plan: Plan,
  agentKey: string,
  fields: Partial<Pick<PlannedAgent, "name" | "specialty" | "autonomy" | "web">>,
): Plan {
  return { ...plan, agents: plan.agents.map((agent) => (agent.key === agentKey ? { ...agent, ...fields } : agent)) };
}

/** A new task for the agent, after its last one so they run in order. */
export function addTask(plan: Plan, agentKey: string, title: string): { plan: Plan; key: string } {
  const key = uniqueKey("task", plan.tasks.map((task) => task.key));
  const last = plan.tasks.filter((task) => task.agent === agentKey).at(-1);
  const task: PlannedTask = {
    key,
    title,
    description: "",
    agent: agentKey,
    dependsOn: last ? [last.key] : [],
    doneWhen: "",
    tokensIn: 300_000,
    tokensOut: 10_000,
    status: "pending",
    runId: "",
    summary: "",
    costUsd: null,
    attempts: 0,
  };
  return { plan: { ...plan, tasks: [...plan.tasks, task] }, key };
}

/** The task, and every other task's wait for it. */
export function removeTask(plan: Plan, taskKey: string): Plan {
  return {
    ...plan,
    tasks: plan.tasks
      .filter((task) => task.key !== taskKey)
      .map((task) => ({ ...task, dependsOn: task.dependsOn.filter((key) => key !== taskKey) })),
  };
}

export function withTaskFields(
  plan: Plan,
  taskKey: string,
  fields: Partial<Pick<PlannedTask, "title" | "agent" | "dependsOn">>,
): Plan {
  return { ...plan, tasks: plan.tasks.map((task) => (task.key === taskKey ? { ...task, ...fields } : task)) };
}

/** Whether `taskKey` may wait for `otherKey`: not itself, and not a task
 * that already waits for it (directly or further down), which would loop. */
export function canDependOn(plan: Plan, taskKey: string, otherKey: string): boolean {
  if (taskKey === otherKey) return false;
  const seen = new Set<string>();
  const stack = [otherKey];
  while (stack.length > 0) {
    const key = stack.pop()!;
    if (key === taskKey) return false;
    if (seen.has(key)) continue;
    seen.add(key);
    stack.push(...(plan.tasks.find((task) => task.key === key)?.dependsOn ?? []));
  }
  return true;
}

/** Tasks a run would skip: nothing asked of them. */
export function tasksWithoutRequest(plan: Plan): PlannedTask[] {
  return plan.tasks.filter((task) => isTaskEditable(task) && task.description.trim() === "");
}

/** Whether a task's request can still change: it hasn't run, or it has to
 * run again. */
export function isTaskEditable(task: PlannedTask): boolean {
  return task.status !== "running" && task.status !== "done";
}

/** Whether the plan of an orchestration that isn't a draft can be edited:
 * when nothing of it is running. */
export function canEditPlan(status: OrchestrationStatus): boolean {
  return status === "stopped" || status === "paused" || status === "failed" || status === "done";
}

export interface TreeBranch {
  agent: PlannedAgent;
  tasks: PlannedTask[];
}

/** The tree the plan is shown as: the orchestrator at the root, each agent
 * under it, and under each agent its tasks in the order they'll run. */
export function treeOf(plan: Plan): TreeBranch[] {
  return plan.agents.map((agent) => ({ agent, tasks: plan.tasks.filter((task) => task.agent === agent.key) }));
}

/** The titles of the tasks one depends on, for "after …". */
export function dependencyTitles(plan: Plan, task: PlannedTask): string[] {
  return task.dependsOn.map((key) => plan.tasks.find((other) => other.key === key)?.title ?? key);
}

export interface Progress {
  done: number;
  total: number;
  running: number;
  failed: number;
}

export function progressOf(plan: Plan): Progress {
  const live = plan.tasks.filter((task) => task.status !== "dropped");
  return {
    done: live.filter((task) => task.status === "done").length,
    total: live.length,
    running: live.filter((task) => task.status === "running").length,
    failed: live.filter((task) => task.status === "failed").length,
  };
}

/** One change of a revision, in words. */
export function changeLabel(change: PlanChange, plan: Plan): { key: MessageKey; params: Record<string, string> } {
  const task = (key: string) => plan.tasks.find((other) => other.key === key)?.title ?? key;
  const agent = (key: string) => plan.agents.find((other) => other.key === key)?.name ?? key;
  switch (change.kind) {
    case "addAgent":
      return { key: "agents.orchestration.change.addAgent", params: { agent: change.agent.name || change.agent.key } };
    case "addTask":
      return {
        key: "agents.orchestration.change.addTask",
        params: { task: change.task.title || change.task.key, agent: agent(change.task.agent) },
      };
    case "reassign":
      return { key: "agents.orchestration.change.reassign", params: { task: task(change.task), agent: agent(change.agent) } };
    case "changeModel":
      return {
        key: "agents.orchestration.change.changeModel",
        params: { agent: agent(change.agent), model: `${ENGINE_NAMES[change.engine]} · ${change.model || "—"}` },
      };
    case "dropTask":
      return { key: "agents.orchestration.change.dropTask", params: { task: task(change.task) } };
    case "retryTask":
      return { key: "agents.orchestration.change.retryTask", params: { task: task(change.task) } };
  }
}

/** The revision waiting for the user's answer, if any. */
export function pendingRevision(orchestration: Orchestration) {
  return orchestration.revisions.find((revision) => revision.state === "pending") ?? null;
}

/** Whether "Resume" applies: it stopped, ran out of budget or failed. */
export function canResume(status: OrchestrationStatus): boolean {
  return status === "stopped" || status === "paused" || status === "failed";
}
