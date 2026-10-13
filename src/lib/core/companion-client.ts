import { invoke } from "@tauri-apps/api/core";
import type { PlanStep } from "./orchestration-client";

/** What a request was billed for, by kind (see src-tauri/src/companion). */
export interface Tokens {
  /** Sent and not served from the cache. */
  input: number;
  /** Sent and saved to the cache: dearer than `input`. */
  cacheWrite: number;
  /** Served from the cache: a fraction of `input`. */
  cacheRead: number;
  output: number;
}

/** What the user asked, and everything the agent did to answer it. */
export interface Turn {
  /** Unix milliseconds of the prompt. */
  at: number;
  /** The start of the prompt. */
  prompt: string;
  promptChars: number;
  /** Not typed by the user: a background task reporting back, a resumed
   * session's first work. */
  automatic: boolean;
  model: string | null;
  effort: string | null;
  tokens: Tokens;
  requestCount: number;
  toolCalls: number;
  peakContext: number;
  largestResultChars: number;
  /** At API prices; null when a model's price isn't known. */
  costUsd: number | null;
}

export interface Compaction {
  at: number;
  automatic: boolean;
  tokensBefore: number;
  tokensAfter: number;
}

/** What the agent's session files say; what's false isn't known, not zero. */
export interface Coverage {
  tokens: boolean;
  cache: boolean;
  prompts: boolean;
  tools: boolean;
  subagents: boolean;
  /** Its tokens are billed at a price each; not Copilot CLI's, which counts
   * requests instead. */
  priced: boolean;
}

export type CompanionTool = "claude" | "codex" | "gemini" | "copilot" | "grok";

/** An MCP server a session had connected: what having it there put in every
 * request, and whether it was ever called. */
export interface ServerUse {
  name: string;
  /** The tools of it the agent was told of. */
  tools: number;
  /** Characters every request carried for it: its instructions and its
   * tools' names. */
  contextChars: number;
  calls: number;
}

export interface SessionUsage {
  tool: CompanionTool;
  sessionId: string;
  cwd: string | null;
  title: string | null;
  startedAt: number;
  endedAt: number;
  turns: Turn[];
  tokens: Tokens;
  /** Counted in `tokens` and `costUsd` too. */
  subagentTokens: Tokens;
  subagentCostUsd: number | null;
  compactions: Compaction[];
  /** Empty for the agents whose files don't say: only Claude Code's do. */
  servers: ServerUse[];
  costUsd: number | null;
  /** The agent's own figure, when it wrote one down. */
  reportedCostUsd: number | null;
  coverage: Coverage;
}

export type FindingKind =
  | "contextBloat"
  | "cacheExpired"
  | "oversizedModel"
  | "smallTurnsLargeContext"
  | "largeResults"
  | "repeatedReads"
  | "subagentHeavy"
  | "heavyBaseline"
  | "longSession";

export type Severity = "info" | "warning" | "high";

export interface Finding {
  kind: FindingKind;
  severity: Severity;
  /** The turns it's about, by position in the session. */
  turns: number[];
  /** The figures it rests on, by name. */
  numbers: Record<string, number>;
  /** Dollars that needn't have been spent, where that can be said. */
  wastedUsd: number | null;
}

export interface SessionReport {
  session: SessionUsage;
  findings: Finding[];
  /** The day the prices were last checked, as YYYY-MM-DD. */
  pricesAsOf: string;
}

export interface SessionFile {
  tool: CompanionTool;
  sessionId: string;
  modifiedAt: number;
  bytes: number;
}

/** What's worth a word about a session while it's still going. */
export interface Live {
  /** The context the last request carried. */
  contextTokens: number;
  /** When what the last request cached stops being there (Unix ms), and how
   * long it's kept for; null where that isn't known or doesn't matter. */
  cacheExpiresAt: number | null;
  cacheTtlMs: number | null;
  /** The findings about what to do next that still hold, gravest first. */
  findings: Finding[];
}

/** A session's live reading, asked for as each of its turns ends. */
export function companionLive(tool: string, sessionId: string): Promise<Live | null> {
  return invoke("companion_live", { tool, sessionId });
}

