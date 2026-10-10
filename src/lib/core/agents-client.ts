import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Mirrors src-tauri/src/agents/spec.rs. */
export type AgentEngine = "claude" | "codex" | "gemini" | "copilot" | "grok";
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
  triggers: {
    schedule: AgentSchedule | null;
    watch: { path: string } | null;
    /** Runs on a git event in `repoPath`, through a hook Termo installs. */
    git: { repoPath: string; event: "commit" | "push" } | null;
    /** Runs when its local URL is called with this token. */
    webhook: { token: string } | null;
    /** Runs when another agent finishes, with its result as context. */
    after: { agentId: string; on: AfterOutcome } | null;
  };
  /** Working with other agents (see src-tauri/src/agents/team.rs). */
  collaboration: {
    /** The agents it may ask or message, by id. */
    contacts: string[];
    /** Reading and writing the board the agents share. */
    board: boolean;
    /** Whether it may ask agents allowed to do more than itself. */
    allowEscalation: boolean;
    /** Asking the user mid-task, in a run someone is there for. */
    askUser?: boolean;
    /** Reachable from the user's consoles (Claude Code, Codex…). */
    fromConsoles?: boolean;
    /** One that runs commands is reachable only with this too. */
    consolesMayRun?: boolean;
  };
  secrets: string[];
  enabled: boolean;
  deploy: { github: { repo: string; repoPath: string; workflowFile: string; deployedAt: number } | null };
  /** The project it belongs to; empty: none. */
  projectId: string;
  createdAt: number;
  updatedAt: number;
}

export type AfterOutcome = "success" | "failure" | "any";

export type RunStatus = "running" | "success" | "error" | "stopped" | "timedOut";
export type RunTrigger =
  | "manual"
  | "test"
  | "schedule"
  | "watch"
  | "git"
  | "webhook"
  | "chain"
  | "agent"
  | "message"
  | "orchestrator"
  | "console";

/** Where a run sits in a conversation between agents. */
export interface RunLink {
  conversationId: string;
  parentRunId: string;
  parentAgentId: string;
  /** The agents from the conversation's first down to the parent. */
  path: string[];
  rehearsal: boolean;
}

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
  /** Set when another agent's run started this one. */
  link: RunLink | null;
  /** A rehearsal: it could only look. */
  rehearsal: boolean;
  /** Why it failed, said precisely. */
  failure?: RunFailure;
}

/** The conversation a run belongs to: its parent's, or its own. */
export function conversationOf(run: RunRecord): string {
  return run.link?.conversationId ?? run.id;
}

/** A model an agent can run on, as its CLI offers it. */
export interface ModelOption {
  /** What goes to --model / -m. */
  id: string;
  name: string;
  description: string;
  /** One the CLI puts first; the rest are older or niche. */
  main: boolean;
  /** A remark the CLI shows next to it. */
  note: string;
}

/** The models the engine's CLI offers this account; empty until it has run once. */
export function agentModels(engine: AgentEngine): Promise<ModelOption[]> {
  return invoke("agent_models", { engine });
}

/** Every run of a conversation between agents, oldest first. */
export function agentConversation(conversationId: string): Promise<RunRecord[]> {
  return invoke("agent_conversation", { conversationId });
}

/** A note on the board the agents share. */
export interface BoardEntry {
  value: string;
  /** The agent that wrote it. */
  by: string;
  /** Unix milliseconds. */
  at: number;
}

/** A team project's own board, or (null) the one agents outside a team share. */
export function agentsBoard(projectId: string | null = null): Promise<Record<string, BoardEntry>> {
  return invoke("agents_board", { projectId });
}

export function removeBoardEntry(key: string, projectId: string | null = null): Promise<Record<string, BoardEntry>> {
  return invoke("agents_board_remove", { projectId, key });
}

/** Mirrors src-tauri/src/agents/projects.rs. */
export interface Project {
  id: string;
  name: string;
  icon: string;
  goal: string;
  /** New agents in the project start with this folder. */
  workDir: string;
  /** A team: its agents only talk among themselves and share its own board. */
  isolated: boolean;
  /** The agent "Run project" starts. */
  entryAgentId: string;
  limits: { maxRunsPerDay: number };
  /** Set on the team an orchestration runs; empty for the user's own. */
  orchestrationId: string;
  createdAt: number;
  updatedAt: number;
}

export interface ProjectStats {
  runsToday: number;
  costToday: number;
  costWeek: number;
  maxRunsPerDay: number;
}

export interface ProjectVersion {
  /** Unix ms; also the version's name. */
  at: number;
  agents: string[];
}

