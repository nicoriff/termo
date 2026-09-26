import { describe, expect, it } from "vitest";
import { computeDividers, dropZoneAt, makeSession, makeTile, splitLeaf, type SplitNode } from "./split-tree";

describe("computeDividers", () => {
  it("keeps a nested divider inside its own split region", () => {
    // Two tiles side by side on top, one full-width tile below.
    const root: SplitNode = {
      kind: "split",
      id: "outer",
      direction: "col",
      ratio: 0.5,
      children: [
        {
          kind: "split",
          id: "top",
          direction: "row",
          ratio: 0.5,
          children: [makeTile(makeSession()), makeTile(makeSession())],
        },
        makeTile(makeSession()),
      ],
    };

    const dividers = new Map(computeDividers(root).map((d) => [d.id, d]));

    // The vertical divider between the two top tiles must stop where the
    // top half ends, not run down through the bottom tile.
    const top = dividers.get("top")!;
    expect(top.regionTop).toBe(0);
    expect(top.regionHeight).toBe(0.5);
    expect(top.left).toBe(0.5);

    const outer = dividers.get("outer")!;
    expect(outer.regionLeft).toBe(0);
    expect(outer.regionWidth).toBe(1);
    expect(outer.top).toBe(0.5);
  });
});

describe("splitLeaf", () => {
  it("puts the new tile after the target by default, or before it with newFirst", () => {
    const target = makeTile(makeSession());
    const added = makeTile(makeSession());
    const after = splitLeaf(target, target.id, "row", added, "s1");
    const before = splitLeaf(target, target.id, "row", added, "s2", true);
    expect(after.kind === "split" && after.children.map((child) => child.id)).toEqual([target.id, added.id]);
    expect(before.kind === "split" && before.children.map((child) => child.id)).toEqual([added.id, target.id]);
  });
});

describe("dropZoneAt", () => {
  it("splits toward a side near its edge, and drops in the middle otherwise", () => {
    expect(dropZoneAt(0.1, 0.5)).toBe("left");
    expect(dropZoneAt(0.9, 0.5)).toBe("right");
    expect(dropZoneAt(0.5, 0.05)).toBe("top");
    expect(dropZoneAt(0.5, 0.95)).toBe("bottom");
    expect(dropZoneAt(0.5, 0.5)).toBe("center");
    expect(dropZoneAt(0.3, 0.7)).toBe("center");
  });

  it("lets the nearest side win in a corner", () => {
    expect(dropZoneAt(0.05, 0.2)).toBe("left");
    expect(dropZoneAt(0.2, 0.05)).toBe("top");
  });
});
