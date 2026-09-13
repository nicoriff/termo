import { describe, expect, it } from "vitest";
import { computeDividers, makeSession, makeTile, type SplitNode } from "./split-tree";

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
