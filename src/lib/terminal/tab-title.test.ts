import { describe, expect, it } from "vitest";
import {
  applyProcessChange,
  applyProgramTitle,
  cleanProgramTitle,
  EMPTY_TAB_TITLE,
  processDisplayName,
  resolveTabTitle,
  TITLE_GRACE_MS,
} from "./tab-title";

describe("processDisplayName", () => {
  it("hides the shell itself", () => {
    expect(processDisplayName("/bin/zsh")).toBeNull();
    expect(processDisplayName("C:\\Windows\\System32\\cmd.exe")).toBeNull();
    expect(processDisplayName("/opt/tools/xonsh", "/opt/tools/xonsh")).toBeNull();
  });

  it("uses the executable name for other programs", () => {
    expect(processDisplayName("/usr/bin/vim")).toBe("vim");
    expect(processDisplayName("/usr/local/bin/node")).toBe("node");
  });

  it("names version-pinned binaries after their tool", () => {
    expect(processDisplayName("/Users/nicolas/.local/share/claude/versions/2.1.269")).toBe("claude");
  });

  it("returns null without a path", () => {
    expect(processDisplayName(null)).toBeNull();
  });
});

describe("cleanProgramTitle", () => {
  it("collapses whitespace and drops empty titles", () => {
    expect(cleanProgramTitle("  zsh   in\ttermini ")).toBe("zsh in termini");
    expect(cleanProgramTitle("   ")).toBeNull();
  });

  it("truncates long titles", () => {
    const title = cleanProgramTitle("x".repeat(80))!;
    expect(title).toHaveLength(40);
    expect(title.endsWith("…")).toBe(true);
  });
});

describe("applyProcessChange", () => {
  it("drops a title set long before the process changed", () => {
    const withTitle = applyProgramTitle(EMPTY_TAB_TITLE, "zsh in termini", 1_000);
    const next = applyProcessChange(withTitle, "vim", 1_000 + TITLE_GRACE_MS + 1);
    expect(resolveTabTitle(next, "zsh")).toBe("[zsh] vim");
  });

  it("keeps a title set just before the change was noticed", () => {
    const withTitle = applyProgramTitle(EMPTY_TAB_TITLE, "✳ Claude Code", 5_000);
    const next = applyProcessChange(withTitle, "claude", 5_800);
    expect(resolveTabTitle(next, "zsh")).toBe("[zsh] ✳ Claude Code");
  });

  it("falls back to the shell title once back at the prompt", () => {
    const running = applyProcessChange(EMPTY_TAB_TITLE, "vim", 1_000);
    const back = applyProcessChange(running, null, 60_000);
    expect(resolveTabTitle(back, "zsh")).toBe("zsh");
  });

  it("prefixes the console type without its default marker", () => {
    const running = applyProcessChange(EMPTY_TAB_TITLE, "node", 1_000);
    expect(resolveTabTitle(running, "zsh (default)")).toBe("[zsh] node");
    expect(resolveTabTitle(running, "PowerShell 7")).toBe("[PowerShell 7] node");
    expect(resolveTabTitle(undefined, "zsh (default)")).toBe("zsh (default)");
  });

  it("names the agent instead of the shell while one is running", () => {
    const running = applyProgramTitle(EMPTY_TAB_TITLE, "✳ fixing the parser", 1_000);
    expect(resolveTabTitle(running, "Claude Code", ["claude", "Claude Code"])).toBe("[Claude Code] ✳ fixing the parser");
  });

  it("doesn't repeat the agent's name when that's all the program reports", () => {
    const running = applyProcessChange(EMPTY_TAB_TITLE, "codex", 1_000);
    expect(resolveTabTitle(running, "Codex", ["codex", "Codex"])).toBe("Codex");
  });

  it("is a no-op when the process didn't change", () => {
    const running = applyProcessChange(EMPTY_TAB_TITLE, "vim", 1_000);
    expect(applyProcessChange(running, "vim", 9_000)).toBe(running);
  });
});
