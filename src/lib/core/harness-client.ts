import { invoke } from "@tauri-apps/api/core";

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
