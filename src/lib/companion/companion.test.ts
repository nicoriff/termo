import { describe, expect, it } from "vitest";
import type { Finding, SessionSummary, SessionUsage, Tokens, Turn } from "../core/companion-client";
import {
  dearestTurns,
  findingAmount,
  findingParams,
  formatSpan,
  formatTokens,
  formatUsd,
  projectName,
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
      findings: [],
      ...over,
    };
  }

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
    expect(summarizePeriod([])).toEqual({ sessions: 0, tokens: 0, costUsd: 0, unpriced: 0, byTool: [], byProject: [], habits: [] });
  });

  it("names a project by its folder", () => {
    expect(projectName("E:\\Proyectos\\Termini")).toBe("Termini");
    expect(projectName("/home/me/api/")).toBe("api");
    expect(projectName("/")).toBe("/");
  });
});
