import type { MessageKey } from "$lib/i18n/translate";
import type {
  CommandDecision,
  ExternalEntry,
  HarnessAgent,
  HarnessFileChange,
  HarnessSpec,
  PathAccess,
  SuggestedRule,
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

/** What identifies the rule an entry reads as: entries for the same rule
 * (Bash and PowerShell, with and without arguments) go together. */
export function ruleKey(rule: SuggestedRule): string {
  return rule.kind === "path"
    ? `path:${normalizePattern(rule.pattern)}:${rule.access}`
    : `command:${normalizePrefix(rule.prefix)}:${rule.decision}`;
}

/** Puts a rule in the harness: a path or a command it has already takes
 * the rule's level instead (the stricter, for a path). */
export function addRule(spec: HarnessSpec, rule: SuggestedRule): void {
  if (rule.kind === "path") {
    const pattern = normalizePattern(rule.pattern);
    const known = spec.paths.find((other) => normalizePattern(other.pattern) === pattern);
    if (!known) spec.paths.push({ pattern, access: rule.access });
    else if (rule.access === "noRead") known.access = "noRead";
  } else {
    const prefix = normalizePrefix(rule.prefix);
    const known = spec.commands.find((other) => normalizePrefix(other.prefix) === prefix);
    if (!known) spec.commands.push({ prefix, decision: rule.decision });
    else known.decision = rule.decision;
  }
}

export interface ExternalGroup {
  key: string;
  rule: SuggestedRule | null;
  entries: ExternalEntry[];
}

/** The external entries, one group per rule; those Termo can't read as a
 * rule, one group each. */
export function groupExternal(entries: ExternalEntry[]): ExternalGroup[] {
  const groups: ExternalGroup[] = [];
  for (const entry of entries) {
    const key = entry.rule ? ruleKey(entry.rule) : `raw:${entry.file}:${entry.key}:${entry.value}`;
    const known = groups.find((group) => group.key === key);
    if (known) known.entries.push(entry);
    else groups.push({ key, rule: entry.rule ?? null, entries: [entry] });
  }
  return groups;
}

export function sameEntry(a: ExternalEntry, b: ExternalEntry): boolean {
  return a.file === b.file && a.key === b.key && a.value === b.value;
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

/** A kind of rule, as far as which agents can hold it goes. */
export type RuleKind = "noEdit" | "noRead" | "deny" | "ask" | "allow" | "checks";
/** enforced: the agent itself refuses or stops; partial: not every case
 * (Grok's gate, unconfirmed); text: only told in AGENTS.md; none: no effect. */
export type Support = "enforced" | "partial" | "text" | "none";

export const RULE_KINDS: RuleKind[] = ["noEdit", "noRead", "deny", "ask", "allow", "checks"];

/** What each agent's own project files can hold (see src-tauri/src/harness:
 * one module per agent). */
export const SUPPORT: Record<HarnessAgent, Record<RuleKind, Support>> = {
  claude: { noEdit: "enforced", noRead: "enforced", deny: "enforced", ask: "enforced", allow: "enforced", checks: "enforced" },
  codex: { noEdit: "text", noRead: "text", deny: "enforced", ask: "enforced", allow: "enforced", checks: "enforced" },
  gemini: { noEdit: "text", noRead: "text", deny: "enforced", ask: "text", allow: "enforced", checks: "enforced" },
  copilot: { noEdit: "text", noRead: "text", deny: "text", ask: "text", allow: "none", checks: "enforced" },
  // Through Claude Code's settings, which it reads too.
  grok: { noEdit: "enforced", noRead: "enforced", deny: "enforced", ask: "enforced", allow: "enforced", checks: "partial" },
};

/** The kinds of rule the harness has, in a fixed order. */
export function ruleKinds(spec: HarnessSpec): RuleKind[] {
  const present = new Set<RuleKind>();
  for (const rule of spec.paths) present.add(rule.access);
  for (const rule of spec.commands) present.add(rule.decision);
  if (spec.checks.some((check) => check.command.trim())) present.add("checks");
  return RULE_KINDS.filter((kind) => present.has(kind));
}

/** The agents that only read their repository files once it's trusted. */
export const NEEDS_TRUST: HarnessAgent[] = ["codex", "gemini", "copilot", "grok"];

export type HarnessStatus = "none" | "draft" | "pending" | "synced";

/** What the header says: edits not applied yet win over files out of step. */
export function harnessStatus(exists: boolean, dirty: boolean, pending: number): HarnessStatus {
  if (dirty) return "draft";
  if (pending > 0) return "pending";
  return exists ? "synced" : "none";
}
