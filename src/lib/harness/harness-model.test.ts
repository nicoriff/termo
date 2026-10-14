import { describe, expect, it } from "vitest";
import type { ExternalEntry, HarnessSpec, SuggestedRule } from "$lib/core/harness-client";
import {
  addRule,
  groupExternal,
  changeKind,
  commandProblem,
  harnessStatus,
  normalizePattern,
  normalizePrefix,
  pathProblem,
  relativeToRoot,
  ruleKinds,
  sameSpec,
} from "./harness-model";

function spec(overrides: Partial<HarnessSpec> = {}): HarnessSpec {
  return {
    version: 1,
    agents: ["claude", "codex"],
    paths: [],
    commands: [],
    checks: [],
    instructions: "",
    ...overrides,
  };
}

describe("normalizing", () => {
  it("writes paths the way the backend does", () => {
    expect(normalizePattern(" ./src\\gen/ ")).toBe("src/gen");
    expect(normalizePattern(".env")).toBe(".env");
  });

  it("collapses a command's spaces", () => {
    expect(normalizePrefix("  git   push  --force ")).toBe("git push --force");
  });
});

describe("sameSpec", () => {
  it("ignores what doesn't change what's written", () => {
    const a = spec({ paths: [{ pattern: "./gen/", access: "noEdit" }], instructions: "Hi\n" });
    const b = spec({
      agents: ["codex", "claude"],
      paths: [{ pattern: "gen", access: "noEdit" }],
      instructions: "Hi",
      managed: { "x.json": { a: [1] } },
    });
    expect(sameSpec(a, b)).toBe(true);
  });

  it("sees a rule that changed", () => {
    expect(sameSpec(spec(), spec({ commands: [{ prefix: "git push", decision: "ask" }] }))).toBe(false);
  });
});

describe("pathProblem", () => {
  it("takes a path inside the repository", () => {
    expect(pathProblem("src/**", [])).toBeNull();
  });

  it("refuses one outside, a duplicate and parentheses", () => {
    expect(pathProblem("../x", [])).toBe("harness.paths.outside");
    expect(pathProblem("C:\\x", [])).toBe("harness.paths.outside");
    expect(pathProblem("./.env", [".env"])).toBe("harness.paths.duplicate");
    expect(pathProblem("a(b)", [])).toBe("harness.paths.parentheses");
    expect(pathProblem("  ", [])).toBe("harness.paths.empty");
  });
});

describe("commandProblem", () => {
  it("refuses a duplicate written differently", () => {
    expect(commandProblem("git  push", ["git push"])).toBe("harness.commands.duplicate");
    expect(commandProblem("git push", [])).toBeNull();
  });
});

describe("relativeToRoot", () => {
  it("makes a picked file relative to the repository", () => {
    expect(relativeToRoot("C:\\Repo", "c:\\repo\\src\\a.ts")).toBe("src/a.ts");
    expect(relativeToRoot("/home/me/repo", "/home/me/repo/.env")).toBe(".env");
  });

  it("refuses one outside it", () => {
    expect(relativeToRoot("/home/me/repo", "/home/me/repository/x")).toBeNull();
    expect(relativeToRoot("/home/me/repo", "/etc/passwd")).toBeNull();
  });
});

describe("external rules", () => {
  const entry = (value: string, rule?: SuggestedRule): ExternalEntry => ({
    file: ".claude/settings.json",
    key: "permissions.ask",
    value,
    rule,
  });

  it("groups the entries of one rule", () => {
    const push: SuggestedRule = { kind: "command", prefix: "git push", decision: "ask" };
    const groups = groupExternal([entry("Bash(git push *)", push), entry("PowerShell(git push)", push), entry("WebFetch")]);
    expect(groups.map((group) => group.entries.length)).toEqual([2, 1]);
    expect(groups[1].rule).toBeNull();
  });

  it("adopting changes a rule the harness already has", () => {
    const target = spec({
      paths: [{ pattern: ".env", access: "noEdit" }],
      commands: [{ prefix: "git push", decision: "deny" }],
    });
    addRule(target, { kind: "path", pattern: "./.env", access: "noRead" });
    addRule(target, { kind: "command", prefix: "git  push", decision: "ask" });
    addRule(target, { kind: "command", prefix: "rm -rf", decision: "deny" });
    expect(target.paths).toEqual([{ pattern: ".env", access: "noRead" }]);
    expect(target.commands).toEqual([
      { prefix: "git push", decision: "ask" },
      { prefix: "rm -rf", decision: "deny" },
    ]);
  });
});

describe("ruleKinds", () => {
  it("lists the kinds present, in order", () => {
    const kinds = ruleKinds(
      spec({
        commands: [{ prefix: "git push", decision: "ask" }],
        paths: [{ pattern: ".env", access: "noRead" }],
        checks: [{ command: "  " }],
      }),
    );
    expect(kinds).toEqual(["noRead", "ask"]);
  });
});

describe("status", () => {
  it("puts unapplied edits first", () => {
    expect(harnessStatus(true, true, 2)).toBe("draft");
    expect(harnessStatus(true, false, 2)).toBe("pending");
    expect(harnessStatus(true, false, 0)).toBe("synced");
    expect(harnessStatus(false, false, 0)).toBe("none");
  });

  it("names a change by what happens to the file", () => {
    expect(changeKind({ path: "a", before: null, after: "x" })).toBe("create");
    expect(changeKind({ path: "a", before: "x", after: null })).toBe("delete");
    expect(changeKind({ path: "a", before: "x", after: "y" })).toBe("update");
  });
});