/** Mirrors src-tauri/src/agents/project_ai.rs. */
export interface GeneratedProject {
  project: Project;
  agents: AgentSpec[];
  integrations: { agentId: string; query: string; why: string }[];
  explanation: string;
}

export type SuggestionField = "task" | "instructions" | "autonomy" | "timeoutMin";

export interface Suggestion {
  agentId: string;
  field: SuggestionField;
  value: string;
  reason: string;
}

export interface Improvement {
  summary: string;
  suggestions: Suggestion[];
}

export function listProjects(): Promise<Project[]> {
  return invoke("projects_list");
}

/** Returns the full list after saving. */
export function saveProject(project: Project): Promise<Project[]> {
  return invoke("project_save", { project });
}

/** Deletes the project, and its agents when asked (else they're left loose). */
export function deleteProject(id: string, deleteAgents: boolean): Promise<[Project[], AgentSpec[]]> {
  return invoke("project_delete", { id, deleteAgents });
}

/** Moves an agent into a project, or out of any with an empty id. */
export function moveAgent(agentId: string, projectId: string): Promise<AgentSpec[]> {
  return invoke("agent_move", { agentId, projectId });
}

export function setProjectEnabled(id: string, enabled: boolean): Promise<AgentSpec[]> {
  return invoke("project_set_enabled", { id, enabled });
}

/** Starts the project's entry agent; a rehearsal only looks, all the way down. */
export function runProject(id: string, rehearsal: boolean): Promise<RunRecord> {
  return invoke("project_run", { id, rehearsal });
}

export function stopProject(id: string): Promise<void> {
  return invoke("project_stop", { id });
}

export function projectStats(id: string): Promise<ProjectStats> {
  return invoke("project_stats", { id });
}

/** The latest runs of the project's agents, newest first. */
export function projectRuns(id: string, limit: number): Promise<RunRecord[]> {
  return invoke("project_runs", { id, limit });
}

export function exportProject(id: string, path: string): Promise<void> {
  return invoke("project_export", { id, path });
}

export function importProject(path: string): Promise<[Project[], AgentSpec[], ImportReport]> {
  return invoke("project_import", { path });
}

/** A team drafted with AI, not saved yet. Takes a while. */
export function draftProject(description: string, workDir: string): Promise<GeneratedProject> {
  return invoke("project_draft", { description, workDir });
}

/** Saves a reviewed project with its agents. */
export function createProject(project: Project, agents: AgentSpec[]): Promise<[Project[], AgentSpec[]]> {
  return invoke("project_create", { project, agents });
}

/** Changes suggested from how the project's runs went. Takes a while. */
export function improveProject(id: string): Promise<Improvement> {
  return invoke("project_improve", { id });
}

export function projectVersions(id: string): Promise<ProjectVersion[]> {
  return invoke("project_versions", { id });
}

export function restoreProject(id: string, at: number): Promise<[Project[], AgentSpec[]]> {
  return invoke("project_restore", { id, at });
}

/** What turning "agents in every console" on or off did for one CLI. */
export interface ConsoleRegistration {
  /** Every CLI Termo knows, not only the ones agents run on. */
  engine: import("$lib/ai-mode/pattern-table").ToolId;
  /** Not installed: nothing to do. */
  skipped: boolean;
  ok: boolean;
  message: string;
}

/** Adds the agents to (or takes them out of) every installed CLI's own
 * settings, so consoles opened anywhere reach them. */
export function setAgentsInEveryConsole(enable: boolean): Promise<ConsoleRegistration[]> {
  return invoke("agents_console_global", { enable });
}

/** A new token for consoles outside Termo; the old one stops working. */
export function rotateConsoleToken(): Promise<ConsoleRegistration[]> {
  return invoke("agents_console_rotate");
}

export type RunEventKind =
  | "started"
  | "session"
  | "usage"
  | "failure"
  | "tool"
  | "text"
  | "result"
  | "stderr"
  | "question"
  | "answer"
  | "ended";

/** Who answered with an error (see src-tauri/src/agents/diagnose.rs). */
export type Provider = "anthropic" | "openai" | "google" | "github" | "xai" | "local";

export type FailureKind =
  | "auth"
  | "modelAccess"
  | "rateLimit"
  | "quota"
  | "overloaded"
  | "server"
  | "badRequest"
  | "network"
  | "timeout"
  | "unknown";

/** Why a run failed, said precisely. */
export interface RunFailure {
  provider: Provider;
  engine: AgentEngine;
  kind: FailureKind;
  /** The HTTP status the service answered with, when it said. */
  status: number | null;
  message: string;
  requestId: string;
  model: string;
  exitCode: number | null;
  /** The last lines the CLI printed on stderr. */
  stderr: string[];
}

