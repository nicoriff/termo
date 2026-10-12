import { describe, expect, it } from "vitest";
import { changesLabel, countChanges, diffNames, groupChanges, markIn, parseNames, splitPath, syncLabel, type ChangedFile } from "./git-summary";

const file = (path: string, staged: ChangedFile["staged"], unstaged: ChangedFile["unstaged"], conflict = false): ChangedFile => ({
  path,
  staged,
  unstaged,
  conflict,
});

const files = [
  file("a.rs", "modified", null),
  file("b.rs", null, "modified"),
  file("both.rs", "added", "modified"),
  file("new.md", null, "untracked"),
  file("clash.rs", null, "conflict", true),
  file("gone.rs", null, "deleted"),
];

describe("countChanges", () => {
  it("counts each kind, and a file staged and changed again in both", () => {
    expect(countChanges(files)).toEqual({ staged: 2, changed: 3, untracked: 1, conflicts: 1, total: 6 });
  });

  it("is all zeros on a clean tree", () => {
    expect(countChanges([])).toEqual({ staged: 0, changed: 0, untracked: 0, conflicts: 0, total: 0 });
  });
});

describe("groupChanges", () => {
  it("puts each file in the sections it belongs to", () => {
    const groups = groupChanges(files);
    expect(groups.conflicts.map((f) => f.path)).toEqual(["clash.rs"]);
    expect(groups.staged.map((f) => f.path)).toEqual(["a.rs", "both.rs"]);
    expect(groups.changes.map((f) => f.path)).toEqual(["b.rs", "both.rs", "gone.rs"]);
    expect(groups.untracked.map((f) => f.path)).toEqual(["new.md"]);
  });

  it("shows in each section the mark that section is about", () => {
    const both = files[2];
    expect(markIn(both, "staged")).toBe("added");
    expect(markIn(both, "changes")).toBe("modified");
    expect(markIn(files[4], "conflicts")).toBe("conflict");
  });
});

describe("the status bar's labels", () => {
  it("lists only what there is", () => {
    expect(changesLabel(countChanges(files))).toBe("+2 ~3 ?1 !1");
    expect(changesLabel(countChanges([file("a", null, "modified")]))).toBe("~1");
    expect(changesLabel(countChanges([]))).toBe("");
  });

  it("shows what's to push and to pull", () => {
    expect(syncLabel(2, 1)).toBe("↑2 ↓1");
    expect(syncLabel(0, 3)).toBe("↓3");
    expect(syncLabel(0, 0)).toBe("");
  });
});

describe("splitPath", () => {
  it("sets the folder apart from the name", () => {
    expect(splitPath("src/lib/a.ts")).toEqual({ name: "a.ts", folder: "src/lib" });
    expect(splitPath("README.md")).toEqual({ name: "README.md", folder: "" });
  });
});

describe("names in a box", () => {
  it("reads them without blanks, repeats or the @", () => {
    expect(parseNames(" bug, @ana ,, bug\nfeature ")).toEqual(["bug", "ana", "feature"]);
    expect(parseNames("")).toEqual([]);
  });

  it("tells what was added from what was taken away", () => {
    expect(diffNames(["a", "b"], ["b", "c"])).toEqual({ add: ["c"], remove: ["a"] });
    expect(diffNames(["a"], ["a"])).toEqual({ add: [], remove: [] });
  });
});