/** A prompt to look over, and how its turn went. */
export interface AdviceRequest {
  /** Whose CLI answers. */
  engine: CompanionTool;
  /** The agent the prompt was sent to, by its name. */
  agent: string;
  prompt: string;
  model: string | null;
  requestCount: number;
  toolCalls: number;
  contextTokens: number;
  outputTokens: number;
}

export interface Advice {
  rewritten: string;
  /** What was changed and why, in a few short lines. */
  changes: string[];
  /** The prompt was fine as it was. */
  alreadyGood: boolean;
  /** What asking cost, as the CLI reports it. */
  costUsd: number | null;
}

/** A prompt written again to cost less, by the CLI of one of the user's
 * agents. The only call here that asks a model, so it costs something and
 * takes a while; it fails with the CLI's own words. */
export function companionAdvise(request: AdviceRequest): Promise<Advice> {
  return invoke("companion_advise", { request });
}

/** The last thing asked in a session, and what its agent answered. */
export interface Exchange {
  prompt: string;
  reply: string;
}

export interface QuickRequest {
  /** Whose CLI answers. */
  engine: CompanionTool;
  /** The folder of the console the question came up in. */
  cwd: string | null;
  question: string;
  /** The questions asked before it, oldest first. */
  history: { question: string; answer: string }[];
  /** The session's last exchange, when it goes along. */
  exchange: Exchange | null;
}

export interface QuickAnswer {
  answer: string;
  /** What asking cost, as the CLI reports it. */
  costUsd: number | null;
  /** The model that answered, where the CLI says: Claude Code's only. */
  model: string | null;
}

/** A step of a quick question being answered: the `companion-quick-step`
 * event, with the id its asking was given. */
export interface QuickStep {
  id: string;
  step: PlanStep;
}

/** A quick question, answered to one side of the session it came up in (see
 * src-tauri/src/companion/quick.rs). It asks a model, so it costs something
 * and takes a while; it fails with the CLI's own words. */
export function companionQuickAsk(id: string, request: QuickRequest): Promise<QuickAnswer> {
  return invoke("companion_quick_ask", { id, request });
}

/** A session's last exchange; null where there's none to read, which for
 * now is every agent's but Claude Code's. */
export function companionLastExchange(tool: string, sessionId: string): Promise<Exchange | null> {
  return invoke("companion_last_exchange", { tool, sessionId });
}

export interface ModelShare {
  model: string;
  tokens: number;
  costUsd: number | null;
  /** The prompts the user typed that it did most of the answering of. */
  turns: number;
  /** Of those, the ones that asked for little, and what they cost; null
   * where the agent's files don't say enough to tell. */
  lightTurns: number | null;
  lightCostUsd: number | null;
  /** One of the dear ones; null when its price isn't known. */
  topTier: boolean | null;
}

/** An MCP server a session had connected, in brief. */
export interface ServerBrief {
  name: string;
  tools: number;
  /** What it added to every request, roughly. */
  contextTokens: number;
  calls: number;
  /** What carrying that through the session came to; null when a model's
   * price isn't known. */
  carriedUsd: number | null;
}

export interface FindingBrief {
  kind: FindingKind;
  severity: Severity;
  wastedUsd: number | null;
}

/** A session in brief, for looking at many at once. */
export interface SessionSummary {
  tool: CompanionTool;
  sessionId: string;
  cwd: string | null;
  title: string | null;
  startedAt: number;
  endedAt: number;
  /** The prompts the user typed. */
  turns: number;
  tokens: Tokens;
  /** The agent's own figure when it wrote one down, else the estimate. */
  costUsd: number | null;
  costReported: boolean;
  /** Billed by token at all (see Coverage.priced). */
  priced: boolean;
  models: ModelShare[];
  servers: ServerBrief[];
  findings: FindingBrief[];
}

/** Every agent's sessions of the last `days`, in brief, newest first. The
 * first look at a stretch reads every session in it, which can take a few
 * seconds; after that only the ones worked in since. */
export function companionOverview(days: number): Promise<SessionSummary[]> {
  return invoke("companion_overview", { days });
}

/** One session, by the agent's own id for it; null when it isn't on disk. */
export function companionSession(tool: string, sessionId: string): Promise<SessionReport | null> {
  return invoke("companion_session", { tool, sessionId });
}

/** The sessions last worked in, newest first. */
export function companionRecent(): Promise<SessionFile[]> {
  return invoke("companion_recent");
}
