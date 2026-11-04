import { describe, expect, it } from "vitest";
import {
  EMPTY_LAYOUT,
  closePanel,
  dropSideAt,
  expand,
  moveIntoGroup,
  openAsGroup,
  openInAccordion,
  resizeGroups,
  toggleExpanded,
  sanitizeLayout,
  type SideLayout,
} from "./side-panel-layout";

const two: SideLayout = {
  groups: [
    { panels: ["explorer", "git"], expanded: "git", size: 0.6 },
    { panels: ["harness"], expanded: "harness", size: 0.4 },
  ],
  active: 0,
};

describe("openInAccordion", () => {
  it("makes the first section the column's only group", () => {
    expect(openInAccordion(EMPTY_LAYOUT, "git")).toEqual({
      groups: [{ panels: ["git"], expanded: "git", size: 1 }],
      active: 0,
    });
  });

  it("adds a new section to the topmost group, unfolded, whichever was in use", () => {
    const layout = openInAccordion({ ...two, active: 1 }, "kanban");
    expect(layout.groups[0]).toEqual({ panels: ["explorer", "git", "kanban"], expanded: "kanban", size: 0.6 });
    expect(layout.groups[1].panels).toEqual(["harness"]);
    expect(layout.active).toBe(0);
  });

  it("unfolds one already there, and its group becomes the active one", () => {
    const layout = openInAccordion({ ...two, active: 1 }, "explorer");
    expect(layout.groups[0].expanded).toBe("explorer");
    expect(layout.active).toBe(0);
  });
});

describe("closePanel", () => {
  it("unfolds the neighbor when the one showing goes", () => {
    expect(closePanel(two, "git").groups[0]).toEqual({ panels: ["explorer"], expanded: "explorer", size: 0.6 });
  });

  it("drops an empty group and gives its height to the rest", () => {
    const layout = closePanel({ ...two, active: 1 }, "harness");
    expect(layout.groups).toEqual([{ panels: ["explorer", "git"], expanded: "git", size: 1 }]);
    expect(layout.active).toBe(0);
  });

  it("closing the last section leaves the column empty", () => {
    expect(closePanel(openInAccordion(EMPTY_LAYOUT, "git"), "git").groups).toEqual([]);
  });
});

describe("openAsGroup", () => {
  it("splits a section off into a group of its own, below", () => {
    const layout = openAsGroup(two, "explorer", 0, "below");
    expect(layout.groups.map((g) => g.panels)).toEqual([["git"], ["explorer"], ["harness"]]);
    expect(layout.groups[0].size).toBeCloseTo(0.3);
    expect(layout.groups[1].size).toBeCloseTo(0.3);
    expect(layout.active).toBe(1);
  });

  it("opens a section not yet in the column above a group", () => {
    const layout = openAsGroup(two, "kanban", 1, "above");
    expect(layout.groups.map((g) => g.panels)).toEqual([["explorer", "git"], ["kanban"], ["harness"]]);
  });

  it("moves a lone section's group next to another", () => {
    const layout = openAsGroup(two, "harness", 0, "above");
    expect(layout.groups.map((g) => g.panels)).toEqual([["harness"], ["explorer", "git"]]);
    expect(layout.groups.reduce((sum, g) => sum + g.size, 0)).toBeCloseTo(1);
  });

  it("does nothing dropping a group's only section on itself", () => {
    expect(openAsGroup(two, "harness", 1, "above")).toBe(two);
  });
});

describe("moveIntoGroup", () => {
  it("reorders within its group", () => {
    expect(moveIntoGroup(two, "git", 0, 0).groups[0].panels).toEqual(["git", "explorer"]);
    expect(moveIntoGroup(two, "explorer", 0, 2).groups[0].panels).toEqual(["git", "explorer"]);
  });

  it("moves to another group's accordion, unfolded there", () => {
    const layout = moveIntoGroup(two, "harness", 0, 1);
    expect(layout.groups).toEqual([{ panels: ["explorer", "harness", "git"], expanded: "harness", size: 1 }]);
  });
});

describe("expand and resize", () => {
  it("unfolds a folded section", () => {
    expect(expand(two, 0, "explorer").groups[0].expanded).toBe("explorer");
  });

  it("keeps each group at its minimum", () => {
    const layout = resizeGroups(two, 0, 0.5, 0.2);
    expect(layout.groups[0].size).toBeCloseTo(0.8);
    expect(layout.groups[1].size).toBeCloseTo(0.2);
  });
});

describe("dropSideAt", () => {
  it("opens a group at the edges, joins the accordion in between", () => {
    expect(dropSideAt(0.1)).toBe("above");
    expect(dropSideAt(0.9)).toBe("below");
    expect(dropSideAt(0.5)).toBeNull();
  });
});

describe("sanitizeLayout", () => {
  it("drops what doesn't make sense and keeps the rest", () => {
    const layout = sanitizeLayout({
      groups: [
        { panels: ["git", "nope", "git"], expanded: "nope", size: 3 },
        { panels: [], expanded: "x", size: 1 },
        { panels: ["git", "kanban"], expanded: "kanban", size: 1 },
      ],
      active: 9,
    });
    expect(layout).toEqual({
      groups: [
        { panels: ["git"], expanded: "git", size: 0.75 },
        { panels: ["kanban"], expanded: "kanban", size: 0.25 },
      ],
      active: 1,
    });
  });

  it("is empty for anything that isn't a layout", () => {
    expect(sanitizeLayout(null)).toEqual(EMPTY_LAYOUT);
    expect(sanitizeLayout({ groups: "x" })).toEqual(EMPTY_LAYOUT);
  });
});

describe("toggleExpanded", () => {
  it("folds the unfolded section, and unfolds it again", () => {
    const folded = toggleExpanded(two, 0, "git");
    expect(folded.groups[0].expanded).toBeNull();
    expect(toggleExpanded(folded, 0, "git").groups[0].expanded).toBe("git");
  });

  it("unfolds another section of the group", () => {
    expect(toggleExpanded(two, 0, "explorer").groups[0].expanded).toBe("explorer");
  });

  it("keeps a group all folded through storage", () => {
    const folded = toggleExpanded(two, 0, "git");
    expect(sanitizeLayout(JSON.parse(JSON.stringify(folded))).groups[0].expanded).toBeNull();
  });
});
