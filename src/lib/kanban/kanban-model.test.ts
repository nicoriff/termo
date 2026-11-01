import { describe, expect, it } from "vitest";
import type { KanbanBoard, KanbanColumn, KanbanTask } from "$lib/core/kanban-client";
import { beforeAt, blockers, boardActivity, needsCancel, newBoardSettings, nextUp, queuePlaces, tasksIn } from "./kanban-model";

function task(id: string, column: KanbanColumn, dependsOn: string[] = [], extra: Partial<KanbanTask> = {}): KanbanTask {
  return {
    id,
    number: 0,
    title: id,
    spec: "",
    successCriteria: "",
    dependsOn,
    column,
    role: null,
    runId: "",
    agentId: "",
    summaryPending: false,
    reviewRounds: 0,
    reviewNotes: "",
    verdict: null,
    report: null,
    workSummary: "",
    needsYou: "",
    outcome: null,
    costUsd: null,
    branch: "",
    worktree: "",
    merged: false,
    log: [],
    createdAt: 0,
    updatedAt: 0,
    ...extra,
  };
}

function board(tasks: KanbanTask[]): KanbanBoard {
  return {
    ...newBoardSettings(() => ""),
    id: "b",
    enabled: false,
    tasks: tasks.map((each, index) => ({ ...each, number: index + 1 })),
    projectId: "",
    nextNumber: tasks.length + 1,
    createdAt: 0,
    updatedAt: 0,
  };
}

describe("nextUp", () => {
  it("follows the board's order and skips what waits, is stuck or runs", () => {
    const b = board([
      task("a", "todo", ["c"]),
      task("b", "todo"),
      task("c", "todo"),
      task("d", "todo", [], { needsYou: "why" }),
      task("e", "todo", [], { runId: "r" }),
      task("f", "notReady"),
    ]);
    expect(nextUp(b)).toEqual(["b", "c"]);
    expect(queuePlaces(b).get("c")).toBe(2);
    expect(blockers(b, b.tasks[0])).toEqual([3]);
  });

  it("forgets a dependency that was deleted", () => {
    expect(nextUp(board([task("a", "todo", ["gone"])]))).toEqual(["a"]);
  });
});

describe("beforeAt", () => {
  it("ignores the card being moved", () => {
    const column = tasksIn(board([task("a", "todo"), task("b", "todo"), task("c", "todo")]), "todo");
    expect(beforeAt(column, "a", 0)).toBe("b");
    expect(beforeAt(column, "a", 1)).toBe("c");
    expect(beforeAt(column, "a", 2)).toBeNull();
    expect(beforeAt(column, "z", 0)).toBe("a");
  });
});

describe("needsCancel", () => {
  it("asks only when an agent is on it and it leaves its column", () => {
    const running = task("a", "inProgress", [], { runId: "r" });
    expect(needsCancel(running, "notReady")).toBe(true);
    expect(needsCancel(running, "inProgress")).toBe(false);
    expect(needsCancel(task("b", "inProgress"), "todo")).toBe(false);
  });
});

describe("boardActivity", () => {
  it("counts what the panel shows", () => {
    const b = board([task("a", "inProgress", [], { runId: "r" }), task("b", "review", [], { needsYou: "x" }), task("c", "done")]);
    expect(boardActivity(b)).toEqual({ running: 1, needsYou: 1, done: 1, total: 3 });
  });
});
