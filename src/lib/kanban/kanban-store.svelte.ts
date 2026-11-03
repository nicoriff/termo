import {
  cancelKanbanTask,
  deleteKanbanBoard,
  deleteKanbanTask,
  listKanbanBoards,
  mergeKanbanTask,
  moveKanbanTask,
  noteKanbanTask,
  onKanban,
  resolveKanbanTask,
  saveKanbanBoard,
  setKanbanEnabled,
  upsertKanbanTask,
  type KanbanBoard,
  type KanbanColumn,
  type KanbanDecision,
  type KanbanSettings,
  type KanbanTaskInput,
} from "$lib/core/kanban-client";
import { agentsStore } from "$lib/agents/agents-store.svelte";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { explorerStore } from "$lib/explorer/explorer-store.svelte";
import { gitStore } from "$lib/git/git-store.svelte";
import { harnessStore } from "$lib/harness/harness-store.svelte";
import { collectSessions } from "$lib/layout/split-tree";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { tabGroupsStore } from "$lib/tab-groups/tab-groups-store.svelte";
import { announceKanban } from "./kanban-attention";

export const KANBAN_WIDTH = { initial: 260, min: 200, max: 480 };
const WIDTH_KEY = "termo.kanban.width";

function clampWidth(width: number): number {
  return Math.round(Math.min(KANBAN_WIDTH.max, Math.max(KANBAN_WIDTH.min, width)));
}

function loadWidth(): number {
  try {
    const saved = Number(localStorage.getItem(WIDTH_KEY));
    return saved ? clampWidth(saved) : KANBAN_WIDTH.initial;
  } catch {
    return KANBAN_WIDTH.initial;
  }
}

/** Which dialog is open over the window: a board's settings (new when the
 * settings have no id), or a task's editor (new when there's no task id). */
export type KanbanDialog =
  | { kind: "board"; settings: KanbanSettings | null; tab: "general" | "agents" }
  | { kind: "task"; boardId: string; taskId: string | null; column?: KanbanColumn };

/**
 * The kanban boards (see src-tauri/src/agents/kanban), kept by the backend:
 * every change arrives through the `kanban` event, so a board that's
 * running shows its cards moving. Plus the side panel that lists them,
 * docked on the window's left edge like the others: one at a time.
 */
