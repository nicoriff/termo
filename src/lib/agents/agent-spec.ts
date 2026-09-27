import type { AgentSpec, RunEvent, RunRecord } from "$lib/core/agents-client";
import type { MessageKey } from "$lib/i18n/translate";

/** The wizard's steps, in order. */
export const WIZARD_STEPS = ["idea", "logic", "engine", "tools", "triggers", "test", "deploy"] as const;
export type WizardStep = (typeof WIZARD_STEPS)[number];

export const STEP_LABEL_KEYS: Record<WizardStep, MessageKey> = {
  idea: "agents.wizard.steps.idea",
  logic: "agents.wizard.steps.logic",
  engine: "agents.wizard.steps.engine",
  tools: "agents.wizard.steps.tools",
  triggers: "agents.wizard.steps.triggers",
  test: "agents.wizard.steps.test",
  deploy: "agents.wizard.steps.deploy",
};

export const AGENT_ICONS = ["🤖", "🔍", "📝", "🧹", "📊", "🛠️", "📬", "🧪", "📚", "⚡"];

export function newAgentSpec(id: string): AgentSpec {
  return {
    id,
    name: "",
    description: "",
    icon: AGENT_ICONS[0],
    engine: "claude",
    model: "",
    workDir: "",
    task: "",
    instructions: "",
    autonomy: "read",
    tools: { web: false, mcp: [] },
    limits: { timeoutMin: 15 },
    triggers: { schedule: null, watch: null },
    secrets: [],
    enabled: true,
    deploy: { github: null },
    createdAt: 0,
    updatedAt: 0,
  };
}

/** What still has to be filled in before leaving `step`; empty when it's
 * complete. */
export function stepProblems(step: WizardStep, spec: AgentSpec): MessageKey[] {
  const problems: MessageKey[] = [];
  if (step === "logic") {
    if (!spec.name.trim()) problems.push("agents.wizard.problems.name");
    if (!spec.task.trim()) problems.push("agents.wizard.problems.task");
  }
  if (step === "engine") {
    if (!Number.isFinite(spec.limits.timeoutMin) || spec.limits.timeoutMin < 1)
      problems.push("agents.wizard.problems.timeout");
  }
  if (step === "triggers") {
    const schedule = spec.triggers.schedule;
    if (schedule?.kind === "interval" && !(schedule.minutes >= 1)) problems.push("agents.wizard.problems.interval");
    if (schedule?.kind === "daily" && !/^([01]\d|2[0-3]):[0-5]\d$/.test(schedule.at))
      problems.push("agents.wizard.problems.time");
    if (spec.triggers.watch && !spec.triggers.watch.path.trim()) problems.push("agents.wizard.problems.watch");
  }
  return problems;
}

/** Every problem across the whole spec, for the final save. */
export function specProblems(spec: AgentSpec): MessageKey[] {
  return WIZARD_STEPS.flatMap((step) => stepProblems(step, spec));
}

export type AgentTone = "running" | "success" | "error" | "idle" | "paused";

/** What the sidebar says about an agent: a tone for its dot and a line of
 * text (a catalog key with its parameters). */
export function agentState(
  spec: AgentSpec,
  lastRun: RunRecord | null,
): { tone: AgentTone; key: MessageKey; params?: Record<string, string> } {
  if (lastRun?.status === "running") {
    return lastRun.activity
      ? { tone: "running", key: "agents.state.runningDoing", params: { activity: lastRun.activity } }
      : { tone: "running", key: "agents.state.running" };
  }
  if (!spec.enabled) return { tone: "paused", key: "agents.state.paused" };
  if (!lastRun) return { tone: "idle", key: "agents.state.never" };
  if (lastRun.status === "success") return { tone: "success", key: "agents.state.lastOk" };
  if (lastRun.status === "stopped") return { tone: "idle", key: "agents.state.lastStopped" };
  return { tone: "error", key: "agents.state.lastFailed" };
}

/** "hace 5 min"-style age of a timestamp, as a catalog key and params. */
export function ago(thenMs: number, nowMs: number): { key: MessageKey; params?: Record<string, string> } {
  const seconds = Math.max(0, Math.round((nowMs - thenMs) / 1000));
  if (seconds < 60) return { key: "agents.time.justNow" };
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return { key: "agents.time.minutes", params: { n: String(minutes) } };
  const hours = Math.round(minutes / 60);
  if (hours < 24) return { key: "agents.time.hours", params: { n: String(hours) } };
  return { key: "agents.time.days", params: { n: String(Math.round(hours / 24)) } };
}

/** Claude says its final answer as a message and then again as the run's
 * result; the message is dropped so the answer shows once. */
export function withoutEchoes(events: RunEvent[]): RunEvent[] {
  const results = new Set(events.filter((event) => event.kind === "result").map((event) => event.text.trim()));
  return events.filter((event) => event.kind !== "text" || !results.has(event.text.trim()));
}

/** "1 min 05 s" for how long a run took. */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return minutes > 0 ? `${minutes} min ${String(seconds).padStart(2, "0")} s` : `${seconds} s`;
}
