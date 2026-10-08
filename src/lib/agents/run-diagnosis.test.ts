import { describe, expect, it } from "vitest";
import type { RunFailure, SessionInfo } from "$lib/core/agents-client";
import { brokenServers, diagnosisText, failureSource, sessionParts, usageText } from "./run-diagnosis";

/** Echoes the key and its params, so the tests see what was asked for. */
const tr = (key: string, params?: Record<string, string>) =>
  params ? `${key}(${Object.values(params).join(",")})` : key;

const failure: RunFailure = {
  provider: "anthropic",
  engine: "claude",
  kind: "modelAccess",
  status: 403,
  message: "Failed to authenticate. API Error: 403 Access to this model requires an access grant",
  requestId: "req_0123456789",
  model: "claude-opus-5-5",
  exitCode: 1,
  stderr: [],
};

describe("a run's diagnosis", () => {
  it("names who answered and with which status", () => {
    expect(failureSource(failure, tr)).toBe("agents.diagnosis.provider.anthropic(Claude Code) · HTTP 403");
    expect(failureSource({ ...failure, provider: "local", status: null }, tr)).toBe(
      "agents.diagnosis.provider.local(Claude Code)",
    );
  });

  it("puts everything a ticket needs in the text to copy", () => {
    const text = diagnosisText({ ...failure, stderr: ["boom"] }, tr, 0);
    expect(text).toContain("agents.diagnosis.kind.modelAccess.title");
    expect(text).toContain("claude-opus-5-5");
    expect(text).toContain("req_0123456789");
    expect(text).toContain("stderr:\nboom");
    expect(text.trim().endsWith("agents.diagnosis.kind.modelAccess.hint")).toBe(true);
  });

  it("says what ran it and which tools didn't connect", () => {
    const session: SessionInfo = {
      engine: "claude",
      version: "2.1.290",
      model: "",
      auth: "apiKey",
      authSource: "ANTHROPIC_API_KEY",
      cwd: "",
      command: "",
      mcp: [
        { name: "termo", status: "connected" },
        { name: "github", status: "failed" },
      ],
    };
    expect(sessionParts(session, tr)).toEqual([
      "Claude Code 2.1.290",
      "agents.diagnosis.defaultModel",
      "agents.diagnosis.auth.apiKeyFrom(ANTHROPIC_API_KEY)",
    ]);
    expect(brokenServers(session)).toEqual(["github"]);
  });

  it("words usage warnings by their window", () => {
    expect(usageText({ window: "seven_day", status: "allowed_warning", utilization: 0.92, resetsAt: 0 }, tr)).toBe(
      "agents.diagnosis.usage.warning(agents.diagnosis.usage.sevenDay,92%)",
    );
    expect(usageText({ window: "odd", status: "rejected", utilization: 1, resetsAt: 0 }, tr)).toBe(
      "agents.diagnosis.usage.rejected(odd)",
    );
  });
});
