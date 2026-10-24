import { describe, expect, it } from "vitest";

import {
  DEFAULT_LAYOUT,
  defaultLayout,
  normalizeLayout,
  optionOn,
  optionValue,
  placeItem,
  removeItem,
  sameLayout,
  setItemOption,
  STATUS_ITEMS,
} from "./layout";

describe("defaultLayout", () => {
  it("is the bar as it was, for a user who never touched git's settings", () => {
    expect(defaultLayout()).toEqual(DEFAULT_LAYOUT);
    expect(defaultLayout({ visible: true, parts: { branch: true, sync: true, changes: true, pr: true } })).toEqual(DEFAULT_LAYOUT);
  });

  it("keeps the repository's item hidden when the Git section had it off", () => {
    const layout = defaultLayout({ visible: false, parts: { branch: true, sync: true, changes: true, pr: true } });
    expect(layout.right.map((ref) => ref.id)).toEqual(["system.daemon", "system.remote", "console.memory"]);
  });

  it("carries over the parts that were turned off", () => {
    const layout = defaultLayout({ visible: true, parts: { branch: true, sync: false, changes: true, pr: false } });
    const git = layout.right.find((ref) => ref.id === "git.repo")!;
    expect(git.options).toEqual({ sync: "off", pr: "off" });
    expect(optionOn(git, "branch")).toBe(true);
    expect(optionOn(git, "sync")).toBe(false);
  });
});

describe("normalizeLayout", () => {
  it("drops what it doesn't know and what's there twice", () => {
    const layout = normalizeLayout({
      left: [{ id: "agent.model" }, { id: "gone.item" }, { id: "agent.model" }, null, "text"],
      right: [{ id: "agent.model" }, { id: "system.clock" }],
    });
    expect(layout).toEqual({ left: [{ id: "agent.model" }], right: [{ id: "system.clock" }] });
  });

  it("keeps only options the item has, with values they can take", () => {
    const layout = normalizeLayout({
      left: [
        { id: "path.cwd", options: { format: "folder", color: "red" } },
        { id: "agent.context", options: { format: "bogus" } },
        { id: "git.repo", options: { pr: "off", sync: true } },
      ],
      right: [],
    });
    expect(layout?.left).toEqual([{ id: "path.cwd", options: { format: "folder" } }, { id: "agent.context" }, { id: "git.repo", options: { pr: "off" } }]);
  });

  it("is null for something that isn't a layout", () => {
    expect(normalizeLayout(null)).toBeNull();
    expect(normalizeLayout("left")).toBeNull();
  });

  it("takes a side that's missing as empty", () => {
    expect(normalizeLayout({ left: [{ id: "ai.mode" }] })).toEqual({ left: [{ id: "ai.mode" }], right: [] });
  });
});

describe("placeItem", () => {
  const base = { left: [{ id: "ai.mode" as const }, { id: "agent.plan" as const }], right: [{ id: "git.repo" as const, options: { pr: "off" } }] };

  it("adds an item that wasn't in the bar", () => {
    expect(placeItem(base, "system.clock", "right", 1).right.map((ref) => ref.id)).toEqual(["git.repo", "system.clock"]);
  });

  it("moves one across, with its options", () => {
    const moved = placeItem(base, "git.repo", "left", 0);
    expect(moved.left[0]).toEqual({ id: "git.repo", options: { pr: "off" } });
    expect(moved.right).toEqual([]);
  });

  it("reorders within a side, and changes nothing dropped where it was", () => {
    expect(placeItem(base, "agent.plan", "left", 0).left.map((ref) => ref.id)).toEqual(["agent.plan", "ai.mode"]);
    expect(sameLayout(placeItem(base, "agent.plan", "left", 1), base)).toBe(true);
  });

  it("clamps an index past the end", () => {
    expect(placeItem(base, "ai.mode", "left", 99).left.map((ref) => ref.id)).toEqual(["agent.plan", "ai.mode"]);
  });
});

describe("options", () => {
  it("reads defaults when nothing was set", () => {
    expect(optionValue({ id: "path.cwd" }, "format")).toBe("home");
    expect(optionOn({ id: "system.clock" }, "seconds")).toBe(false);
  });

  it("stores only what differs from the default", () => {
    const layout = { left: [{ id: "path.cwd" as const }], right: [] };
    const changed = setItemOption(layout, "path.cwd", "format", "full");
    expect(changed.left[0]).toEqual({ id: "path.cwd", options: { format: "full" } });
    expect(setItemOption(changed, "path.cwd", "format", "home").left[0]).toEqual({ id: "path.cwd" });
  });

  it("ignores an option the item doesn't have", () => {
    const layout = { left: [{ id: "ai.mode" as const }], right: [] };
    expect(setItemOption(layout, "ai.mode", "format", "full")).toBe(layout);
  });
});

it("removeItem takes it from either side", () => {
  expect(removeItem(DEFAULT_LAYOUT, "git.repo").right.some((ref) => ref.id === "git.repo")).toBe(false);
});

it("every item in the default layout is in the catalog, once", () => {
  const ids = [...DEFAULT_LAYOUT.left, ...DEFAULT_LAYOUT.right].map((ref) => ref.id);
  expect(new Set(ids).size).toBe(ids.length);
  for (const id of ids) expect(STATUS_ITEMS.some((spec) => spec.id === id)).toBe(true);
});
