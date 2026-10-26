/**
 * What the status bar can show and where: the catalog of items, the layout
 * the user arranged in Preferences, and the one it has until they do. Pure,
 * so it runs under Node (see layout.test.ts); each item's component lives in
 * ./items and is picked by id in StatusBar.svelte.
 */
import type { MessageKey } from "$lib/i18n/translate";

export type StatusItemId =
  | "ai.mode"
  | "agent.engine"
  | "agent.state"
  | "agent.model"
  | "agent.plan"
  | "agent.context"
  | "agent.cost"
  | "agent.spendLimit"
  | "agent.lines"
  | "agent.duration"
  | "agent.version"
  | "agents.running"
  | "companion.nudge"
  | "git.repo"
  | "git.lastCommit"
  | "git.stash"
  | "git.upstream"
  | "git.lastFetch"
  | "path.cwd"
  | "console.process"
  | "console.size"
  | "console.memory"
  | "console.cpu"
  | "system.appMemory"
  | "system.cpu"
  | "system.memory"
  | "console.prompt"
  | "system.clock"
  | "system.daemon"
  | "system.remote";

export type StatusSide = "left" | "right";

export type StatusCategory = "agent" | "git" | "path" | "console" | "diagnostics" | "system";

export const STATUS_CATEGORIES: StatusCategory[] = ["agent", "git", "path", "console", "diagnostics", "system"];

/** A setting of one item: on or off, or one of a few choices. Kept as text
 * in the layout ("on" / "off" for a toggle), so the backend stores it
 * without knowing what any of them mean. */
export type StatusOptionSpec =
  | { key: string; label: MessageKey; kind: "toggle"; default: boolean }
  | { key: string; label: MessageKey; kind: "choice"; default: string; choices: { value: string; label: MessageKey }[] };

export interface StatusItemSpec {
  id: StatusItemId;
  category: StatusCategory;
  label: MessageKey;
  description: MessageKey;
  options?: StatusOptionSpec[];
}

export interface StatusItemRef {
  id: StatusItemId;
  options?: Record<string, string>;
}

export interface StatusBarLayout {
  left: StatusItemRef[];
  right: StatusItemRef[];
}

const toggle = (key: string, label: MessageKey, value = true): StatusOptionSpec => ({ key, label, kind: "toggle", default: value });

