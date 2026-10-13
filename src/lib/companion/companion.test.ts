import { describe, expect, it } from "vitest";
import type { Finding, Live, ModelShare, ServerBrief, SessionSummary, SessionUsage, Tokens, Turn } from "../core/companion-client";
import {
  dailySpend,
  dearestTurns,
  findingAmount,
  findingParams,
  foldSlices,
  formatSpan,
  formatTokens,
  formatUsd,
  modelParts,
  nudgeFor,
  projectName,
  quickAskEngine,
  roundUp,
  sendsExchange,
  sessionCost,
  summarizePeriod,
  tokenShares,
} from "./companion";

const NO_TOKENS: Tokens = { input: 0, cacheWrite: 0, cacheRead: 0, output: 0 };

function turn(over: Partial<Turn>): Turn {
  return {
    at: 0,
    prompt: "",
    promptChars: 0,
    automatic: false,
    model: null,
    effort: null,
    tokens: NO_TOKENS,
    requestCount: 1,
    toolCalls: 0,
    peakContext: 0,
    largestResultChars: 0,
    costUsd: null,
    ...over,
  };
}

function session(over: Partial<SessionUsage>): SessionUsage {
  return {
    tool: "claude",
    sessionId: "abc",
    cwd: null,
    title: null,
    startedAt: 0,
    endedAt: 0,
    turns: [],
    tokens: NO_TOKENS,
    subagentTokens: NO_TOKENS,
    subagentCostUsd: null,
    compactions: [],
    servers: [],
    costUsd: null,
    reportedCostUsd: null,
    coverage: { tokens: true, cache: true, prompts: true, tools: true, subagents: true, priced: true },
    ...over,
  };
}

describe("formatting", () => {
  it("reads token counts at a glance", () => {
    expect(formatTokens(850)).toBe("850");
    expect(formatTokens(12_400)).toBe("12.4k");
    expect(formatTokens(150_000)).toBe("150k");
    expect(formatTokens(3_000_000)).toBe("3M");
    expect(formatTokens(528_405_458)).toBe("528M");
  });

  it("never shows a cost as nothing when it was something", () => {
    expect(formatUsd(0)).toBe("$0.00");
    expect(formatUsd(0.004)).toBe("<$0.01");
    expect(formatUsd(12.4)).toBe("$12.40");
    expect(formatUsd(226.4)).toBe("$226");
  });

  it("gives a span in its largest two units", () => {
    expect(formatSpan(45 * 60_000)).toBe("45 min");
    expect(formatSpan(125 * 60_000)).toBe("2 h 5 min");
    expect(formatSpan(120 * 60_000)).toBe("2 h");
    expect(formatSpan(76 * 3_600_000)).toBe("3 d 4 h");
  });

  it("splits tokens into shares of the whole", () => {
    const shares = tokenShares({ input: 10, cacheWrite: 20, cacheRead: 60, output: 10 });
    expect(shares).toEqual({ input: 10, cacheWrite: 20, cacheRead: 60, output: 10 });
    expect(tokenShares(NO_TOKENS).input).toBe(0);
  });
});

describe("a session's cost", () => {
  it("is the agent's own figure when it wrote one down", () => {
    expect(sessionCost(session({ costUsd: 203, reportedCostUsd: 226 }))).toEqual({ usd: 226, source: "reported" });
    expect(sessionCost(session({ costUsd: 203 }))).toEqual({ usd: 203, source: "estimated" });
    expect(sessionCost(session({}))).toBeNull();
  });

  it("lists the dearest turns first, by tokens where there's no price", () => {
    const turns = [
      turn({ costUsd: 1 }),
      turn({ costUsd: 5 }),
      turn({ costUsd: 3, requestCount: 0 }),
      turn({ tokens: { ...NO_TOKENS, input: 2_000_000 } }),
    ];
    expect(dearestTurns(session({ turns }), 2).map((entry) => entry.index)).toEqual([1, 0]);
    expect(dearestTurns(session({ turns }), 9).map((entry) => entry.index)).toEqual([1, 0, 3]);
  });
});

