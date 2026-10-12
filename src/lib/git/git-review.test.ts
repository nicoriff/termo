import { describe, expect, it } from "vitest";
import {
  asPaste,
  blockingFindings,
  countFindings,
  filesWithFindings,
  findingPlace,
  fixRequest,
  groupByFile,
  openFindings,
  reviewOf,
  reviewsOf,
  scopeKey,
  shownFindings,
  splitFences,
  type Finding,
  type ReviewLike,
  type Scope,
  type Severity,
} from "./git-review";

let next = 0;
function finding(path: string, line: number, severity: Severity, change: Partial<Finding> = {}): Finding {
  return { id: `f${next++}`, path, line, endLine: line, severity, title: "T", body: "B", suggestion: "", dismissed: false, stale: false, ...change };
}

function review(branch: string, scope: Scope, createdAt: number, findings: Finding[] = []): ReviewLike {
  return { id: `r${next++}`, branch, scope, head: "abc", createdAt, findings };
}

describe("what's shown of a review", () => {
  const findings = [
    finding("a.ts", 1, "suggestion"),
    finding("a.ts", 2, "serious"),
    finding("b.ts", 3, "warning", { dismissed: true }),
    finding("c.ts", 4, "warning"),
  ];

  it("leaves out what's less serious than asked for, and what was dismissed", () => {
    expect(shownFindings(findings, "suggestion", false).map((f) => f.line)).toEqual([1, 2, 4]);
    expect(shownFindings(findings, "warning", false).map((f) => f.line)).toEqual([2, 4]);
    expect(shownFindings(findings, "serious", true).map((f) => f.line)).toEqual([2]);
    expect(shownFindings(findings, "warning", true).map((f) => f.line)).toEqual([2, 3, 4]);
  });

  it("counts the open ones by severity, apart from the dismissed", () => {
    const counts = countFindings([...findings, finding("d.ts", 5, "serious", { stale: true })]);
    expect(counts).toEqual({ serious: 2, warning: 1, suggestion: 1, open: 4, dismissed: 1, stale: 1 });
  });

  it("groups by file, the files with the most serious first and each by line", () => {
    const groups = groupByFile([finding("z.ts", 9, "warning"), finding("z.ts", 2, "suggestion"), finding("m.ts", 1, "serious"), finding("a.ts", 1, "warning")]);
    expect(groups.map((group) => group.path)).toEqual(["m.ts", "a.ts", "z.ts"]);
    expect(groups[2].findings.map((f) => f.line)).toEqual([2, 9]);
  });

  it("says where a finding is", () => {
    expect(findingPlace({ path: "a.ts", line: 12, endLine: 12 })).toBe("a.ts:12");
    expect(findingPlace({ path: "a.ts", line: 12, endLine: 18 })).toBe("a.ts:12-18");
    expect(findingPlace({ path: "a.ts", line: 0, endLine: 0 })).toBe("a.ts");
  });
});

describe("what holds a push up", () => {
  it("is serious, still open, and about the file as it is", () => {
    const held = blockingFindings([
      finding("a.ts", 1, "serious"),
      finding("a.ts", 2, "serious", { dismissed: true }),
      finding("a.ts", 3, "serious", { stale: true }),
      finding("a.ts", 4, "warning"),
    ]);
    expect(held.map((f) => f.line)).toEqual([1]);
  });
});

