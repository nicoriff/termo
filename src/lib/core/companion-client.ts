import { invoke } from "@tauri-apps/api/core";

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

/** One session, by the agent's own id for it; null when it isn't on disk. */
export function companionSession(tool: string, sessionId: string): Promise<SessionReport | null> {
  return invoke("companion_session", { tool, sessionId });
}

/** The sessions last worked in, newest first. */
export function companionRecent(): Promise<SessionFile[]> {
  return invoke("companion_recent");
}
