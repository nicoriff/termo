import { describe, expect, it } from "vitest";
import { commitSides, emptyGraphState, layoutGraph, type GraphInput } from "./git-graph";

const c = (sha: string, ...parents: string[]): GraphInput => ({ sha, parents });

/** Each row as `lane: top lanes > bottom lanes | through lanes`. */
function shape(commits: GraphInput[]) {
  return layoutGraph(commits).rows.map(
    (row) =>
      `${row.commit.sha}@${row.lane} ${row.top.map((l) => l.lane).join(",")}>${row.bottom.map((l) => l.lane).join(",")}|${row.through
        .map((l) => l.lane)
        .join(",")}`,
  );
}

describe("layoutGraph", () => {
  it("keeps a straight history in one column and one color", () => {
    const { rows } = layoutGraph([c("c", "b"), c("b", "a"), c("a")]);
    expect(rows.map((row) => row.lane)).toEqual([0, 0, 0]);
    expect(new Set(rows.map((row) => row.color)).size).toBe(1);
    expect(rows.map((row) => row.width)).toEqual([1, 1, 1]);
    // The tip has nothing above it, the root nothing below.
    expect(rows[0].top).toEqual([]);
    expect(rows[2].bottom).toEqual([]);
  });

  it("gives a second branch its own column until they meet at the fork", () => {
    // main: m2 - m1 - base; topic: t1 - base
    expect(shape([c("m2", "m1"), c("t1", "base"), c("m1", "base"), c("base")])).toEqual([
      "m2@0 >0|",
      "t1@1 >1|0",
      "m1@0 0>0|1",
      "base@0 0,1>|",
    ]);
  });

  it("draws a merge as two lines leaving the dot", () => {
    // merge of topic (t) into main (m), both from base
    const rows = layoutGraph([c("merge", "m", "t"), c("m", "base"), c("t", "base"), c("base")]).rows;
    expect(rows[0].bottom.map((link) => link.lane)).toEqual([0, 1]);
    expect(rows[0].bottom[0].color).not.toBe(rows[0].bottom[1].color);
    expect(rows[1].through.map((link) => link.lane)).toEqual([1]);
    expect(rows[2].lane).toBe(1);
    expect(rows[3].top.map((link) => link.lane)).toEqual([0, 1]);
  });

  it("joins a merged-in parent to the column already heading there", () => {
    // b's line is already going to a when the merge names a as second parent.
    const rows = layoutGraph([c("b", "a"), c("m", "x", "a"), c("x", "a"), c("a")]).rows;
    expect(rows[1].lane).toBe(1);
    expect(rows[1].bottom.map((link) => link.lane)).toEqual([1, 0]);
    expect(rows[3].top.map((link) => link.lane)).toEqual([0, 1]);
  });

  it("handles a merge of three", () => {
    const rows = layoutGraph([c("o", "a", "b", "d"), c("a", "r"), c("b", "r"), c("d", "r"), c("r")]).rows;
    expect(rows[0].bottom.map((link) => link.lane)).toEqual([0, 1, 2]);
    expect(rows[0].width).toBe(3);
    expect(rows[4].top.map((link) => link.lane)).toEqual([0, 1, 2]);
  });

  it("starts histories that share nothing in columns of their own", () => {
    const rows = layoutGraph([c("a2", "a1"), c("b1"), c("a1")]).rows;
    expect(rows.map((row) => row.lane)).toEqual([0, 1, 0]);
    expect(rows[1].top).toEqual([]);
    expect(rows[1].bottom).toEqual([]);
    expect(rows[1].color).not.toBe(rows[0].color);
  });

  it("reuses a column once its branch has ended", () => {
    // t ends at base; a later tip (u) takes the freed column 1.
    const rows = layoutGraph([c("m2", "m1"), c("t", "m1"), c("m1", "m0"), c("u", "m0"), c("m0")]).rows;
    expect(rows[1].lane).toBe(1);
    expect(rows[3].lane).toBe(1);
    expect(rows[3].color).not.toBe(rows[1].color);
  });

  it("goes on across pages exactly as if it had been one", () => {
    const all = [c("m", "a", "b"), c("a", "r"), c("b", "r"), c("r", "q"), c("q")];
    const whole = layoutGraph(all).rows;
    const first = layoutGraph(all.slice(0, 2));
    const second = layoutGraph(all.slice(2), first.state);
    expect([...first.rows, ...second.rows]).toEqual(whole);
  });

  it("leaves the state it was given untouched", () => {
    const start = emptyGraphState();
    layoutGraph([c("a", "b")], start);
    expect(start).toEqual(emptyGraphState());
  });
});

describe("commitSides", () => {
  // local: l2 - l1 - base;  upstream: u1 - base
  const commits = [c("l2", "l1"), c("u1", "base"), c("l1", "base"), c("base", "old"), c("old")];

  it("tells what's still to push from what's still to pull", () => {
    const sides = commitSides(commits, "l2", "u1");
    expect(sides.get("l2")).toBe("ahead");
    expect(sides.get("l1")).toBe("ahead");
    expect(sides.get("u1")).toBe("behind");
    expect(sides.get("base")).toBe("shared");
    expect(sides.get("old")).toBe("shared");
  });

  it("marks nothing without an upstream", () => {
    expect(commitSides(commits, "l2", null).size).toBe(0);
    expect(commitSides(commits, null, "u1").size).toBe(0);
  });

  it("stops at what isn't loaded instead of failing", () => {
    const sides = commitSides([c("a", "missing")], "a", "missing");
    expect(sides.get("a")).toBe("ahead");
  });
});
