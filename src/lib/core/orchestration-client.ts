import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentAutonomy, AgentEngine } from "./agents-client";

/** Mirrors src-tauri/src/agents/orchestration/plan.rs. */
export type TaskStatus = "pending" | "running" | "done" | "failed" | "dropped";
export type OrchestrationStatus = "draft" | "running" | "awaitingApproval" | "paused" | "done" | "failed" | "stopped";

export interface PlannedAgent {
  /** How tasks refer to it. */
  key: string;
  name: string;
  icon: string;
  specialty: string;
  engine: AgentEngine;
  /** Empty: the engine's default. */
  model: string;
  whyModel: string;
  autonomy: AgentAutonomy;
  web: boolean;
  instructions: string;
  /** The saved agent that runs its tasks, once the plan is approved. */
  agentId: string;
}

export interface PlannedTask {
  key: string;
  title: string;
  description: string;
  /** The key of the agent that does it. */
  agent: string;
  dependsOn: string[];
  doneWhen: string;
  /** The planner's guess, all of the task's turns together. */
  tokensIn: number;
  tokensOut: number;
  status: TaskStatus;
  runId: string;
  summary: string;
  /** What its runs cost, when the CLI says (only Claude Code does). */
  costUsd: number | null;
  attempts: number;
}

export interface Plan {
  agents: PlannedAgent[];
  tasks: PlannedTask[];
}

export interface Strategy {
  key: string;
  name: string;
  summary: string;
  plan: Plan;
}

export type PlanChange =
  | { kind: "addAgent"; agent: PlannedAgent }
  | { kind: "addTask"; task: PlannedTask }
  | { kind: "reassign"; task: string; agent: string }
  | { kind: "changeModel"; agent: string; engine: AgentEngine; model: string }
  | { kind: "dropTask"; task: string }
  | { kind: "retryTask"; task: string };

export type RevisionState = "applied" | "pending" | "rejected";
/** Why a revision waits for the user. */
export type RevisionHold = "none" | "budget" | "autonomy" | "limit";

export interface Revision {
  id: string;
  /** Unix milliseconds. */
  at: number;
  note: string;
  changes: PlanChange[];
  state: RevisionState;
  hold: RevisionHold;
  /** What the whole plan is expected to cost with it. */
  estimateUsd: number;
}

export interface Orchestration {
  id: string;
  name: string;
  icon: string;
  prompt: string;
  workDir: string;
  status: OrchestrationStatus;
  orchestratorModel: string;
  strategies: Strategy[];
  recommended: string;
  why: string;
  chosen: string;
  /** 0: no limit. */
  budgetUsd: number;
  maxAutonomy: AgentAutonomy;
  /** The plan as it stands: the approved one, with every change since. */
  plan: Plan;
  revisions: Revision[];
  projectId: string;
  /** What planning and supervising cost. */
  overheadUsd: number;
  note: string;
  createdAt: number;
  updatedAt: number;
}

/** USD per million tokens. */
export interface Price {
  input: number;
  output: number;
}

export interface CatalogModel {
  id: string;
  name: string;
  description: string;
  price: Price | null;
}

export interface CatalogEngine {
  engine: AgentEngine;
  installed: boolean;
  models: CatalogModel[];
}

export type Catalog = CatalogEngine[];

export function orchestrationCatalog(): Promise<Catalog> {
  return invoke("orchestration_catalog");
}

export function listOrchestrations(): Promise<Orchestration[]> {
  return invoke("orchestration_list");
}

/** Plans the work with AI: a draft to review, nothing runs yet. */
export function planOrchestration(prompt: string, workDir: string, model: string): Promise<Orchestration> {
  return invoke("orchestration_plan", { prompt, workDir, model });
}

export function approveOrchestration(id: string, chosen: string, plan: Plan, budgetUsd: number): Promise<Orchestration> {
  return invoke("orchestration_approve", { id, chosen, plan, budgetUsd });
}

/** Saves changes to the plan of one that isn't running, keeping what has run. */
export function editOrchestrationPlan(id: string, plan: Plan): Promise<Orchestration> {
  return invoke("orchestration_edit_plan", { id, plan });
}

export interface RevisedPlan {
  plan: Plan;
  /** The orchestrator's note on what it changed. */
  note: string;
  /** Saved with what asking cost added to its overhead. */
  orchestration: Orchestration;
}

/** Asks the orchestrator to work these changes into the plan under review.
 * Nothing is saved but the cost: the plan comes back to review. */
export function reviseOrchestration(id: string, plan: Plan, instructions: string): Promise<RevisedPlan> {
  return invoke("orchestration_revise", { id, plan, instructions });
}

export function stopOrchestration(id: string): Promise<void> {
  return invoke("orchestration_stop", { id });
}

export function resumeOrchestration(id: string, budgetUsd: number | null): Promise<Orchestration> {
  return invoke("orchestration_resume", { id, budgetUsd });
}

export function resolveRevision(id: string, revisionId: string, approve: boolean): Promise<Orchestration> {
  return invoke("orchestration_resolve_revision", { id, revisionId, approve });
}

export function deleteOrchestration(id: string): Promise<Orchestration[]> {
  return invoke("orchestration_delete", { id });
}

/** Every change to an orchestration, as saved. */
export function onOrchestration(handler: (orchestration: Orchestration) => void): Promise<UnlistenFn> {
  return listen<Orchestration>("orchestration", (event) => handler(event.payload));
}
