import { describe, expect, it } from "vitest";
import type { ToolId } from "../ai-mode/pattern-table";
import { checkMissingAgents, forgetAgents, sameKnown } from "./missing-agents";

const NONE: Record<ToolId, boolean> = { claude: false, codex: false, gemini: false, copilot: false, grok: false };

describe("agents that went missing", () => {
  it("remembers each one found, under the Node.js it was found with", () => {
    const check = checkMissingAgents({}, { ...NONE, claude: true, codex: true }, "v20.20.1");
    expect(check.missing).toEqual([]);
    expect(check.known).toEqual({ claude: "v20.20.1", codex: "v20.20.1" });
  });

  it("names the ones a change of Node.js left behind, and keeps them for next time", () => {
    const known = { claude: "v20.20.1", codex: "v20.20.1", grok: "v20.20.1" };
    const check = checkMissingAgents(known, { ...NONE, claude: true }, "v24.21.0");
    expect(check.missing).toEqual(["codex", "grok"]);
    expect(check.known).toEqual({ claude: "v24.21.0", codex: "v20.20.1", grok: "v20.20.1" });
  });

  it("forgets one taken off under the same Node.js, without a word", () => {
    const check = checkMissingAgents({ codex: "v24.21.0", gemini: "v24.21.0" }, { ...NONE, gemini: true }, "v24.21.0");
    expect(check.missing).toEqual([]);
    expect(check.known).toEqual({ gemini: "v24.21.0" });
  });

  it("says nothing and forgets nothing while there's no Node.js", () => {
    const known = { claude: "v20.20.1", codex: "v20.20.1" };
    const check = checkMissingAgents(known, { ...NONE, claude: true, gemini: true }, null);
    expect(check.missing).toEqual([]);
    expect(check.known).toEqual(known);
  });

  it("says nothing of one never found", () => {
    expect(checkMissingAgents({}, NONE, "v24.21.0")).toEqual({ missing: [], known: {} });
  });

  it("stops asking about the ones it's told to forget", () => {
    const known = { codex: "v20.20.1", grok: "v20.20.1", claude: "v24.21.0" };
    const left = forgetAgents(known, ["codex", "grok"]);
    expect(left).toEqual({ claude: "v24.21.0" });
    expect(checkMissingAgents(left, { ...NONE, claude: true }, "v24.21.0").missing).toEqual([]);
  });

  it("compares what's remembered", () => {
    expect(sameKnown({ a: "1", b: "2" }, { b: "2", a: "1" })).toBe(true);
    expect(sameKnown({ a: "1" }, { a: "2" })).toBe(false);
    expect(sameKnown({ a: "1" }, { a: "1", b: "2" })).toBe(false);
  });
});
