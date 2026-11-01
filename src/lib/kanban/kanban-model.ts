import type { AgentEngine } from "$lib/core/agents-client";
import type {
  KanbanActor,
  KanbanBoard,
  KanbanColumn,
  KanbanColumnAgent,
  KanbanRole,
  KanbanSettings,
  KanbanTask,
} from "$lib/core/kanban-client";
import type { MessageKey } from "$lib/i18n/translate";

/** The rules the board shows, the same as src-tauri/src/agents/kanban/model.rs:
 * what's next, what waits, what an agent is on. Pure, so it's tested in Node. */

/** Whether everything a task waits for is done. One that was deleted no
 * longer holds it back. */
export function unblocked(board: KanbanBoard, task: KanbanTask): boolean {
  return task.dependsOn.every((dep) => {
    const other = board.tasks.find((candidate) => candidate.id === dep);
    return !other || other.column === "done";
  });
}

/** The numbers of the tasks it still waits for. */
export function blockers(board: KanbanBoard, task: KanbanTask): number[] {
  return task.dependsOn
    .map((dep) => board.tasks.find((candidate) => candidate.id === dep))
    .filter((other): other is KanbanTask => other !== undefined && other.column !== "done")
    .map((other) => other.number);
}

export const isRunning = (task: KanbanTask): boolean => task.runId !== "";

/** The To do tasks the work agent takes, in the order it takes them. */
export function nextUp(board: KanbanBoard): string[] {
  return board.tasks
    .filter((task) => task.column === "todo" && task.needsYou === "" && !isRunning(task) && unblocked(board, task))
    .map((task) => task.id);
}

/** Each task's place in the queue (1 is next), by id. */
export function queuePlaces(board: KanbanBoard): Map<string, number> {
  return new Map(nextUp(board).map((id, index) => [id, index + 1]));
}

export function tasksIn(board: KanbanBoard, column: KanbanColumn): KanbanTask[] {
  return board.tasks.filter((task) => task.column === column);
}

/** The task a card dropped at `index` of a column lands before (its id), or
 * null for the end. The card being moved doesn't count. */
export function beforeAt(column: KanbanTask[], movingId: string, index: number): string | null {
  const others = column.filter((task) => task.id !== movingId);
  return others[index]?.id ?? null;
}

/** Whether a move needs the user to confirm: an agent is on the task and it
 * would leave its column. */
export function needsCancel(task: KanbanTask, to: KanbanColumn): boolean {
  return isRunning(task) && task.column !== to;
}

/** What the harness of each column says by default, in the user's language. */
export const DEFAULT_HARNESS: Record<KanbanRole, MessageKey> = {
  work: "kanban.agents.harnessDefault.work",
  review: "kanban.agents.harnessDefault.review",
  summary: "kanban.agents.harnessDefault.summary",
};

export function newColumnAgent(engine: AgentEngine = "claude", harness = ""): KanbanColumnAgent {
  return { engine, model: "", autonomy: "edit", web: false, context: "", harness, agentIds: [] };
}

/** A new board's settings: a work agent that edits, a reviewer that only looks. */
export function newBoardSettings(harness: (role: KanbanRole) => string): KanbanSettings {
  return {
    id: "",
    name: "",
    icon: "",
    workDir: "",
    mode: "sequential",
    maxParallel: 2,
    maxReviewRounds: 3,
    baseBranch: "",
    askLevel: "essential",
    agents: {
      work: newColumnAgent("claude", harness("work")),
      review: { ...newColumnAgent("claude", harness("review")), autonomy: "read" },
      summary: null,
    },
  };
}

export function settingsOf(board: KanbanBoard): KanbanSettings {
  const { id, name, icon, workDir, mode, maxParallel, maxReviewRounds, baseBranch, askLevel, agents } = board;
  return structuredClone({ id, name, icon, workDir, mode, maxParallel, maxReviewRounds, baseBranch, askLevel, agents });
}

export function actorKey(actor: KanbanActor): MessageKey {
  switch (actor.kind) {
    case "user":
      return "kanban.actor.user";
    case "console":
      return "kanban.actor.console";
    case "termo":
      return "kanban.actor.termo";
    case "agent":
      return actor.role === "work" ? "kanban.actor.work" : actor.role === "review" ? "kanban.actor.review" : "kanban.actor.summary";
  }
}

/** What a board shows on its row in the panel. */
export function boardActivity(board: KanbanBoard): { running: number; needsYou: number; done: number; total: number } {
  return {
    running: board.tasks.filter(isRunning).length,
    needsYou: board.tasks.filter((task) => task.needsYou !== "").length,
    done: board.tasks.filter((task) => task.column === "done").length,
    total: board.tasks.length,
  };
}
