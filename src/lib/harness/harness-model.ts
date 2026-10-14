import type { MessageKey } from "$lib/i18n/translate";
import type {
  CommandDecision,
  HarnessAgent,
  HarnessFileChange,
  HarnessSpec,
  PathAccess,
} from "$lib/core/harness-client";

/** In the order the panel lists them. */
export const AGENTS: HarnessAgent[] = ["claude", "codex", "gemini", "copilot", "grok"];
export const ACCESS_LEVELS: PathAccess[] = ["noEdit", "noRead"];
export const DECISIONS: CommandDecision[] = ["deny", "ask", "allow"];

export function copySpec(spec: HarnessSpec): HarnessSpec {
  return JSON.parse(JSON.stringify(spec)) as HarnessSpec;
}

/** Forward slashes, no leading `./`, no trailing slash: as the backend
 * writes it (see model.rs, normalize_pattern). */
export function normalizePattern(pattern: string): string {
  return pattern
    .trim()
    .replaceAll("\\", "/")
    .replace(/^(\.\/)+/, "")
    .replace(/\/+$/, "");
}

export function normalizePrefix(prefix: string): string {
  return prefix.trim().split(/\s+/).filter(Boolean).join(" ");
}

/** The rules as they'd be written: what tells an edit that changes
 * something from one that doesn't. Termo's record is left out. */
function comparable(spec: HarnessSpec): string {
  return JSON.stringify({
    agents: [...new Set(spec.agents)].sort(),
    paths: spec.paths.map((rule) => [normalizePattern(rule.pattern), rule.access]).filter(([pattern]) => pattern),
    commands: spec.commands.map((rule) => [normalizePrefix(rule.prefix), rule.decision]).filter(([prefix]) => prefix),
    checks: spec.checks.map((check) => [check.command.trim(), check.timeoutSec ?? null]).filter(([command]) => command),
    instructions: spec.instructions.trim(),
  });
}

export function sameSpec(a: HarnessSpec, b: HarnessSpec): boolean {
  return comparable(a) === comparable(b);
}

/** Why a path can't be a rule, or null when it can: it has to be inside the
 * repository, and not one of the rules already there (`known`). */
export function pathProblem(pattern: string, known: string[]): MessageKey | null {
  const normalized = normalizePattern(pattern);
  if (!normalized) return "harness.paths.empty";
  if (/^[a-zA-Z]:/.test(normalized) || normalized.startsWith("//") || normalized.split("/").includes("..")) {
    return "harness.paths.outside";
  }
  if (/[()]/.test(normalized)) return "harness.paths.parentheses";
  if (known.map(normalizePattern).includes(normalized)) return "harness.paths.duplicate";
  return null;
}

export function commandProblem(prefix: string, known: string[]): MessageKey | null {
  const normalized = normalizePrefix(prefix);
  if (!normalized) return "harness.commands.empty";
  if (/[()]/.test(normalized)) return "harness.commands.parentheses";
  if (known.map(normalizePrefix).includes(normalized)) return "harness.commands.duplicate";
  return null;
}

/** A file picked anywhere on disk, as a pattern relative to the root; null
 * when it's outside the repository. */
export function relativeToRoot(root: string, file: string): string | null {
  const norm = (path: string) => path.replaceAll("\\", "/").replace(/\/+$/, "");
  const base = norm(root);
  const target = norm(file);
  // Windows paths compare without regard to case.
  const fold = /^[a-zA-Z]:/.test(base) ? (path: string) => path.toLowerCase() : (path: string) => path;
  if (!fold(target).startsWith(`${fold(base)}/`)) return null;
  return target.slice(base.length + 1);
}

export type ChangeKind = "create" | "update" | "delete";

export function changeKind(change: HarnessFileChange): ChangeKind {
  if (change.before === null) return "create";
  if (change.after === null) return "delete";
  return "update";
}

export const CHANGE_KIND_LABEL: Record<ChangeKind, MessageKey> = {
  create: "harness.review.create",
  update: "harness.review.update",
  delete: "harness.review.delete",
};

export type HarnessStatus = "none" | "draft" | "pending" | "synced";

/** What the header says: edits not applied yet win over files out of step. */
export function harnessStatus(exists: boolean, dirty: boolean, pending: number): HarnessStatus {
  if (dirty) return "draft";
  if (pending > 0) return "pending";
  return exists ? "synced" : "none";
}
