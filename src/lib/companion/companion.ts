import type {
  CompanionTool,
  Finding,
  FindingKind,
  Live,
  SessionSummary,
  SessionUsage,
  Tokens,
  Turn,
} from "../core/companion-client";
import type { MessageKey } from "../i18n/translate";

/** "850", "12.4k", "3.2M": token counts are only ever read at a glance. */
export function formatTokens(count: number): string {
  if (count < 1000) return String(Math.round(count));
  if (count < 1_000_000) return `${trim(count / 1000)}k`;
  return `${trim(count / 1_000_000)}M`;
}

/** One decimal under 100, none from there on. */
function trim(value: number): string {
  return value >= 100 ? String(Math.round(value)) : value.toFixed(1).replace(/\.0$/, "");
}

/** "$12.40", "$0.03", and "<$0.01" for what would round to nothing. */
export function formatUsd(dollars: number): string {
  if (dollars > 0 && dollars < 0.01) return "<$0.01";
  return `$${dollars >= 100 ? Math.round(dollars) : dollars.toFixed(2)}`;
}

/** "45 min", "2 h 5 min", "3 d 4 h": the same in both languages. */
export function formatSpan(ms: number): string {
  const minutes = Math.max(0, Math.round(ms / 60_000));
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return minutes % 60 === 0 ? `${hours} h` : `${hours} h ${minutes % 60} min`;
  const days = Math.floor(hours / 24);
  return hours % 24 === 0 ? `${days} d` : `${days} d ${hours % 24} h`;
}

export type TokenPart = keyof Tokens;

/** The order the breakdown is drawn in: what was sent, then what came back. */
export const TOKEN_PARTS: readonly TokenPart[] = ["input", "cacheWrite", "cacheRead", "output"];

export function totalTokens(tokens: Tokens): number {
  return tokens.input + tokens.cacheWrite + tokens.cacheRead + tokens.output;
}

/** Each part's share of the whole, 0–100, for the breakdown bar. */
export function tokenShares(tokens: Tokens): Record<TokenPart, number> {
  const total = totalTokens(tokens) || 1;
  return {
    input: (tokens.input / total) * 100,
    cacheWrite: (tokens.cacheWrite / total) * 100,
    cacheRead: (tokens.cacheRead / total) * 100,
    output: (tokens.output / total) * 100,
  };
}

/** The session's cost and where the figure comes from: the agent's own when
 * it wrote one down, else the estimate at API prices, else nothing. */
export function sessionCost(session: SessionUsage): { usd: number; source: "reported" | "estimated" } | null {
  if (session.reportedCostUsd != null) return { usd: session.reportedCostUsd, source: "reported" };
  if (session.costUsd != null) return { usd: session.costUsd, source: "estimated" };
  return null;
}

/** The turns that cost the most, dearest first, each with its place in the
 * session. By tokens where no price is known. */
export function dearestTurns(session: SessionUsage, limit: number): { index: number; turn: Turn }[] {
  const weight = (turn: Turn) => turn.costUsd ?? totalTokens(turn.tokens) / 1e9;
  return session.turns
    .map((turn, index) => ({ index, turn }))
    .filter(({ turn }) => turn.requestCount > 0)
    .sort((a, b) => weight(b.turn) - weight(a.turn))
    .slice(0, limit);
}

export interface FindingText {
  title: MessageKey;
  text: MessageKey;
  tip: MessageKey;
}