describe("a finding", () => {
  const finding: Finding = {
    kind: "cacheExpired",
    severity: "warning",
    turns: [2],
    numbers: { times: 3, rewrittenTokens: 3_904_666, longestGapMinutes: 1300, share: 0.62, spentUsd: 4.5 },
    wastedUsd: null,
  };

  it("has its figures written the way each one reads", () => {
    expect(findingParams(finding)).toEqual({
      times: "3",
      rewrittenTokens: "3.9M",
      longestGapMinutes: "21 h 40 min",
      share: "62%",
      spentUsd: "$4.50",
    });
  });

  it("says what it wasted, or else what its turns came to", () => {
    expect(findingAmount(finding)).toEqual({ usd: 4.5, kind: "spent" });
    expect(findingAmount({ ...finding, wastedUsd: 18.7 })).toEqual({ usd: 18.7, kind: "wasted" });
    expect(findingAmount({ ...finding, numbers: {} })).toBeNull();
  });
});

describe("a period", () => {
  function summary(over: Partial<SessionSummary>): SessionSummary {
    return {
      tool: "claude",
      sessionId: "a",
      cwd: "E:\\work\\termo",
      title: null,
      startedAt: 0,
      endedAt: 0,
      turns: 1,
      tokens: { ...NO_TOKENS, input: 1000 },
      costUsd: 1,
      costReported: false,
      priced: true,
      models: [],
      servers: [],
      findings: [],
      ...over,
    };
  }

  const share = (model: string, tokens: number, costUsd: number | null, over: Partial<ModelShare> = {}): ModelShare => ({
    model,
    tokens,
    costUsd,
    turns: 0,
    lightTurns: null,
    lightCostUsd: null,
    topTier: null,
    ...over,
  });

  it("adds up its sessions, by agent and by project, dearest first", () => {
    const period = summarizePeriod([
      summary({ costUsd: 2, findings: [{ kind: "cacheExpired", severity: "info", wastedUsd: 0.5 }] }),
      summary({ tool: "codex", cwd: "/home/me/api", costUsd: 5 }),
      summary({
        costUsd: 3,
        findings: [
          { kind: "cacheExpired", severity: "warning", wastedUsd: 1.5 },
          { kind: "longSession", severity: "info", wastedUsd: null },
        ],
      }),
      // Billed by request: its tokens count, and it has no cost to add.
      summary({ tool: "copilot", cwd: null, costUsd: null, priced: false }),
    ]);

    expect(period).toMatchObject({ sessions: 4, tokens: 4000, costUsd: 10, unpriced: 1 });
    expect(period.byTool).toEqual([
      { key: "claude", sessions: 2, tokens: 2000, costUsd: 5 },
      { key: "codex", sessions: 1, tokens: 1000, costUsd: 5 },
      { key: "copilot", sessions: 1, tokens: 1000, costUsd: 0 },
    ]);
    expect(period.byProject.map((row) => row.key)).toEqual(["E:\\work\\termo", "/home/me/api", null]);
    expect(period.habits).toEqual([
      { kind: "cacheExpired", sessions: 2, wastedUsd: 2 },
      { kind: "longSession", sessions: 1, wastedUsd: null },
    ]);
  });

  it("is empty without sessions", () => {
    expect(summarizePeriod([])).toEqual({
      sessions: 0,
      tokens: 0,
      costUsd: 0,
      unpriced: 0,
      byTool: [],
      byModel: [],
      byProject: [],
      byServer: [],
      habits: [],
    });
  });

  it("splits each session's cost among its models, and says how many of their messages asked for little", () => {
    const period = summarizePeriod([
      // The agent's own figure, subagents and all: twice what its models' requests add up to.
      summary({
        costUsd: 8,
        models: [
          share("opus", 900, 3, { turns: 10, lightTurns: 4, lightCostUsd: 0.5, topTier: true }),
          share("haiku", 100, 1, { turns: 2, lightTurns: 2, lightCostUsd: 0.1, topTier: false }),
        ],
      }),
      summary({ costUsd: 2, models: [share("opus", 500, 2, { turns: 5, lightTurns: 1, lightCostUsd: 0.25, topTier: true })] }),
      // A model without a price: by tokens, of a cost that isn't known.
      summary({ tool: "gemini", costUsd: null, models: [share("new-model", 700, null, { turns: 3 })] }),
    ]);
    expect(period.byModel).toEqual([
      { key: "opus", sessions: 2, tokens: 1400, costUsd: 8, turns: 15, lightTurns: 5, lightCostUsd: 0.75, topTier: true },
      { key: "haiku", sessions: 1, tokens: 100, costUsd: 2, turns: 2, lightTurns: 2, lightCostUsd: 0.1, topTier: false },
      { key: "new-model", sessions: 1, tokens: 700, costUsd: 0, turns: 3, lightTurns: null, lightCostUsd: null, topTier: null },
    ]);
    expect(modelParts(summary({ costUsd: 6, models: [share("a", 100, null), share("b", 200, 1)] })).map((part) => part.costUsd)).toEqual([2, 4]);
  });

  it("draws the dearest few as parts of a ring, and the rest as one", () => {
    const slices = [5, 3, 0, 2, 1].map((value, index) => ({ key: `k${index}`, label: `n${index}`, value }));
    expect(foldSlices(slices, 5, "Others").map((slice) => slice.key)).toEqual(["k0", "k1", "k3", "k4"]);
    expect(foldSlices(slices, 2, "Others")).toEqual([slices[0], slices[1], { key: "", label: "Others", value: 3, rest: true }]);
  });

  it("puts the servers never called first, with what carrying them cost", () => {
    const server = (name: string, calls: number, carriedUsd: number | null, contextTokens = 100): ServerBrief => ({
      name,
      tools: 3,
      contextTokens,
      calls,
      carriedUsd,
    });
    const period = summarizePeriod([
      summary({ servers: [server("docs", 2, 0.4), server("vercel", 0, 1.5, 2900), server("drive", 0, null)] }),
      summary({ servers: [server("docs", 0, 0.1), server("vercel", 0, 0.5, 3300)] }),
    ]);
    expect(period.byServer).toEqual([
      { name: "vercel", sessions: 2, usedSessions: 0, calls: 0, contextTokens: 3300, idleUsd: 2 },
      { name: "drive", sessions: 1, usedSessions: 0, calls: 0, contextTokens: 100, idleUsd: null },
      // Used in one session; what it cost in the other is all that was idle.
      { name: "docs", sessions: 2, usedSessions: 1, calls: 2, contextTokens: 100, idleUsd: 0.1 },
    ]);
  });

  it("draws each day's spend by model, on a round scale", () => {
    const at = (day: number, hour = 12) => new Date(2026, 9, day, hour).getTime();
    const spend = dailySpend(
      [
        summary({ endedAt: at(12, 9), costUsd: 30, models: [share("opus", 10, 20), share("haiku", 10, 10)] }),
        summary({ endedAt: at(12, 1), costUsd: 4, models: [share("sonnet", 10, 4)] }),
        summary({ endedAt: at(10), costUsd: 7, models: [share("opus", 10, 7)] }),
        // Worked in before the period began.
        summary({ endedAt: at(2), costUsd: 99, models: [share("opus", 10, 99)] }),
      ],
      3,
      at(12, 18),
      ["opus", "haiku"],
    );
    expect(spend.unit).toBe("usd");
    expect(spend.days.map((day) => new Date(day.at).getDate())).toEqual([9, 10, 11, 12]);
    expect(spend.days.map((day) => day.total)).toEqual([0, 7, 0, 34]);
    // Opus, Haiku, and every other model together.
    expect(spend.days[3].parts).toEqual([20, 10, 4]);
    expect(spend.top).toBe(50);

    const unpriced = dailySpend([summary({ endedAt: at(12), costUsd: null, models: [share("new-model", 700, null)] })], 3, at(12, 18), []);
    expect([unpriced.unit, unpriced.days[3].parts, unpriced.top]).toEqual(["tokens", [700], 1000]);
    expect([roundUp(0), roundUp(0.13), roundUp(1), roundUp(101)]).toEqual([1, 0.2, 1, 200]);
  });

  it("names a project by its folder", () => {
    expect(projectName("E:\\Proyectos\\Termini")).toBe("Termini");
    expect(projectName("/home/me/api/")).toBe("api");
    expect(projectName("/")).toBe("/");
  });
});

