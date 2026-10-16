import { describe, expect, it } from "vitest";
import { hydrateLayout, layoutHasSessions, snapshotLayout, type LayoutNode } from "./layout-snapshot";
import { collectTiles, type SplitNode } from "./split-tree";

function ids() {
  let next = 0;
  return () => `id-${next++}`;
}

const tree: SplitNode = {
  kind: "split",
  id: "split-1",
  direction: "row",
  ratio: 0.42,
  children: [
    {
      kind: "leaf",
      id: "tile-1",
      sessions: [
        { id: "a", title: "PowerShell", shellPath: "pwsh.exe", shellArgs: ["-NoLogo"] },
        { id: "b", title: "Claude Code", shellPath: "bash", cwd: "E:/proj" },
      ],
      activeSessionId: "b",
    },
    { kind: "leaf", id: "tile-2", sessions: [{ id: "c", title: "bash" }], activeSessionId: "c" },
  ],
};

/** Tab "b" runs Claude, and it has moved below the folder the tab opened in. */
const agents = { b: { tool: "claude" as const, agentSessionId: "conv-1", cwd: "E:/proj/api" } };

function firstLeaf(node: LayoutNode) {
  const leaf = node.kind === "split" ? node.children[0] : node;
  if (leaf.kind !== "leaf") throw new Error("expected a leaf");
  return leaf;
}

describe("snapshotLayout", () => {
  it("keeps the shape, the ratio and what each console is", () => {
    const snapshot = snapshotLayout(tree);
    expect(snapshot).toEqual({
      kind: "split",
      direction: "row",
      ratio: 0.42,
      children: [
        {
          kind: "leaf",
          activeIndex: 1,
          sessions: [
            { title: "PowerShell", shellPath: "pwsh.exe", shellArgs: ["-NoLogo"] },
            { title: "Claude Code", shellPath: "bash", cwd: "E:/proj" },
          ],
        },
        { kind: "leaf", activeIndex: 0, sessions: [{ title: "bash" }] },
      ],
    });
  });

  it("stores no ids at all", () => {
    expect(JSON.stringify(snapshotLayout(tree))).not.toContain("tile-1");
    expect(JSON.stringify(snapshotLayout(tree))).not.toContain('"a"');
  });

  it("stores the agent conversation of each console only when asked", () => {
    const withAgents = snapshotLayout(tree, { agentByTab: agents, withSessions: true });

    expect(layoutHasSessions(snapshotLayout(tree))).toBe(false);
    expect(layoutHasSessions(withAgents)).toBe(true);
    expect(firstLeaf(withAgents).sessions[1].agentSessionId).toBe("conv-1");
    // A console with no agent stays as it was.
    expect(firstLeaf(withAgents).sessions[0].agentSessionId).toBeUndefined();
  });

  /** SessionTab.cwd is only where the console was opened; a console that moved
   * (or that an agent is driving elsewhere) must come back where it is now. */
  it("prefers the folder the agent reports over the one the tab was created in", () => {
    const snapshot = snapshotLayout(tree, { agentByTab: agents, withSessions: true });
    expect(firstLeaf(snapshot).sessions[1].cwd).toBe("E:/proj/api");
  });

  /** The shell reports its own directory (OSC 7); the agent only reports where
   * it is working. When both are known, the shell wins. */
  it("prefers the folder the shell reports over the agent's", () => {
    const snapshot = snapshotLayout(tree, {
      agentByTab: agents,
      cwdByTab: { b: "E:/proj/api/src" },
      withSessions: true,
    });
    expect(firstLeaf(snapshot).sessions[1].cwd).toBe("E:/proj/api/src");
  });

  it("stores that folder even when the conversations are not saved", () => {
    const snapshot = snapshotLayout(tree, { agentByTab: agents, withSessions: false });
    expect(firstLeaf(snapshot).sessions[1].cwd).toBe("E:/proj/api");
    expect(layoutHasSessions(snapshot)).toBe(false);
  });
});

describe("hydrateLayout", () => {
  it("restores the layout with fresh ids", () => {
    const { root } = hydrateLayout(snapshotLayout(tree), ids());
    expect(root.kind).toBe("split");
    const tiles = collectTiles(root);
    expect(tiles).toHaveLength(2);
    expect(tiles[0].sessions.map((s) => s.title)).toEqual(["PowerShell", "Claude Code"]);
    expect(tiles[0].sessions[0].shellArgs).toEqual(["-NoLogo"]);
    expect(tiles[0].sessions[1].cwd).toBe("E:/proj");

    const everyId = [...tiles.map((t) => t.id), ...tiles.flatMap((t) => t.sessions.map((s) => s.id))];
    expect(new Set(everyId).size).toBe(everyId.length);
    expect(everyId).not.toContain("tile-1");
  });

  /** What a window records for consoles that outlive it: the ones the
   * sessions daemon still runs come back as they are, the rest start anew
   * (and reopen their conversation). */
  it("shows a console that is still running again instead of starting it", () => {
    const recorded = snapshotLayout(tree, {
      agentByTab: agents,
      withSessions: true,
      consoleByTab: { a: "console-a", b: "console-b" },
    });
    const { root, resumes } = hydrateLayout(recorded, ids(), new Set(["console-b"]));
    const [first] = collectTiles(root);
    expect(first.sessions[0].attach).toBeUndefined();
    expect(first.sessions[1].attach).toBe("console-b");
    // Its conversation is still open in it: nothing to resume.
    expect(resumes).toEqual([]);
  });

  it("brings back the tab that was active, by position", () => {
    const { root } = hydrateLayout(snapshotLayout(tree), ids());
    const tiles = collectTiles(root);
    const active = tiles[0].sessions.find((s) => s.id === tiles[0].activeSessionId);
    expect(active?.title).toBe("Claude Code");
  });

  it("reports which of the new tabs should reopen a conversation", () => {
    const { root, resumes } = hydrateLayout(
      snapshotLayout(tree, { agentByTab: agents, withSessions: true }),
      ids(),
    );

    expect(resumes).toHaveLength(1);
    expect(resumes[0]).toMatchObject({ tool: "claude", agentSessionId: "conv-1" });
    // The tab id points at the console that was actually created for it.
    const titles = collectTiles(root).flatMap((tile) =>
      tile.sessions.filter((s) => s.id === resumes[0].tabId).map((s) => s.title),
    );
    expect(titles).toEqual(["Claude Code"]);
  });

  it("has nothing to resume for a layout saved without sessions", () => {
    expect(hydrateLayout(snapshotLayout(tree), ids()).resumes).toEqual([]);
  });

  it("survives a hand-edited file with an empty tile or a bad active index", () => {
    const broken: LayoutNode = { kind: "leaf", sessions: [], activeIndex: 7 };
    const tile = collectTiles(hydrateLayout(broken, ids()).root)[0];
    expect(tile.sessions).toHaveLength(1);
    expect(tile.activeSessionId).toBe(tile.sessions[0].id);
  });
});
