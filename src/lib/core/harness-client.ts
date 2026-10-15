import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentEngine } from "./agents-client";
import type { PlanStep } from "./orchestration-client";

/** See src-tauri/src/harness and src-tauri/src/commands/harness_commands.rs. */
export type HarnessAgent = "claude" | "codex" | "gemini" | "copilot" | "grok";
export type PathAccess = "noEdit" | "noRead";
export type CommandDecision = "allow" | "ask" | "deny";

export interface PathRule {
  /** Relative to the repository: `src/generated/**`, or a bare name such as
   * `.env` for that name in any folder. */
  pattern: string;
  access: PathAccess;
}

export interface CommandRule {
  /** The command's first words: `git push` covers `git push origin main`. */
  prefix: string;
  decision: CommandDecision;
}

export interface Check {
  command: string;
  timeoutSec?: number;
}

export interface HarnessSpec {
  version: number;
  agents: HarnessAgent[];
  paths: PathRule[];
  commands: CommandRule[];
  checks: Check[];
  instructions: string;
  /** The files the harness writes can't be edited by its agents. Absent in
   * a harness from before the option: on. */
  protectOwnFiles?: boolean;
  /** Kept by Termo: what it wrote into each file. Sent back untouched. */
  managed?: Record<string, Record<string, unknown[]>>;
}

export interface HarnessFileChange {
  /** Relative to the repository, with forward slashes. */
  path: string;
  /** null: the file isn't there yet. */
  before: string | null;
  /** null: the file goes. */
  after: string | null;
}

export type SuggestedRule =
  | { kind: "path"; pattern: string; access: PathAccess }
  | { kind: "command"; prefix: string; decision: CommandDecision };

/** A rule in an agent's files that isn't Termo's (see external.rs). */
export interface ExternalEntry {
  file: string;
  /** The list it's in, dotted: `permissions.deny`. */
  key: string;
  value: string;
  /** The harness rule it reads as; absent when it's none Termo writes. */
  rule?: SuggestedRule;
}

export interface HarnessState {
  /** The repository has a `.termo/harness.json`. */
  exists: boolean;
  spec: HarnessSpec;
  /** What the files lack to match the harness; empty when in sync. */
  pending: HarnessFileChange[];
  external: ExternalEntry[];
  /** The instruction files there are, relative to the root. */
  instructionFiles: string[];
}

export function harnessLoad(root: string): Promise<HarnessState> {
  return invoke("harness_load", { root });
}

/** `adopted`: the user's entries whose rules the harness now holds, taken
 * out of their files for Termo's own. */
export function harnessPreview(root: string, spec: HarnessSpec, adopted: ExternalEntry[]): Promise<HarnessFileChange[]> {
  return invoke("harness_preview", { root, spec, adopted });
}

/** The "done" checks the repository's own files suggest (package.json
 * scripts, Cargo, a Makefile…). */
export function harnessDetectChecks(root: string): Promise<string[]> {
  return invoke("harness_detect_checks", { root });
}

export function harnessApply(root: string, spec: HarnessSpec, adopted: ExternalEntry[]): Promise<HarnessState> {
  return invoke("harness_apply", { root, spec, adopted });
}

/** A rule an agent proposes (see src-tauri/src/harness/assist.rs). */
export type ProposedRule =
  | { kind: "path"; pattern: string; access: PathAccess }
  | { kind: "command"; prefix: string; decision: CommandDecision }
  | { kind: "check"; command: string };

export interface Proposal {
  rule: ProposedRule;
  reason: string;
  /** Where it came from, when it was a sentence in an instruction file. */
  source?: { file: string; quote: string };
}

export interface Proposals {
  summary: string;
  proposals: Proposal[];
}

export interface Unified {
  summary: string;
  changes: HarnessFileChange[];
}

export interface HarnessStepEvent {
  root: string;
  step: PlanStep;
}

/** Rules an agent proposes, beyond the ones `spec` has. It costs a model's
 * answer; what the agent does on the way comes through onHarnessStep. */
export function harnessPropose(root: string, engine: AgentEngine, spec: HarnessSpec): Promise<Proposals> {
  return invoke("harness_propose", { root, engine, spec });
}

/** The instruction files unified by an agent, as changes to review. */
export function harnessUnify(root: string, engine: AgentEngine): Promise<Unified> {
  return invoke("harness_unify", { root, engine });
}

export function harnessAssistCancel(root: string): Promise<void> {
  return invoke("harness_assist_cancel", { root });
}

/** Writes reviewed changes to the instruction files; fails when one changed
 * since they were worked out. */
export function harnessWriteInstructions(root: string, changes: HarnessFileChange[]): Promise<HarnessState> {
  return invoke("harness_write_instructions", { root, changes });
}

export function onHarnessStep(handler: (event: HarnessStepEvent) => void): Promise<UnlistenFn> {
  return listen<HarnessStepEvent>("harness-step", (event) => handler(event.payload));
}