describe("the notice on the status bar", () => {
  const MINUTE = 60_000;
  const finding = (kind: Finding["kind"], severity: Finding["severity"]): Finding => ({ kind, severity, turns: [], numbers: {}, wastedUsd: null });
  const live = (over: Partial<Live>): Live => ({ contextTokens: 0, cacheExpiresAt: null, cacheTtlMs: null, findings: [], ...over });
  const hour = live({ cacheExpiresAt: 60 * MINUTE, cacheTtlMs: 60 * MINUTE });

  it("is the gravest finding, once a session", () => {
    const now = live({ findings: [finding("contextBloat", "high"), finding("largeResults", "warning")] });
    expect(nudgeFor("s", now, 0, [], {})).toEqual({ kind: "contextBloat", key: "s:contextBloat" });
    // Looked at, or muted for good: the next one down.
    expect(nudgeFor("s", now, 0, [], { "s:contextBloat": true })?.kind).toBe("largeResults");
    expect(nudgeFor("s", now, 0, ["contextBloat"], {})?.kind).toBe("largeResults");
    expect(nudgeFor("s", now, 0, ["contextBloat", "largeResults"], {})).toBeNull();
    // Another session's is its own.
    expect(nudgeFor("other", now, 0, [], { "s:contextBloat": true })?.kind).toBe("contextBloat");
    expect(nudgeFor("s", live({}), 0, [], {})).toBeNull();
  });

  it("warns of the cache only in its last stretch", () => {
    expect(nudgeFor("s", hour, 30 * MINUTE, [], {})).toBeNull();
    expect(nudgeFor("s", hour, 56 * MINUTE, [], {})).toEqual({ kind: "cacheExpiring", key: `s:cacheExpiring:${60 * MINUTE}`, minutes: 4 });
    expect(nudgeFor("s", hour, 59.8 * MINUTE, [], {})?.minutes).toBe(1);
    // Gone already: nothing left to save.
    expect(nudgeFor("s", hour, 61 * MINUTE, [], {})).toBeNull();
    // A five-minute cache gets a little over a minute's warning.
    const short = live({ cacheExpiresAt: 5 * MINUTE, cacheTtlMs: 5 * MINUTE });
    expect(nudgeFor("s", short, 3 * MINUTE, [], {})).toBeNull();
    expect(nudgeFor("s", short, 4 * MINUTE, [], {})?.kind).toBe("cacheExpiring");
  });

  it("puts the cache before a finding, and each expiry gets its own notice", () => {
    const both = live({ ...hour, findings: [finding("contextBloat", "high")] });
    expect(nudgeFor("s", both, 58 * MINUTE, [], {})?.kind).toBe("cacheExpiring");
    expect(nudgeFor("s", both, 58 * MINUTE, ["cacheExpiring"], {})?.kind).toBe("contextBloat");
    const seen = { [`s:cacheExpiring:${60 * MINUTE}`]: true as const };
    expect(nudgeFor("s", both, 58 * MINUTE, [], seen)?.kind).toBe("contextBloat");
    const later = live({ cacheExpiresAt: 120 * MINUTE, cacheTtlMs: 60 * MINUTE });
    expect(nudgeFor("s", later, 118 * MINUTE, [], seen)?.kind).toBe("cacheExpiring");
  });
});

describe("a quick question", () => {
  it("is answered by the engine picked, when it's installed", () => {
    expect(quickAskEngine("codex", ["claude", "codex"])).toBe("codex");
    expect(quickAskEngine("gemini", ["claude", "codex"])).toBe("claude");
    expect(quickAskEngine("", ["grok"])).toBe("grok");
    expect(quickAskEngine("claude", [])).toBeNull();
  });

  it("is answered by the console's own agent when none is picked", () => {
    expect(quickAskEngine("", ["claude", "codex"], "codex")).toBe("codex");
    expect(quickAskEngine("claude", ["claude", "codex"], "codex")).toBe("claude");
    expect(quickAskEngine("gemini", ["claude"], "grok")).toBe("grok");
    expect(quickAskEngine("", ["claude"], null)).toBe("claude");
  });

  it("can take the last exchange along only from a Claude Code console", () => {
    expect(sendsExchange("claude")).toBe(true);
    expect(sendsExchange("codex")).toBe(false);
    expect(sendsExchange(undefined)).toBe(false);
  });
});