export const STATUS_ITEMS: StatusItemSpec[] = [
  { id: "ai.mode", category: "agent", label: "statusBar.items.aiMode", description: "statusBar.items.aiModeHint" },
  { id: "agent.engine", category: "agent", label: "statusBar.items.engine", description: "statusBar.items.engineHint" },
  { id: "agent.state", category: "agent", label: "statusBar.items.state", description: "statusBar.items.stateHint" },
  { id: "agent.model", category: "agent", label: "statusBar.items.model", description: "statusBar.items.modelHint" },
  {
    id: "agent.plan",
    category: "agent",
    label: "statusBar.items.plan",
    description: "statusBar.items.planHint",
    options: [toggle("resets", "statusBar.options.resets", false)],
  },
  {
    id: "agent.context",
    category: "agent",
    label: "statusBar.items.context",
    description: "statusBar.items.contextHint",
    options: [
      {
        key: "format",
        label: "statusBar.options.format",
        kind: "choice",
        default: "percent",
        choices: [
          { value: "percent", label: "statusBar.options.percent" },
          { value: "tokens", label: "statusBar.options.tokens" },
        ],
      },
    ],
  },
  { id: "agent.cost", category: "agent", label: "statusBar.items.cost", description: "statusBar.items.costHint" },
  { id: "agent.spendLimit", category: "agent", label: "statusBar.items.spendLimit", description: "statusBar.items.spendLimitHint" },
  { id: "agent.lines", category: "agent", label: "statusBar.items.lines", description: "statusBar.items.linesHint" },
  { id: "agent.duration", category: "agent", label: "statusBar.items.duration", description: "statusBar.items.durationHint" },
  { id: "agent.version", category: "agent", label: "statusBar.items.version", description: "statusBar.items.versionHint" },
  { id: "agents.running", category: "agent", label: "statusBar.items.agentsRunning", description: "statusBar.items.agentsRunningHint" },
  { id: "companion.nudge", category: "agent", label: "statusBar.items.nudge", description: "statusBar.items.nudgeHint" },
  {
    id: "git.repo",
    category: "git",
    label: "statusBar.items.gitRepo",
    description: "statusBar.items.gitRepoHint",
    options: [
      toggle("branch", "preferences.git.parts.branch"),
      toggle("sync", "preferences.git.parts.sync"),
      toggle("changes", "preferences.git.parts.changes"),
      toggle("pr", "preferences.git.parts.pr"),
    ],
  },
  { id: "git.lastCommit", category: "git", label: "statusBar.items.lastCommit", description: "statusBar.items.lastCommitHint" },
  { id: "git.stash", category: "git", label: "statusBar.items.stash", description: "statusBar.items.stashHint" },
  { id: "git.upstream", category: "git", label: "statusBar.items.upstream", description: "statusBar.items.upstreamHint" },
  { id: "git.lastFetch", category: "git", label: "statusBar.items.lastFetch", description: "statusBar.items.lastFetchHint" },
  {
    id: "path.cwd",
    category: "path",
    label: "statusBar.items.cwd",
    description: "statusBar.items.cwdHint",
    options: [
      {
        key: "format",
        label: "statusBar.options.format",
        kind: "choice",
        default: "home",
        choices: [
          { value: "full", label: "statusBar.options.pathFull" },
          { value: "home", label: "statusBar.options.pathHome" },
          { value: "repo", label: "statusBar.options.pathRepo" },
          { value: "folder", label: "statusBar.options.pathFolder" },
        ],
      },
    ],
  },
  { id: "console.process", category: "console", label: "statusBar.items.process", description: "statusBar.items.processHint" },
  { id: "console.size", category: "console", label: "statusBar.items.size", description: "statusBar.items.sizeHint" },
  { id: "console.prompt", category: "console", label: "statusBar.items.prompt", description: "statusBar.items.promptHint" },
  { id: "console.memory", category: "diagnostics", label: "statusBar.items.consoleMemory", description: "statusBar.items.consoleMemoryHint" },
  { id: "console.cpu", category: "diagnostics", label: "statusBar.items.consoleCpu", description: "statusBar.items.consoleCpuHint" },
  { id: "system.appMemory", category: "diagnostics", label: "statusBar.items.appMemory", description: "statusBar.items.appMemoryHint" },
  { id: "system.cpu", category: "diagnostics", label: "statusBar.items.systemCpu", description: "statusBar.items.systemCpuHint" },
  { id: "system.memory", category: "diagnostics", label: "statusBar.items.systemMemory", description: "statusBar.items.systemMemoryHint" },
  {
    id: "system.clock",
    category: "system",
    label: "statusBar.items.clock",
    description: "statusBar.items.clockHint",
    options: [toggle("seconds", "statusBar.options.seconds", false)],
  },
  { id: "system.daemon", category: "system", label: "statusBar.items.daemon", description: "statusBar.items.daemonHint" },
  { id: "system.remote", category: "system", label: "statusBar.items.remote", description: "statusBar.items.remoteHint" },
];

const SPECS = new Map(STATUS_ITEMS.map((spec) => [spec.id, spec]));

export function itemSpec(id: StatusItemId): StatusItemSpec {
  return SPECS.get(id)!;
}

export function isStatusItemId(id: string): id is StatusItemId {
  return SPECS.has(id as StatusItemId);
}

/** The bar as it was before it could be arranged: what a user who never
 * touched it keeps seeing. */
export const DEFAULT_LAYOUT: StatusBarLayout = {
  left: [{ id: "ai.mode" }, { id: "agent.engine" }, { id: "agent.plan" }, { id: "agent.context" }, { id: "agent.cost" }, { id: "companion.nudge" }],
  right: [{ id: "system.daemon" }, { id: "system.remote" }, { id: "git.repo" }, { id: "console.memory" }],
};

export interface LegacyGitSettings {
  visible: boolean;
  parts: Record<"branch" | "sync" | "changes" | "pr", boolean>;
}

/** The default layout, with what the Git section used to say about the
 * repository's item: hidden, or with some of its parts off. */
export function defaultLayout(git?: LegacyGitSettings): StatusBarLayout {
  const right = DEFAULT_LAYOUT.right.flatMap((ref): StatusItemRef[] => {
    if (ref.id !== "git.repo" || !git) return [ref];
    if (!git.visible) return [];
    const off = Object.entries(git.parts).filter(([, shown]) => !shown);
    return [off.length ? { id: "git.repo", options: Object.fromEntries(off.map(([part]) => [part, "off"])) } : ref];
  });
  return { left: [...DEFAULT_LAYOUT.left], right };
}

