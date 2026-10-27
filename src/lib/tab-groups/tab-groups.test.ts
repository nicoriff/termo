import { describe, expect, it } from "vitest";
import { gatherGroups, insertGroup, insertTab, joinedGroupAt, nextColor, stripItems } from "./tab-groups";

const tab = (id: string, groupId?: string) => (groupId ? { id, groupId } : { id });

describe("nextColor", () => {
  it("picks the first color no group uses", () => {
    expect(nextColor([])).toBe("grey");
    expect(nextColor([{ color: "grey" }, { color: "red" }])).toBe("blue");
  });

  it("once all are taken, the one used least", () => {
    const all = ["grey", "blue", "red", "yellow", "green", "pink", "purple", "cyan", "orange"] as const;
    const groups = [...all, "grey", "blue"].map((color) => ({ color }));
    expect(nextColor(groups as never)).toBe("red");
  });
});

describe("gatherGroups", () => {
  it("brings a group's tabs together where its first one is", () => {
    const sessions = [tab("a", "g"), tab("b"), tab("c", "g"), tab("d")];
    expect(gatherGroups(sessions).map((s) => s.id)).toEqual(["a", "c", "b", "d"]);
  });

  it("returns the same array when nothing moves", () => {
    const sessions = [tab("a", "g"), tab("b", "g"), tab("c")];
    expect(gatherGroups(sessions)).toBe(sessions);
  });
});

describe("joinedGroupAt", () => {
  const sessions = [tab("a", "g"), tab("b", "g"), tab("c"), tab("d", "h")];

  it("is the group on both sides of the drop", () => {
    expect(joinedGroupAt(sessions, 1)).toBe("g");
  });

  it("is nothing at a group's edge or at either end", () => {
    expect(joinedGroupAt(sessions, 0)).toBeUndefined();
    expect(joinedGroupAt(sessions, 2)).toBeUndefined();
    expect(joinedGroupAt(sessions, 3)).toBeUndefined();
    expect(joinedGroupAt(sessions, 4)).toBeUndefined();
  });
});

describe("stripItems", () => {
  const group = { id: "g", name: "API", color: "blue" as const, collapsed: false };
  const sessions = [tab("a", "g"), tab("b", "g"), tab("c")];

  it("puts the chip ahead of its tabs", () => {
    const items = stripItems(sessions, [group], "c");
    expect(items.map((item) => (item.kind === "chip" ? `chip:${item.count}` : item.session.id))).toEqual([
      "chip:2",
      "a",
      "b",
      "c",
    ]);
  });

  it("folds a collapsed group's tabs, except the active one", () => {
    const folded = { ...group, collapsed: true };
    const ids = (activeId: string) =>
      stripItems(sessions, [folded], activeId).map((item) => (item.kind === "chip" ? "chip" : item.session.id));
    expect(ids("c")).toEqual(["chip", "c"]);
    expect(ids("b")).toEqual(["chip", "b", "c"]);
  });

  it("shows a tab whose group is gone as ungrouped", () => {
    expect(stripItems(sessions, [], "c").every((item) => item.kind === "tab")).toBe(true);
  });
});

describe("insertTab", () => {
  const strip = [tab("a", "g"), tab("b", "g"), tab("c"), tab("d")];
  const order = (list: { id: string; groupId?: string }[]) => list.map((s) => `${s.id}${s.groupId ? `:${s.groupId}` : ""}`);

  it("joins the group it's dropped inside of", () => {
    expect(order(insertTab(strip, tab("x"), "b", true))).toEqual(["a:g", "x:g", "b:g", "c", "d"]);
  });

  it("leaves its group when dropped clear of it on the strip", () => {
    const without = [tab("a", "g"), tab("c"), tab("d")];
    expect(order(insertTab(without, tab("b", "g"), "d", true))).toEqual(["a:g", "c", "b", "d"]);
  });

  it("stays in its group right at the group's edge", () => {
    const without = [tab("a", "g"), tab("c"), tab("d")];
    expect(order(insertTab(without, tab("b", "g"), "c", true))).toEqual(["a:g", "b:g", "c", "d"]);
  });

  it("keeps its group when dropped on the tile as a whole", () => {
    expect(order(insertTab([tab("a", "g"), tab("c")], tab("b", "g"), undefined, false))).toEqual(["a:g", "b:g", "c"]);
  });
});

describe("insertGroup", () => {
  it("goes past a group rather than inside it", () => {
    const strip = [tab("a", "g"), tab("b", "g"), tab("c")];
    const moved = [tab("x", "h"), tab("y", "h")];
    expect(insertGroup(strip, moved, "b").map((s) => s.id)).toEqual(["a", "b", "x", "y", "c"]);
    expect(insertGroup(strip, moved, "a").map((s) => s.id)).toEqual(["x", "y", "a", "b", "c"]);
    expect(insertGroup(strip, moved, undefined).map((s) => s.id)).toEqual(["a", "b", "c", "x", "y"]);
  });
});

describe("insertTab joining the group it was dropped on", () => {
  it("joins even at the group's edge, where the strip alone wouldn't say", () => {
    const strip = [tab("a", "g"), tab("b", "g"), tab("c")];
    const placed = insertTab(strip, tab("x"), "c", true, "g");
    expect(placed.map((s) => `${s.id}${s.groupId ? ":" + s.groupId : ""}`)).toEqual(["a:g", "b:g", "x:g", "c"]);
  });
});