export const FINDING_TEXT: Record<FindingKind, FindingText> = {
  contextBloat: {
    title: "companion.findings.contextBloat.title",
    text: "companion.findings.contextBloat.text",
    tip: "companion.findings.contextBloat.tip",
  },
  cacheExpired: {
    title: "companion.findings.cacheExpired.title",
    text: "companion.findings.cacheExpired.text",
    tip: "companion.findings.cacheExpired.tip",
  },
  oversizedModel: {
    title: "companion.findings.oversizedModel.title",
    text: "companion.findings.oversizedModel.text",
    tip: "companion.findings.oversizedModel.tip",
  },
  smallTurnsLargeContext: {
    title: "companion.findings.smallTurnsLargeContext.title",
    text: "companion.findings.smallTurnsLargeContext.text",
    tip: "companion.findings.smallTurnsLargeContext.tip",
  },
  largeResults: {
    title: "companion.findings.largeResults.title",
    text: "companion.findings.largeResults.text",
    tip: "companion.findings.largeResults.tip",
  },
  repeatedReads: {
    title: "companion.findings.repeatedReads.title",
    text: "companion.findings.repeatedReads.text",
    tip: "companion.findings.repeatedReads.tip",
  },
  subagentHeavy: {
    title: "companion.findings.subagentHeavy.title",
    text: "companion.findings.subagentHeavy.text",
    tip: "companion.findings.subagentHeavy.tip",
  },
  heavyBaseline: {
    title: "companion.findings.heavyBaseline.title",
    text: "companion.findings.heavyBaseline.text",
    tip: "companion.findings.heavyBaseline.tip",
  },
  longSession: {
    title: "companion.findings.longSession.title",
    text: "companion.findings.longSession.text",
    tip: "companion.findings.longSession.tip",
  },
};

/** How each of a finding's figures reads in its sentence, by what it counts
 * (see src-tauri/src/companion/rules.rs for the names). */
function formatNumber(name: string, value: number): string {
  if (name.endsWith("Tokens") || name === "tokens") return formatTokens(value);
  if (name.endsWith("Chars")) return formatTokens(value);
  if (name.endsWith("Usd")) return formatUsd(value);
  if (name.endsWith("Minutes")) return formatSpan(value * 60_000);
  if (name === "share") return `${Math.round(value * 100)}%`;
  if (name === "days") return value < 1 ? "1" : String(Math.round(value));
  return String(Math.round(value));
}

/** A finding's figures, ready for its sentence's placeholders. */
export function findingParams(finding: Finding): Record<string, string> {
  return Object.fromEntries(Object.entries(finding.numbers).map(([name, value]) => [name, formatNumber(name, value)]));
}

/** What a finding cost, and whether that's money that needn't have been
 * spent or only what the turns it's about came to. */
export function findingAmount(finding: Finding): { usd: number; kind: "wasted" | "spent" } | null {
  if (finding.wastedUsd != null) return { usd: finding.wastedUsd, kind: "wasted" };
  const spent = finding.numbers.spentUsd;
  return spent != null && spent > 0 ? { usd: spent, kind: "spent" } : null;
}

/** A row of the period view: an agent's or a project's share of it. */
export interface PeriodRow<Key> {
  key: Key;
  sessions: number;
  tokens: number;
  /** Of the sessions whose cost is known. */
  costUsd: number;
}

export interface HabitRow {
  kind: FindingKind;
  /** How many sessions it was found in. */
  sessions: number;
  /** Null when it can't be put a figure on in any of them. */
  wastedUsd: number | null;
}

export interface Period {
  sessions: number;
  tokens: number;
  /** Of the sessions whose cost is known. */
  costUsd: number;
  /** Sessions with tokens but no cost: a model without a price, or an agent
   * that isn't billed by token. They add to `tokens` and not to `costUsd`. */
  unpriced: number;
  byTool: PeriodRow<CompanionTool>[];
  /** By folder; the null key is the sessions that have none. */
  byProject: PeriodRow<string | null>[];
  habits: HabitRow[];
}

/** Everything the period view shows, worked out of its sessions: the totals,
 * who and what they went to (dearest first, then by tokens), and the habits
 * found, the ones that wasted the most first. */
