import { describe, expect, it } from "vitest";
import type { KanbanBoard, KanbanColumn, KanbanTask } from "$lib/core/kanban-client";
import { addTag, beforeAt, blockers, boardActivity, boardTags, decisionsFor, retryKey, tagHue, needsCancel, newBoardSettings, nextUp, queuePlaces, tasksIn } from "./kanban-model";

function task(id: string, column: KanbanColumn, dependsOn: string[] = [], extra: Partial<KanbanTask> = {}): KanbanTask {
  return {
    id,
    number: 0,
    title: id,
    spec: "",
    successCriteria: "",
    tags: [],
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
    stuck: null,
    lastAgentId: "",
    lastRunId: "",
    instructions: "",
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
    onHold: "",
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

describe("tags", () => {
  it("adds a typed tag once, trimmed and without '#'", () => {
    expect(addTag([], "  #bug ")).toEqual(["bug"]);
    expect(addTag(["bug"], "BUG")).toEqual(["bug"]);
    expect(addTag(["bug"], "   ")).toEqual(["bug"]);
  });

  it("lists the board's tags, most used first, and colors each the same way", () => {
    const b = board([task("a", "todo", [], { tags: ["ui", "bug"] }), task("b", "todo", [], { tags: ["Bug"] })]);
    expect(boardTags(b)).toEqual(["bug", "ui"]);
    expect(tagHue("Bug")).toBe(tagHue("bug"));
  });
});

describe("boardActivity", () => {
  it("counts what the panel shows", () => {
    const b = board([task("a", "inProgress", [], { runId: "r" }), task("b", "review", [], { needsYou: "x" }), task("c", "done")]);
    expect(boardActivity(b)).toEqual({ running: 1, needsYou: 1, done: 1, total: 3 });
  });
});

describe("decisions on a stuck task", () => {
  it("offers approving only when there is work to approve", () => {
    expect(decisionsFor(task("a", "inProgress", [], { stuck: "tooManyRounds", workSummary: "did it" }))).toEqual(["retry", "approve", "park"]);
    expect(decisionsFor(task("a", "inProgress", [], { stuck: "workFailed" }))).toEqual(["retry", "park"]);
    expect(decisionsFor(task("a", "review", [], { stuck: "noVerdict", workSummary: "did it" }))[0]).toBe("approve");
    expect(decisionsFor(task("a", "review", [], { stuck: "noVerdict" }))).toEqual(["retry", "rework", "park"]);
  });

  it("names taking it up again after what stopped it", () => {
    expect(retryKey(task("a", "review", [], { stuck: "noVerdict" }))).toBe("kanban.decide.reviewAgain");
    expect(retryKey(task("a", "inProgress", [], { stuck: "tooManyRounds" }))).toBe("kanban.decide.anotherRound");
    expect(retryKey(task("a", "inProgress", [], { stuck: "cancelled" }))).toBe("kanban.decide.resume");
  });
});
