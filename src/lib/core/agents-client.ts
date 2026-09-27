import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Mirrors src-tauri/src/agents/spec.rs. */
export type AgentEngine = "claude" | "codex";
export type AgentAutonomy = "read" | "edit" | "run";

export interface McpServer {
  name: string;
  config: Record<string, unknown>;
}

export type AgentSchedule =
  | { kind: "interval"; minutes: number }
  | { kind: "daily"; at: string; weekdays: number[] };

export interface AgentSpec {
  id: string;
  name: string;
  description: string;
  icon: string;
  engine: AgentEngine;
  model: string;
  workDir: string;
  task: string;
  instructions: string;
  autonomy: AgentAutonomy;
  tools: { web: boolean; mcp: McpServer[] };
  limits: { timeoutMin: number };
  triggers: { schedule: AgentSchedule | null; watch: { path: string } | null };
  secrets: string[];
  enabled: boolean;
  deploy: { github: { repoPath: string; workflowFile: string; deployedAt: number } | null };
  createdAt: number;
  updatedAt: number;
}

export type RunStatus = "running" | "success" | "error" | "stopped" | "timedOut";
export type RunTrigger = "manual" | "test" | "schedule" | "watch";

export interface RunRecord {
  id: string;
  agentId: string;
  trigger: RunTrigger;
  status: RunStatus;
  /** Unix milliseconds. */
  startedAt: number;
  endedAt: number | null;
  summary: string;
  activity: string;
  costUsd: number | null;
}

export type RunEventKind = "started" | "tool" | "text" | "result" | "stderr" | "ended";

export interface RunEvent {
  kind: RunEventKind;
  tool?: string;
  text: string;
  at: number;
}

export function listAgents(): Promise<AgentSpec[]> {
  return invoke("agents_list");
}

/** Returns the full list after saving. */
export function saveAgent(spec: AgentSpec): Promise<AgentSpec[]> {
  return invoke("agents_save", { spec });
}

export function deleteAgent(id: string): Promise<AgentSpec[]> {
  return invoke("agents_delete", { id });
}

/** Runs `spec` as given (saved or not); resolves once it has started. */
export function runAgent(spec: AgentSpec, trigger: RunTrigger): Promise<RunRecord> {
  return invoke("agent_run", { spec, trigger });
}

export function stopAgent(agentId: string): Promise<void> {
  return invoke("agent_stop", { agentId });
}

export function runningAgents(): Promise<RunRecord[]> {
  return invoke("agents_running");
}

/** Newest first. */
export function agentRuns(agentId: string): Promise<RunRecord[]> {
  return invoke("agent_runs", { agentId });
}

export function agentRunLog(agentId: string, runId: string): Promise<RunEvent[]> {
  return invoke("agent_run_log", { agentId, runId });
}

export function onAgentRun(handler: (run: RunRecord) => void): Promise<UnlistenFn> {
  return listen<RunRecord>("agent-run", (event) => handler(event.payload));
}

export function onAgentRunEvent(
  handler: (step: { agentId: string; runId: string; event: RunEvent }) => void,
): Promise<UnlistenFn> {
  return listen<{ agentId: string; runId: string; event: RunEvent }>("agent-run-event", (event) =>
    handler(event.payload),
  );
}
