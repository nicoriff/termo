import type { MessageKey } from "$lib/i18n/translate";
import type {
  CommandDecision,
  CommandRule,
  ExternalEntry,
  HarnessAgent,
  HarnessFileChange,
  HarnessSpec,
  PathAccess,
  ProposedRule,
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

/** Puts a rule an agent proposed in the harness. */
export function addProposed(spec: HarnessSpec, rule: ProposedRule): void {
  if (rule.kind !== "check") {
    addRule(spec, rule);
    return;
  }
  const command = rule.command.trim();
  if (command && !spec.checks.some((check) => check.command.trim() === command)) spec.checks.push({ command });
}

/** Whether the harness has that rule already, as proposed. */
export function hasProposed(spec: HarnessSpec, rule: ProposedRule): boolean {
  switch (rule.kind) {
    case "path": {
      const pattern = normalizePattern(rule.pattern);
      return spec.paths.some((other) => normalizePattern(other.pattern) === pattern && other.access === rule.access);
    }
    case "command": {
      const prefix = normalizePrefix(rule.prefix);
      return spec.commands.some((other) => normalizePrefix(other.prefix) === prefix && other.decision === rule.decision);
    }
    case "check":
      return spec.checks.some((check) => check.command.trim() === rule.command.trim());
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

/** Ready-made sets of command rules, added in one go. */
export interface CommandPack {
  id: "gitDestructive" | "publish" | "install" | "recursiveDelete";
  label: MessageKey;
  rules: { prefix: string; decision: CommandDecision }[];
}

export const COMMAND_PACKS: CommandPack[] = [
  {
    id: "gitDestructive",
    label: "harness.packs.gitDestructive",
    rules: [
      { prefix: "git push --force", decision: "deny" },
      { prefix: "git push -f", decision: "deny" },
      { prefix: "git reset --hard", decision: "deny" },
      { prefix: "git clean", decision: "deny" },
      { prefix: "git branch -D", decision: "ask" },
      { prefix: "git push", decision: "ask" },
    ],
  },
  {
    id: "publish",
    label: "harness.packs.publish",
    rules: [
      { prefix: "npm publish", decision: "deny" },
      { prefix: "pnpm publish", decision: "deny" },
      { prefix: "yarn publish", decision: "deny" },
      { prefix: "cargo publish", decision: "deny" },
      { prefix: "twine upload", decision: "deny" },
      { prefix: "gh release create", decision: "deny" },
    ],
  },
  {
    id: "install",
    label: "harness.packs.install",
    rules: [
      { prefix: "npm install", decision: "ask" },
      { prefix: "pnpm add", decision: "ask" },
      { prefix: "yarn add", decision: "ask" },
      { prefix: "pip install", decision: "ask" },
      { prefix: "cargo add", decision: "ask" },
    ],
  },
  {
    id: "recursiveDelete",
    label: "harness.packs.recursiveDelete",
    rules: [
      { prefix: "rm -rf", decision: "deny" },
      { prefix: "rm -r", decision: "deny" },
      { prefix: "Remove-Item -Recurse", decision: "deny" },
    ],
  },
];

/** Adds a pack's rules; a command the harness already has keeps its own. */
export function addPack(spec: HarnessSpec, pack: CommandPack): void {
  const known = new Set(spec.commands.map((rule) => normalizePrefix(rule.prefix)));
  for (const rule of pack.rules) {
    if (!known.has(rule.prefix)) spec.commands.push({ ...rule });
  }
}

/** Whether every rule of the pack is in the harness already. */
export function hasPack(spec: HarnessSpec, pack: CommandPack): boolean {
  const known = new Set(spec.commands.map((rule) => normalizePrefix(rule.prefix)));
  return pack.rules.every((rule) => known.has(rule.prefix));
}

/** What an agent does with a command: one of the decisions when it holds
 * the rule itself, "text" when it's only told, "none" when no rule covers it
 * (its usual asking). */
export type Verdict = CommandDecision | "text" | "none";

const STRICTNESS: Record<CommandDecision, number> = { allow: 0, ask: 1, deny: 2 };

/** The rule that decides a command: of those whose words start it, the
 * strictest, as Claude Code and Codex weigh them. */
export function matchingRule(spec: HarnessSpec, command: string): CommandRule | null {
  const words = normalizePrefix(command).split(" ");
  let best: CommandRule | null = null;
  for (const rule of spec.commands) {
    const prefix = normalizePrefix(rule.prefix).split(" ");
    if (!prefix[0] || prefix.length > words.length || prefix.some((word, index) => word !== words[index])) continue;
    if (!best || STRICTNESS[rule.decision] > STRICTNESS[best.decision]) best = rule;
  }
  return best;
}

/** What each agent the harness is for does with `command`. */
export function testCommand(spec: HarnessSpec, command: string): { agent: HarnessAgent; verdict: Verdict }[] {
  const rule = matchingRule(spec, command);
  return AGENTS.filter((agent) => spec.agents.includes(agent)).map((agent) => {
    if (!rule) return { agent, verdict: "none" };
    const support = SUPPORT[agent][rule.decision];
    const verdict: Verdict = support === "enforced" || support === "partial" ? rule.decision : support === "text" ? "text" : "none";
    return { agent, verdict };
  });
}

/** A row of the panel's two lists: what the agents do, and what they don't. */
export type RowKind = "instruction" | "check" | "allow" | "noEdit" | "noRead" | "deny" | "ask";
export type RowSide = "do" | "dont";

export const ROW_KINDS: Record<RowSide, RowKind[]> = {
  do: ["check", "allow", "instruction"],
  dont: ["noRead", "noEdit", "deny", "ask"],
};

export interface RuleRow {
  kind: RowKind;
  /** The path, the command, or the instruction's text. */
  subject: string;
  /** Where it is in its own list (paths, commands, checks, instruction lines). */
  index: number;
}

/** The instructions as one item per line, list markers left out. */
export function instructionItems(text: string): string[] {
  return text
    .split("\n")
    .map((line) => line.trim().replace(/^[-*+]\s+/, ""))
    .filter(Boolean);
}

export function joinInstructionItems(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}

/** The harness as the two lists, in a fixed order of kinds. */
export function ruleRows(spec: HarnessSpec): Record<RowSide, RuleRow[]> {
  const rows: RuleRow[] = [];
  spec.checks.forEach((check, index) => rows.push({ kind: "check", subject: check.command, index }));
  spec.commands.forEach((rule, index) => rows.push({ kind: rule.decision, subject: rule.prefix, index }));
  instructionItems(spec.instructions).forEach((item, index) => rows.push({ kind: "instruction", subject: item, index }));
  spec.paths.forEach((rule, index) => rows.push({ kind: rule.access, subject: rule.pattern, index }));
  const side = (kinds: RowKind[]) =>
    rows.filter((row) => kinds.includes(row.kind)).sort((a, b) => kinds.indexOf(a.kind) - kinds.indexOf(b.kind));
  return { do: side(ROW_KINDS.do), dont: side(ROW_KINDS.dont) };
}

export function isPathKind(kind: RowKind): kind is PathAccess {
  return kind === "noEdit" || kind === "noRead";
}

export function isCommandKind(kind: RowKind): kind is CommandDecision {
  return kind === "deny" || kind === "ask" || kind === "allow";
}

/** Why `subject` can't be added as a row of `kind`; null when it can. */
export function rowProblem(spec: HarnessSpec, kind: RowKind, subject: string): MessageKey | null {
  if (isPathKind(kind)) return pathProblem(subject, spec.paths.map((rule) => rule.pattern));
  if (isCommandKind(kind)) return commandProblem(subject, spec.commands.map((rule) => rule.prefix));
  const text = subject.trim();
  if (!text) return "harness.rows.empty";
  if (kind === "check") {
    return spec.checks.some((check) => check.command.trim() === text) ? "harness.rows.duplicate" : null;
  }
  return instructionItems(spec.instructions).includes(text) ? "harness.rows.duplicate" : null;
}

export function addRow(spec: HarnessSpec, kind: RowKind, subject: string): void {
  if (isPathKind(kind)) spec.paths.push({ pattern: normalizePattern(subject), access: kind });
  else if (isCommandKind(kind)) spec.commands.push({ prefix: normalizePrefix(subject), decision: kind });
  else if (kind === "check") spec.checks.push({ command: subject.trim() });
  else spec.instructions = joinInstructionItems([...instructionItems(spec.instructions), subject.trim()]);
}

export function removeRow(spec: HarnessSpec, row: RuleRow): void {
  if (isPathKind(row.kind)) spec.paths.splice(row.index, 1);
  else if (isCommandKind(row.kind)) spec.commands.splice(row.index, 1);
  else if (row.kind === "check") spec.checks.splice(row.index, 1);
  else {
    const items = instructionItems(spec.instructions);
    items.splice(row.index, 1);
    spec.instructions = joinInstructionItems(items);
  }
}

/** What each agent the harness is for does with a row: the support of its
 * kind (see SUPPORT); an instruction is only ever read. */
export function rowSupport(spec: HarnessSpec, kind: RowKind): { agent: HarnessAgent; support: Support }[] {
  return AGENTS.filter((agent) => spec.agents.includes(agent)).map((agent) => ({
    agent,
    support: kind === "instruction" ? "text" : SUPPORT[agent][kind === "check" ? "checks" : kind],
  }));
}

export type HarnessStatus = "none" | "draft" | "pending" | "synced";

/** What the header says: edits not applied yet win over files out of step. */
export function harnessStatus(exists: boolean, dirty: boolean, pending: number): HarnessStatus {
  if (dirty) return "draft";
  if (pending > 0) return "pending";
  return exists ? "synced" : "none";
}
