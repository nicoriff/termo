import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentAutonomy, AgentEngine, AskLevel } from "./agents-client";

/** Mirrors src-tauri/src/agents/kanban/model.rs. */
export type KanbanColumn = "notReady" | "todo" | "inProgress" | "review" | "done";
export type KanbanMode = "sequential" | "worktrees";
export type KanbanRole = "work" | "review" | "summary";
export type KanbanSuccess = "full" | "partial" | "failed";
export type KanbanVerdict = "approve" | "changes";

export const KANBAN_COLUMNS: KanbanColumn[] = ["notReady", "todo", "inProgress", "review", "done"];

export interface KanbanColumnAgent {
  engine: AgentEngine;
  /** Empty: the CLI's default. */
  model: string;
  autonomy: AgentAutonomy;
  web: boolean;
  context: string;
  harness: string;
  /** The saved agents that run it, one per task it can work on at once. */
  agentIds: string[];
}

export interface KanbanAgents {
  work: KanbanColumnAgent;
  review: KanbanColumnAgent;
  summary: KanbanColumnAgent | null;
}

export type KanbanActor =
  | { kind: "user" }
  | { kind: "console" }
  | { kind: "termo" }
  | { kind: "agent"; role: KanbanRole; engine: AgentEngine; model: string };

export type KanbanLogKind =
  | "note"
  | "created"
  | "edited"
  | "moved"
  | "taken"
  | "workDone"
  | "workFailed"
  | "approved"
  | "changesRequested"
  | "tooManyRounds"
  | "noVerdict"
  | "summarized"
  | "cancelled"
  | "interrupted"
  | "branch"
  | "merged"
  | "mergeFailed"
  | "resolved";

export interface KanbanLogEntry {
  at: number;
  actor: KanbanActor;
  kind: KanbanLogKind;
  text: string;
  from?: KanbanColumn;
  to?: KanbanColumn;
}

export interface KanbanOutcome {
  summary: string;
  success: KanbanSuccess;
  notes: string;
  at: number;
}

export interface KanbanTask {
  id: string;
  number: number;
  title: string;
  spec: string;
  successCriteria: string;
  dependsOn: string[];
  column: KanbanColumn;
  role: KanbanRole | null;
  runId: string;
  agentId: string;
  summaryPending: boolean;
  reviewRounds: number;
  reviewNotes: string;
  verdict: KanbanVerdict | null;
  report: KanbanOutcome | null;
  workSummary: string;
  needsYou: string;
  outcome: KanbanOutcome | null;
  costUsd: number | null;
  branch: string;
  worktree: string;
  merged: boolean;
  log: KanbanLogEntry[];
  createdAt: number;
  updatedAt: number;
}

export interface KanbanBoard {
  id: string;
  name: string;
  icon: string;
  workDir: string;
  enabled: boolean;
  mode: KanbanMode;
  maxParallel: number;
  maxReviewRounds: number;
  baseBranch: string;
  askLevel: AskLevel;
  agents: KanbanAgents;
  tasks: KanbanTask[];
  projectId: string;
  nextNumber: number;
  createdAt: number;
  updatedAt: number;
}

/** What the board dialog saves: everything but its tasks. */
export interface KanbanSettings {
  id: string;
  name: string;
  icon: string;
  workDir: string;
  mode: KanbanMode;
  maxParallel: number;
  maxReviewRounds: number;
  baseBranch: string;
  askLevel: AskLevel;
  agents: KanbanAgents;
}

/** A task as the editor writes it. */
export interface KanbanTaskInput {
  /** Empty: a new task. */
  id: string;
  title: string;
  spec: string;
  successCriteria: string;
  dependsOn: string[];
  column?: KanbanColumn;
}

export function listKanbanBoards(): Promise<KanbanBoard[]> {
  return invoke("kanban_list");
}

/** The branches a board's worktrees can start from: the folder's current one first. */
export function kanbanBranches(folder: string): Promise<string[]> {
  return invoke("kanban_branches", { folder });
}

export function saveKanbanBoard(board: KanbanSettings): Promise<KanbanBoard> {
  return invoke("kanban_save", { board });
}

export function setKanbanEnabled(id: string, enabled: boolean): Promise<KanbanBoard> {
  return invoke("kanban_set_enabled", { id, enabled });
}

export function deleteKanbanBoard(id: string): Promise<KanbanBoard[]> {
  return invoke("kanban_delete", { id });
}

export function upsertKanbanTask(boardId: string, task: KanbanTaskInput): Promise<KanbanBoard> {
  return invoke("kanban_task_upsert", { boardId, task });
}

/** Moves a task before `before` (another task's id) or to the end of the
 * column. One an agent is on moves only with `cancel`. */
export function moveKanbanTask(
  boardId: string,
  taskId: string,
  column: KanbanColumn,
  before: string | null,
  cancel: boolean,
): Promise<KanbanBoard> {
  return invoke("kanban_task_move", { boardId, taskId, column, before, cancel });
}

export function cancelKanbanTask(boardId: string, taskId: string): Promise<KanbanBoard> {
  return invoke("kanban_task_cancel", { boardId, taskId });
}

export function deleteKanbanTask(boardId: string, taskId: string): Promise<KanbanBoard> {
  return invoke("kanban_task_delete", { boardId, taskId });
}

export function resolveKanbanTask(boardId: string, taskId: string): Promise<KanbanBoard> {
  return invoke("kanban_task_resolve", { boardId, taskId });
}

export function mergeKanbanTask(boardId: string, taskId: string): Promise<KanbanBoard> {
  return invoke("kanban_task_merge", { boardId, taskId });
}

export function noteKanbanTask(boardId: string, taskId: string, text: string): Promise<KanbanBoard> {
  return invoke("kanban_task_note", { boardId, taskId, text });
}

/** Every change to a board, as saved. */
export function onKanban(handler: (board: KanbanBoard) => void): Promise<UnlistenFn> {
  return listen<KanbanBoard>("kanban", (event) => handler(event.payload));
}
