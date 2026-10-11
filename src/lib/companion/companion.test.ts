import { describe, expect, it } from "vitest";
import type { Finding, SessionUsage, Tokens, Turn } from "../core/companion-client";
import {
  dearestTurns,
  findingAmount,
  findingParams,
  formatSpan,
  formatTokens,
  formatUsd,
  sessionCost,
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
