import type { KanbanBoard, KanbanTask } from "$lib/core/kanban-client";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { notify, type NotificationTarget } from "$lib/core/notification-client";
import { isPrimaryWindow } from "$lib/core/window-client";
import { t } from "$lib/i18n/i18n.svelte";
import { canNotify } from "$lib/ai-mode/attention";

/** How much of the reason goes in the notification. */
const MAX_BODY = 140;

/** What changed on a board worth interrupting for: tasks that now wait for
 * the user, and tasks that just reached Done. A board's runs don't notify one
 * by one (see agent-attention.ts): the board speaks for them. */
export function boardNews(before: KanbanBoard | undefined, after: KanbanBoard): { stuck: KanbanTask[]; done: KanbanTask[] } {
  if (!before) return { stuck: [], done: [] };
  const was = new Map(before.tasks.map((task) => [task.id, task]));
  const stuck = after.tasks.filter((task) => task.needsYou !== "" && (was.get(task.id)?.needsYou ?? "") === "" && was.has(task.id));
  const done = after.tasks.filter((task) => {
    const old = was.get(task.id);
    // Moved there by an agent, not by the user's own hand.
    return old !== undefined && old.column !== "done" && task.column === "done" && old.runId !== "";
  });
  return { stuck, done };
}

export function announceKanban(before: KanbanBoard | undefined, after: KanbanBoard): void {
  const { stuck, done } = boardNews(before, after);
  if (stuck.length === 0 && done.length === 0) return;
  void (async () => {
    // Every window hears every change; one of them speaks.
    if (!(await isPrimaryWindow().catch(() => true))) return;
    if (!(await canNotify())) return;
    const target: NotificationTarget = { kind: "kanban", boardId: after.id, title: after.name };
    const name = `${after.icon} ${after.name}`.trim();
    if (stuck.length > 0 && appConfigStore.aiNotifyAwaiting) {
      const task = stuck[0];
      const reason = task.needsYou.replace(/\s+/g, " ").trim();
      void notify(
        t("kanban.notify.needsYou", { n: String(task.number), title: task.title, name }),
        reason.length > MAX_BODY ? `${reason.slice(0, MAX_BODY)}…` : reason,
        target,
      );
      return;
    }
    if (done.length > 0 && appConfigStore.aiNotifyFinished) {
      const task = done[0];
      void notify(t("kanban.notify.done", { n: String(task.number), title: task.title, name }), "", target);
    }
  })();
}
