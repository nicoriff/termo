import type { AgentEngine } from "$lib/core/agents-client";
import type {
  KanbanActor,
  KanbanBoard,
  KanbanColumn,
  KanbanColumnAgent,
  KanbanDecision,
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
  return { engine, model: "", autonomy: "run", web: false, context: "", harness, agentIds: [] };
}

/** A new board's settings: agents that may run commands, since doing the
 *  work and checking it both take building and running its tests. */
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
      review: newColumnAgent("claude", harness("review")),
      summary: null,
    },
  };
}

export function settingsOf(board: KanbanBoard): KanbanSettings {
  const { id, name, icon, workDir, mode, maxParallel, maxReviewRounds, baseBranch, askLevel, agents } = board;
  // A copy through JSON: the board may be a store's reactive proxy, which
  // structuredClone refuses.
  return JSON.parse(JSON.stringify({ id, name, icon, workDir, mode, maxParallel, maxReviewRounds, baseBranch, askLevel, agents }));
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

/** The hues tags take: the same tag always gets the same one. */
const TAG_HUES = [210, 150, 35, 280, 0, 180, 320, 95, 255, 15];

export function tagHue(tag: string): number {
  let hash = 0;
  for (const char of tag.toLowerCase()) hash = (hash * 31 + char.codePointAt(0)!) | 0;
  return TAG_HUES[Math.abs(hash) % TAG_HUES.length];
}

/** Every tag used on the board, once each (whatever its case), most used first. */
export function boardTags(board: KanbanBoard): string[] {
  const counts = new Map<string, { tag: string; n: number }>();
  for (const tag of board.tasks.flatMap((task) => task.tags ?? [])) {
    const key = tag.toLowerCase();
    const known = counts.get(key);
    counts.set(key, { tag: known?.tag ?? tag, n: (known?.n ?? 0) + 1 });
  }
  return [...counts.values()].sort((a, b) => b.n - a.n || a.tag.localeCompare(b.tag)).map((entry) => entry.tag);
}

/** A tag typed in the editor added to the list: trimmed, without '#', once. */
export function addTag(tags: string[], typed: string): string[] {
  const tag = typed.trim().replace(/^#+/, "").trim().slice(0, 32);
  if (!tag || tags.some((other) => other.toLowerCase() === tag.toLowerCase()) || tags.length >= 12) return tags;
  return [...tags, tag];
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

/** What the user is offered on a stuck task, the likeliest first. Approving
 *  needs work to approve; sending it back to work only makes sense from
 *  review. */
export function decisionsFor(task: KanbanTask): KanbanDecision[] {
  const hasWork = task.workSummary.trim() !== "";
  const inReview = task.column === "review";
  switch (task.stuck) {
    case "tooManyRounds":
      return hasWork ? ["retry", "approve", "park"] : ["retry", "park"];
    case "noVerdict":
      return hasWork ? ["approve", "retry", "rework", "park"] : ["retry", "rework", "park"];
    case "reviewFailed":
      return hasWork ? ["retry", "approve", "rework", "park"] : ["retry", "rework", "park"];
    default:
      return inReview ? ["retry", "rework", "park"] : ["retry", "park"];
  }
}

/** The text of the button that takes a task up again where it is. */
export function retryKey(task: KanbanTask): MessageKey {
  if (task.stuck === "cancelled") return "kanban.decide.resume";
  if (task.column === "review") return "kanban.decide.reviewAgain";
  return task.stuck === "tooManyRounds" ? "kanban.decide.anotherRound" : "kanban.decide.retry";
}
