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

/** Mirrors src-tauri/src/agents/draft.rs. */
export interface AgentDraft {
  name: string;
  icon: string;
  task: string;
  instructions: string;
  autonomy: AgentAutonomy;
  web: boolean;
  schedule: { kind: "none" | "interval" | "daily"; minutes: number; at: string; weekdays: number[] };
  integrations: { query: string; why: string }[];
  explanation: string;
}

/** "Build it with AI": a first draft of the whole agent. Takes a while
 * (Claude answers through its CLI). */
export function draftAgent(description: string, workDir: string): Promise<AgentDraft> {
  return invoke("agent_draft", { description, workDir });
}

export interface AgentMcpRequest {
  agentId: string;
  serverName: string;
  remotes: unknown[];
  packages: unknown[];
  params: Record<string, string>;
  secretParams: string[];
}

/** An MCP server from the registry ready to add to the agent; its secret
 * values are stored in the system keychain on the way. */
export function agentMcpServer(request: AgentMcpRequest): Promise<{ server: McpServer; secrets: string[] }> {
  return invoke("agent_mcp_server", { request });
}

/** When each scheduled agent runs next on its own, Unix ms by agent id. */
export function agentsNextRuns(): Promise<Record<string, number>> {
  return invoke("agents_next_runs");
}

export function forgetAgentSecrets(agentId: string, names: string[]): Promise<void> {
  return names.length > 0 ? invoke("agent_forget_secrets", { agentId, names }) : Promise.resolve();
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