export function summarizePeriod(summaries: readonly SessionSummary[]): Period {
  const byTool = new Map<CompanionTool, PeriodRow<CompanionTool>>();
  const byProject = new Map<string | null, PeriodRow<string | null>>();
  const habits = new Map<FindingKind, HabitRow>();
  const period: Period = { sessions: 0, tokens: 0, costUsd: 0, unpriced: 0, byTool: [], byProject: [], habits: [] };

  const add = <Key>(rows: Map<Key, PeriodRow<Key>>, key: Key, tokens: number, cost: number) => {
    const row = rows.get(key) ?? { key, sessions: 0, tokens: 0, costUsd: 0 };
    row.sessions += 1;
    row.tokens += tokens;
    row.costUsd += cost;
    rows.set(key, row);
  };

  for (const summary of summaries) {
    const tokens = totalTokens(summary.tokens);
    const cost = summary.costUsd ?? 0;
    period.sessions += 1;
    period.tokens += tokens;
    period.costUsd += cost;
    if (summary.costUsd == null) period.unpriced += 1;
    add(byTool, summary.tool, tokens, cost);
    add(byProject, summary.cwd, tokens, cost);
    for (const finding of summary.findings) {
      const habit = habits.get(finding.kind) ?? { kind: finding.kind, sessions: 0, wastedUsd: null };
      habit.sessions += 1;
      if (finding.wastedUsd != null) habit.wastedUsd = (habit.wastedUsd ?? 0) + finding.wastedUsd;
      habits.set(finding.kind, habit);
    }
  }

  const dearest = <Key>(a: PeriodRow<Key>, b: PeriodRow<Key>) => b.costUsd - a.costUsd || b.tokens - a.tokens;
  period.byTool = [...byTool.values()].sort(dearest);
  period.byProject = [...byProject.values()].sort(dearest);
  period.habits = [...habits.values()].sort(
    (a, b) => (b.wastedUsd ?? 0) - (a.wastedUsd ?? 0) || b.sessions - a.sessions || a.kind.localeCompare(b.kind),
  );
  return period;
}

/** A folder's own name, which is what tells projects apart at a glance. */
export function projectName(cwd: string): string {
  const parts = cwd.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? cwd;
}

/** What the status bar's notice is about: one of the findings, or the cache
 * about to run out. */
export type NudgeKind = FindingKind | "cacheExpiring";

/** Every kind there can be a notice of, in the order Preferences lists them. */
export const NUDGE_KINDS: readonly NudgeKind[] = ["cacheExpiring", "contextBloat", "smallTurnsLargeContext", "oversizedModel", "largeResults"];

/** The few words a notice is, on the status bar and in Preferences. */
export function nudgeTitle(kind: NudgeKind): MessageKey {
  return kind === "cacheExpiring" ? "companion.nudge.cacheTitle" : FINDING_TEXT[kind].title;
}

export interface Nudge {
  kind: NudgeKind;
  /** What it's remembered by once looked at. A finding's is the session and
   * the kind, so it's shown once a session; the cache's takes the moment it
   * expires, so each time it's about to gets its own. */
  key: string;
  /** For `cacheExpiring`: whole minutes left, at least one. */
  minutes?: number;
}

/** The cache is worth a word in the last stretch before it expires: five
 * minutes of an hour's, a little over one of a five-minute one. */
const CACHE_WARNING_MS = 5 * 60_000;
const CACHE_WARNING_SHARE = 0.25;

/** The one thing to say about a session now, or nothing: the cache about to
 * expire first, since that has a deadline, then the gravest finding. Kinds
 * the user muted, and notices already looked at, are passed over. */
export function nudgeFor(
  sessionId: string,
  live: Live,
  now: number,
  muted: readonly string[],
  seen: Readonly<Record<string, true>>,
): Nudge | null {
  if (live.cacheExpiresAt != null && live.cacheTtlMs != null && !muted.includes("cacheExpiring")) {
    const left = live.cacheExpiresAt - now;
    const warning = Math.min(CACHE_WARNING_MS, live.cacheTtlMs * CACHE_WARNING_SHARE);
    const key = `${sessionId}:cacheExpiring:${live.cacheExpiresAt}`;
    if (left > 0 && left <= warning && !seen[key]) {
      return { kind: "cacheExpiring", key, minutes: Math.max(1, Math.ceil(left / 60_000)) };
    }
  }
  // Already ordered, gravest first.
  for (const finding of live.findings) {
    const key = `${sessionId}:${finding.kind}`;
    if (!muted.includes(finding.kind) && !seen[key]) return { kind: finding.kind, key };
  }
  return null;
}

/** Whose CLI answers a quick question: the one picked in Preferences when
 * it's installed, else the first installed. Null when none is. */
export function quickAskEngine(picked: string, installed: readonly CompanionTool[]): CompanionTool | null {
  return installed.find((engine) => engine === picked) ?? installed[0] ?? null;
}

/** Whether a console's last exchange can go along with a quick question:
 * only where the agent's session files are read for it, which for now is
 * Claude Code's. */
export function sendsExchange(tool: string | undefined): boolean {
  return tool === "claude";
}
