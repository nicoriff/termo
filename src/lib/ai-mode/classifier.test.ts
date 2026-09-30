import { describe, expect, it } from "vitest";
import { classify, type ClassifyState } from "./classifier";

describe("classify: claude code", () => {
  it("detects identity and defaults to finished/idle", () => {
    const state: ClassifyState = { detectedTool: null };
    const lines = [
      // The banner's first rows share their line with the mascot art
      // rendered to its left, so the text never starts at column 0 —
      // this fixture must not be anchored/left-aligned, or a regression
      // to a start-of-line-anchored identity regex would go unnoticed.
      "     Claude Code v2.1.269",
      "     Sonnet 5 · Claude Pro",
      "     ~/termo",
      "",
      "> ",
      "auto mode on (shift+tab to cycle) · esc to interrupt · ← for agents",
    ];
    // "esc to interrupt" is always present as a static footer hint, not
    // only while generating — this fixture checks that alone doesn't force
    // a "running" classification without the duration/token structure too.
    expect(classify(lines, state, null)).toEqual({ tool: "claude", status: "finished" });
  });

  it("does not treat the static 'esc to interrupt' footer hint alone as running", () => {
    const state: ClassifyState = { detectedTool: "claude" };
    expect(classify(["esc to interrupt"], state, null).status).toBe("finished");
  });

  it("detects running from the duration/token structure regardless of verb", () => {
    const state: ClassifyState = { detectedTool: "claude" };
    const lines = ["Twisting… (2m 56s · ↓ 6.2k tokens)", "esc to interrupt"];
    expect(classify(lines, state, null).status).toBe("running");
  });

  it("detects running with a different randomized verb", () => {
    const state: ClassifyState = { detectedTool: "claude" };
    const lines = ["Caramelizing… (10s · ↓ 169 tokens)"];
    expect(classify(lines, state, null).status).toBe("running");
  });

  it("detects awaiting from a numbered choice list", () => {
    const state: ClassifyState = { detectedTool: "claude" };
    const lines = ["Do you want to proceed?", "❯ 1. Yes", "2. No"];
    expect(classify(lines, state, null).status).toBe("awaiting");
  });

  it("detects error once identity is already confirmed", () => {
    const state: ClassifyState = { detectedTool: "claude" };
    const lines = ["API Error: Your computer went to sleep mid-request."];
    expect(classify(lines, state, null).status).toBe("error");
  });

  it("does not classify a plain shell with no AI tool detected", () => {
    const state: ClassifyState = { detectedTool: null };
    const lines = ["$ ls", "file.txt  node_modules  src"];
    expect(classify(lines, state, null)).toEqual({ tool: null, status: null });
  });

  it("stays classified once identity is detected even after the banner scrolls away", () => {
    const state: ClassifyState = { detectedTool: null };
    classify(["Claude Code v2.1.269"], state, null);
    const result = classify(["> ", "auto mode on"], state, null);
    expect(result.tool).toBe("claude");
  });
});

describe("classify: process-path identity signal", () => {
  it("matches a real resolved path via substring, not exact basename", () => {
    // Claude Code's installer symlinks a stable `claude` name to a
    // version-pinned binary, so the resolved path's basename is a version
    // string ("2.1.269"), not "claude" — this is the actual real-world path
    // observed from `proc_pidpath` on this machine. An exact-basename match
    // would silently never detect Claude Code sessions at all.
    const state: ClassifyState = { detectedTool: null };
    const result = classify(["$ some plain output"], state, "/Users/nicolas/.local/share/claude/versions/2.1.269");
    expect(result.tool).toBe("claude");
  });

  it("detects codex identity via foreground process path", () => {
    const state: ClassifyState = { detectedTool: null };
    const result = classify(["$ some plain output"], state, "/usr/local/bin/codex");
    expect(result.tool).toBe("codex");
  });
});

describe("classify: gemini (from its documented interface)", () => {
  it("recognizes Gemini CLI by its input box, not as Claude or Codex", () => {
    const state: ClassifyState = { detectedTool: null };
    const lines = [">   Type your message or @path/to/file", "~/proj   no sandbox   gemini-2.5-pro (99% context left)"];
    expect(classify(lines, state, null).tool).toBe("gemini");
  });

  it("reads a running turn, an approval prompt and an API error", () => {
    const state: ClassifyState = { detectedTool: "gemini" };
    expect(classify(["⠧ Reading the files (esc to cancel, 4s)"], state, null).status).toBe("running");
    expect(classify(["Allow execution of: 'npm test'?", "● 1. Allow once"], state, null).status).toBe("awaiting");
    expect(classify(["✕ [API Error: quota exceeded]"], state, null).status).toBe("error");
    expect(classify([">   Type your message or @path/to/file"], state, null).status).toBe("finished");
  });
});

describe("classify: codex (best-effort placeholders)", () => {
  it("detects a generic approval prompt as awaiting", () => {
    const state: ClassifyState = { detectedTool: "codex" };
    const lines = ["Allow this command to run? (y/n)"];
    expect(classify(lines, state, null).status).toBe("awaiting");
  });

  it("detects a generic error keyword once identified", () => {
    const state: ClassifyState = { detectedTool: "codex" };
    expect(classify(["Traceback (most recent call last):"], state, null).status).toBe("error");
  });
});