describe("a branch's reviews", () => {
  const reviews = [
    review("topic", { kind: "changes" }, 10),
    review("topic", { kind: "branch" }, 30),
    review("main", { kind: "branch" }, 40),
    review("topic", { kind: "commit", sha: "abc" }, 20),
    review("", { kind: "changes" }, 5),
  ];

  it("are its own, newest first", () => {
    expect(reviewsOf(reviews, "topic").map((r) => r.createdAt)).toEqual([30, 20, 10]);
    // A detached HEAD has the ones made without a branch.
    expect(reviewsOf(reviews, null).map((r) => r.createdAt)).toEqual([5]);
  });

  it("are told apart by what they looked at", () => {
    expect(scopeKey({ kind: "commit", sha: "abc" })).toBe("commit:abc");
    expect(reviewOf(reviews, "topic", { kind: "branch" })?.createdAt).toBe(30);
    expect(reviewOf(reviews, "topic", { kind: "commit", sha: "abc" })?.createdAt).toBe(20);
    expect(reviewOf(reviews, "topic", { kind: "commit", sha: "def" })).toBeNull();
    expect(reviewOf(reviews, "main", { kind: "changes" })).toBeNull();
  });

  it("mark the files of the working tree with what's still open", () => {
    const marks = filesWithFindings(
      [
        review("topic", { kind: "changes" }, 1, [finding("a.ts", 1, "suggestion"), finding("b.ts", 1, "warning", { dismissed: true })]),
        review("topic", { kind: "branch" }, 2, [finding("a.ts", 5, "serious"), finding("c.ts", 1, "warning", { stale: true })]),
        // A commit's findings are of the commit, not of what's on disk.
        review("topic", { kind: "commit", sha: "abc" }, 3, [finding("d.ts", 1, "serious")]),
        review("main", { kind: "changes" }, 4, [finding("e.ts", 1, "serious")]),
      ],
      "topic",
    );
    expect([...marks.entries()]).toEqual([["a.ts", "serious"]]);
  });
});

describe("an agent's text", () => {
  it("has its fenced code set apart from the prose", () => {
    const parts = splitFences("Do this:\n\n```rust\nif x {\n    y();\n}\n```\n\nOr leave it.");
    expect(parts).toEqual([
      { code: false, text: "Do this:" },
      { code: true, text: "if x {\n    y();\n}" },
      { code: false, text: "Or leave it." },
    ]);
  });

  it("is one stretch of prose without a fence, and a fence left open runs to the end", () => {
    expect(splitFences("Just `this`.")).toEqual([{ code: false, text: "Just `this`." }]);
    expect(splitFences("See:\r\n```\r\na\r\nb")).toEqual([
      { code: false, text: "See:" },
      { code: true, text: "a\nb" },
    ]);
    expect(splitFences("  \n")).toEqual([]);
  });
});

describe("how much is still open", () => {
  it("counts what wasn't dismissed and is about the file as it is", () => {
    const reviews = [
      review("topic", { kind: "changes" }, 1, [finding("a.ts", 1, "serious"), finding("a.ts", 2, "warning", { dismissed: true })]),
      review("topic", { kind: "commit", sha: "abc" }, 2, [finding("b.ts", 1, "suggestion"), finding("b.ts", 2, "serious", { stale: true })]),
    ];
    expect(openFindings(reviews)).toBe(2);
    expect(openFindings([])).toBe(0);
  });
});

describe("handing a finding to a console's agent", () => {
  it("puts the request first and the finding under it", () => {
    const text = fixRequest({ path: "a.ts", line: 3, endLine: 5, title: "It breaks", body: " Because. ", suggestion: "Do this" }, "Fix this:");
    expect(text).toBe("Fix this:\n\na.ts:3-5 — It breaks\n\nBecause.\n\nDo this");
    expect(fixRequest({ path: "a.ts", line: 0, endLine: 0, title: "X", body: "Y", suggestion: " " }, "Fix:")).toBe("Fix:\n\na.ts — X\n\nY");
  });

  it("goes in as one paste that nothing in it can end early", () => {
    expect(asPaste("one\r\ntwo\rthree\tx")).toBe("\x1b[200~one\ntwo\nthree\tx\x1b[201~");
    const inside = (text: string) => asPaste(text).slice("\x1b[200~".length, -"\x1b[201~".length);
    // No escape is left to end the paste with, however the end mark is hidden.
    for (const hostile of ["a\x1b[201~rm -rf /\r", "a\x1b[20\x1b[201~1~rm -rf /\r", "a\u009b201~x", "a\x03\x04\x1a\x7f\x00b"]) {
      expect(inside(hostile)).not.toMatch(/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/);
    }
    expect(inside("a\x1b[20\x1b[201~1~rm -rf /\r")).toBe("a[20[201~1~rm -rf /\n");
  });
});