/** What ran a run: Termo's side at the start, the CLI's report after. */
export interface SessionInfo {
  engine: AgentEngine;
  version: string;
  model: string;
  /** "subscription", "apiKey" or "". */
  auth: string;
  authSource: string;
  cwd: string;
  command: string;
  mcp: { name: string; status: string }[];
}

/** A usage-limit warning from the CLI. */
export interface UsageInfo {
  window: string;
  status: string;
  /** 0 to 1. */
  utilization: number;
  /** Unix ms; 0: not said. */
  resetsAt: number;
}

export interface RunEvent {
  kind: RunEventKind;
  tool?: string;
  text: string;
  at: number;
  session?: SessionInfo;
  usage?: UsageInfo;
  failure?: RunFailure;
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

/** Mirrors src-tauri/src/agents/github.rs. */
export interface GithubPreview {
  workflowFile: string;
  yaml: string;
  repo: string | null;
  secrets: string[];
  approximated: boolean;
  dropsWatch: boolean;
}

export function githubPreview(spec: AgentSpec): Promise<GithubPreview> {
  return invoke("agent_github_preview", { spec });
}

/** Writes the workflow into the agent's repository; resolves to its path. */
export function githubWrite(spec: AgentSpec): Promise<string> {
  return invoke("agent_github_write", { spec });
}

export function forgetAgentSecrets(agentId: string, names: string[]): Promise<void> {
  return names.length > 0 ? invoke("agent_forget_secrets", { agentId, names }) : Promise.resolve();
}

export interface CustomMcpRequest {
  agentId: string;
  name: string;
  kind: "stdio" | "http" | "sse";
  command: string;
  args: string[];
  url: string;
  values: { name: string; value: string; secret: boolean }[];
}

/** A connector of the user's own, ready to add; its secrets go to the
 * system keychain on the way. */
export function agentCustomMcp(request: CustomMcpRequest): Promise<{ server: McpServer; secrets: string[] }> {
  return invoke("agent_custom_mcp", { request });
}

/** Last month's downloads of an MCP server's package, or null when its
 * registry doesn't say. */
export function mcpDownloads(registryType: string, identifier: string): Promise<number | null> {
  return invoke("mcp_downloads", { registryType, identifier });
}

/** What an import brought in, and what it couldn't. */
export interface ImportReport {
  imported: string[];
  movedToOwnFolder: string[];
  needsSecrets: string[];
  /** Every imported agent arrives paused; these deserve a close look first. */
  review: { agent: string; runsCommands: boolean; escalates: boolean; commands: string[]; urls: string[] }[];
}

export function exportAgents(ids: string[], path: string): Promise<void> {
  return invoke("agents_export", { ids, path });
}

/** Saves the files' agents as new ones; resolves to the full list and a report. */
export function importAgents(paths: string[]): Promise<[AgentSpec[], ImportReport]> {
  return invoke("agents_import", { paths });
}

/** Which of these secrets have no value on this machine. */
export function missingSecrets(agentId: string, names: string[]): Promise<string[]> {
  return names.length > 0 ? invoke("agent_missing_secrets", { agentId, names }) : Promise.resolve([]);
}

export function setAgentSecret(agentId: string, name: string, value: string): Promise<void> {
  return invoke("agent_set_secret", { agentId, name, value });
}

/** The local webhook's port, and whether Termo could take it. */
export function agentsWebhook(): Promise<{ port: number; listening: boolean }> {
  return invoke("agents_webhook");
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

/** Something a run asked the user and is waiting on (mirrors
 * src-tauri/src/agents/run_questions.rs). */
export interface RunQuestion {
  id: string;
  agentId: string;
  runId: string;
  question: string;
  /** Answers it suggests; the user may write another. */
  options: string[];
  state: "pending" | "answered" | "expired";
  answer: string;
  askedAt: number;
  /** When it stops waiting and the agent decides alone. */
  expiresAt: number;
}

/** The questions waiting on the user now. */
export function agentQuestions(): Promise<RunQuestion[]> {
  return invoke("agent_questions");
}

/** False when the question wasn't waiting any more. */
export function answerAgentQuestion(questionId: string, answer: string): Promise<boolean> {
  return invoke("agent_answer_question", { questionId, answer });
}

/** A question was asked, answered or gave up waiting. */
export function onAgentQuestion(handler: (question: RunQuestion) => void): Promise<UnlistenFn> {
  return listen<RunQuestion>("agent-question", (event) => handler(event.payload));
}