/** What can be trusted of a layout read from the config: items Termo knows,
 * each once, with only the options it has and values they can take. A
 * hand-edited file or an item a later version dropped is no reason for the
 * bar to break. */
export function normalizeLayout(raw: unknown): StatusBarLayout | null {
  if (!raw || typeof raw !== "object") return null;
  const seen = new Set<StatusItemId>();
  const side = (value: unknown): StatusItemRef[] => {
    if (!Array.isArray(value)) return [];
    const refs: StatusItemRef[] = [];
    for (const entry of value) {
      const id = (entry as { id?: unknown } | null)?.id;
      if (typeof id !== "string" || !isStatusItemId(id) || seen.has(id)) continue;
      seen.add(id);
      const options = cleanOptions(id, (entry as { options?: unknown }).options);
      refs.push(options ? { id, options } : { id });
    }
    return refs;
  };
  const layout = raw as { left?: unknown; right?: unknown };
  return { left: side(layout.left), right: side(layout.right) };
}

function cleanOptions(id: StatusItemId, raw: unknown): Record<string, string> | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const kept: Record<string, string> = {};
  for (const spec of itemSpec(id).options ?? []) {
    const value = (raw as Record<string, unknown>)[spec.key];
    if (typeof value !== "string") continue;
    const valid = spec.kind === "toggle" ? value === "on" || value === "off" : spec.choices.some((choice) => choice.value === value);
    if (valid) kept[spec.key] = value;
  }
  return Object.keys(kept).length ? kept : undefined;
}

/** An option's value for an item in the bar, or its default. */
export function optionValue(ref: StatusItemRef, key: string): string {
  const stored = ref.options?.[key];
  if (stored !== undefined) return stored;
  const spec = itemSpec(ref.id).options?.find((option) => option.key === key);
  if (!spec) return "";
  return spec.kind === "toggle" ? (spec.default ? "on" : "off") : spec.default;
}

export function optionOn(ref: StatusItemRef, key: string): boolean {
  return optionValue(ref, key) === "on";
}

export function containsItem(layout: StatusBarLayout, id: StatusItemId): boolean {
  return layout.left.some((ref) => ref.id === id) || layout.right.some((ref) => ref.id === id);
}

/** Where an item is, if it's in the bar. */
export function findItem(layout: StatusBarLayout, id: StatusItemId): { side: StatusSide; index: number } | null {
  for (const side of ["left", "right"] as const) {
    const index = layout[side].findIndex((ref) => ref.id === id);
    if (index >= 0) return { side, index };
  }
  return null;
}

export function removeItem(layout: StatusBarLayout, id: StatusItemId): StatusBarLayout {
  return { left: layout.left.filter((ref) => ref.id !== id), right: layout.right.filter((ref) => ref.id !== id) };
}

/** Puts an item at `index` of `side`, taking it from wherever it was (with
 * its options) or adding it. `index` counts the side as it is after the
 * item left it, so dropping an item back where it was changes nothing. */
export function placeItem(layout: StatusBarLayout, id: StatusItemId, side: StatusSide, index: number): StatusBarLayout {
  const existing = [...layout.left, ...layout.right].find((ref) => ref.id === id) ?? { id };
  const without = removeItem(layout, id);
  const target = [...without[side]];
  target.splice(Math.max(0, Math.min(index, target.length)), 0, existing);
  return { ...without, [side]: target };
}

/** Sets one option, dropping it when it's back to its default so the layout
 * holds only what the user changed. */
export function setItemOption(layout: StatusBarLayout, id: StatusItemId, key: string, value: string): StatusBarLayout {
  const spec = itemSpec(id).options?.find((option) => option.key === key);
  if (!spec) return layout;
  const fallback = spec.kind === "toggle" ? (spec.default ? "on" : "off") : spec.default;
  const update = (ref: StatusItemRef): StatusItemRef => {
    if (ref.id !== id) return ref;
    const options = { ...ref.options };
    if (value === fallback) delete options[key];
    else options[key] = value;
    return Object.keys(options).length ? { id, options } : { id };
  };
  return { left: layout.left.map(update), right: layout.right.map(update) };
}

export function sameLayout(a: StatusBarLayout, b: StatusBarLayout): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