class KanbanStore {
  boards = $state<KanbanBoard[]>([]);
  open = $state(false);
  width = $state(loadWidth());
  dialog = $state<KanbanDialog | null>(null);
  /** The board last opened or worked on: where a new task goes by default. */
  private lastBoardId = $state("");
  private started = false;

  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    await onKanban((board) => this.take(board));
    this.boards = await listKanbanBoards().catch(() => []);
  }

  get(id: string): KanbanBoard | undefined {
    return this.boards.find((board) => board.id === id);
  }

  /** How many tasks wait for the user, on every board: the panel button's badge. */
  get needingYou(): number {
    return this.boards.reduce((sum, board) => sum + board.tasks.filter((task) => task.needsYou !== "").length, 0);
  }

  get running(): number {
    return this.boards.reduce((sum, board) => sum + board.tasks.filter((task) => task.runId !== "").length, 0);
  }

  private take(board: KanbanBoard): void {
    const before = this.get(board.id);
    announceKanban(before, board);
    this.boards = before
      ? this.boards.map((other) => (other.id === board.id ? board : other))
      : [...this.boards, board].sort((a, b) => a.name.localeCompare(b.name));
    // Its column agents and their project are saved by the backend: ones this
    // window hasn't seen. Without the project, the agents would pass for the
    // user's own in the Agents panel.
    const ids = [board.agents.work, board.agents.review, board.agents.summary].flatMap((agent) => agent?.agentIds ?? []);
    const unseenAgent = ids.some((id) => !agentsStore.agents.some((agent) => agent.id === id));
    const unseenProject = board.projectId !== "" && !agentsStore.projects.some((project) => project.id === board.projectId);
    if (unseenAgent || unseenProject) void agentsStore.reload();
  }

  private replace(board: KanbanBoard): KanbanBoard {
    this.take(board);
    return board;
  }

  toggle(): void {
    this.setOpen(!this.open);
  }

  setOpen(open: boolean): void {
    // Agents' work, like every agent feature: it doesn't open with Modo IA off.
    if (open && !appConfigStore.aiModeEnabled) return;
    // One side panel at a time: they're all docked on the same edge.
    if (open) {
      agentsStore.setOpen(false);
      explorerStore.open = false;
      gitStore.open = false;
      harnessStore.setOpen(false);
      tabGroupsStore.open = false;
    }
    this.open = open;
  }

  setWidth(width: number, persist = false): void {
    this.width = clampWidth(width);
    if (!persist) return;
    try {
      localStorage.setItem(WIDTH_KEY, String(this.width));
    } catch {
      // Private mode or storage off: it just won't be remembered.
    }
  }

  openBoard(board: KanbanBoard): void {
    this.lastBoardId = board.id;
    workspaceStore.openKanban(board.id, board.name);
  }

  /** The board a new task goes to: the one in the focused tab, else the one
   * last opened or worked on, else the only one there is. */
  private targetBoard(): KanbanBoard | undefined {
    const root = workspaceStore.root;
    const focused = workspaceStore.focusedSessionId;
    const shown = root && focused ? collectSessions(root).find((loc) => loc.session.id === focused)?.session.kanbanView : undefined;
    return (
      (shown && this.get(shown.boardId)) ||
      this.get(this.lastBoardId) ||
      (this.boards.length === 1 ? this.boards[0] : undefined)
    );
  }

  /** Whether the new-task shortcut would go to this board. */
  isTarget(boardId: string): boolean {
    return this.targetBoard()?.id === boardId;
  }

  /** A new task, from the shortcut: on the board that fits, or (with no way
   * to tell which) the panel, to pick one; a first board when there's none. */
  newTaskAnywhere(): void {
    if (!appConfigStore.aiModeEnabled) return;
    const board = this.targetBoard();
    if (board) {
      this.editTask(board.id, null);
      return;
    }
    this.setOpen(true);
    if (this.boards.length === 0) this.newBoard();
  }

  newBoard(): void {
    this.dialog = { kind: "board", settings: null, tab: "general" };
  }

  editBoard(settings: KanbanSettings, tab: "general" | "agents" = "general"): void {
    this.dialog = { kind: "board", settings, tab };
  }

  editTask(boardId: string, taskId: string | null, column?: KanbanColumn): void {
    this.lastBoardId = boardId;
    this.dialog = { kind: "task", boardId, taskId, column };
  }

  closeDialog(): void {
    this.dialog = null;
  }

  async save(settings: KanbanSettings): Promise<KanbanBoard> {
    return this.replace(await saveKanbanBoard($state.snapshot(settings)));
  }

  async setEnabled(id: string, enabled: boolean): Promise<void> {
    this.replace(await setKanbanEnabled(id, enabled));
  }

  async remove(id: string): Promise<void> {
    this.boards = await deleteKanbanBoard(id);
  }

  async upsertTask(boardId: string, task: KanbanTaskInput): Promise<KanbanBoard> {
    return this.replace(await upsertKanbanTask(boardId, $state.snapshot(task)));
  }

  /** Moves a card at once, here, then as the backend saved it. */
  async move(boardId: string, taskId: string, column: KanbanColumn, before: string | null, cancel = false): Promise<void> {
    const board = this.get(boardId);
    if (board) {
      const tasks = board.tasks.filter((task) => task.id !== taskId);
      const moving = board.tasks.find((task) => task.id === taskId);
      if (moving) {
        const moved = { ...moving, column };
        let at = before ? tasks.findIndex((task) => task.id === before) : -1;
        if (at < 0) {
          const last = tasks.findLastIndex((task) => task.column === column);
          at = last < 0 ? tasks.length : last + 1;
        }
        tasks.splice(at, 0, moved);
        this.boards = this.boards.map((other) => (other.id === boardId ? { ...board, tasks } : other));
      }
    }
    try {
      this.replace(await moveKanbanTask(boardId, taskId, column, before, cancel));
    } catch (error) {
      // As it really is.
      if (board) this.boards = this.boards.map((other) => (other.id === boardId ? board : other));
      throw error;
    }
  }

  async cancelTask(boardId: string, taskId: string): Promise<void> {
    this.replace(await cancelKanbanTask(boardId, taskId));
  }

  async deleteTask(boardId: string, taskId: string): Promise<void> {
    this.replace(await deleteKanbanTask(boardId, taskId));
  }

  async resolveTask(boardId: string, taskId: string, decision: KanbanDecision, text: string): Promise<void> {
    this.replace(await resolveKanbanTask(boardId, taskId, decision, text));
  }

  async mergeTask(boardId: string, taskId: string): Promise<void> {
    this.replace(await mergeKanbanTask(boardId, taskId));
  }

  async noteTask(boardId: string, taskId: string, text: string): Promise<void> {
    this.replace(await noteKanbanTask(boardId, taskId, text));
  }
}

export const kanbanStore = new KanbanStore();
