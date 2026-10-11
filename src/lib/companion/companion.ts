import type { Finding, FindingKind, SessionUsage, Tokens, Turn } from "../core/companion-client";
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
