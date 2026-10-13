import type { Orchestration, OrchestrationStatus, UserQuestion } from "$lib/core/orchestration-client";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { notify, type NotificationTarget } from "$lib/core/notification-client";
import { isPrimaryWindow } from "$lib/core/window-client";
import { t } from "$lib/i18n/i18n.svelte";
import type { MessageKey } from "$lib/i18n/translate";
import { canNotify } from "$lib/ai-mode/attention";

/** How much of the orchestrator's note goes in the notification. */
const MAX_BODY = 140;

/** The states worth interrupting for, once it was running: it's done, it
 * needs an answer, or it can't go on. Stopping it was the user's own doing. */
const TITLES: Partial<Record<OrchestrationStatus, MessageKey>> = {
  done: "agents.orchestration.notify.done",
  awaitingApproval: "agents.orchestration.notify.awaitingApproval",
  paused: "agents.orchestration.notify.paused",
  failed: "agents.orchestration.notify.failed",
};

/** The notification's title for a change of state, if it deserves one. A
 * team's tasks don't notify one by one (see agent-attention.ts): the
 * orchestration speaks for them. */
export function attentionTitle(before: OrchestrationStatus | undefined, after: OrchestrationStatus): MessageKey | null {
  return before === "running" && after !== "running" ? (TITLES[after] ?? null) : null;
}

/** The questions an agent just asked: in `after`, waiting, and not in `before`. */
export function newQuestions(before: Orchestration | undefined, after: Orchestration): UserQuestion[] {
  if (!before) return [];
  const known = new Set((before.questions ?? []).map((question) => question.id));
  return (after.questions ?? []).filter((question) => question.state === "pending" && !known.has(question.id));
}

export async function announceOrchestration(before: Orchestration | undefined, after: Orchestration): Promise<void> {
  // Clicked, either notice leads to the orchestration.
  const target: NotificationTarget = { kind: "orchestration", orchestrationId: after.id, title: after.name };
  const asked = newQuestions(before, after);
  if (asked.length > 0) {
    if (!(await isPrimaryWindow().catch(() => true))) return;
    if (!appConfigStore.aiNotifyFinished || !(await canNotify())) return;
    const question = asked[0].question.replace(/\s+/g, " ").trim();
    void notify(
      t("agents.orchestration.notify.question", { agent: asked[0].agent, name: `${after.icon} ${after.name}`.trim() }),
      question.length > MAX_BODY ? `${question.slice(0, MAX_BODY)}…` : question,
      target,
    );
    return;
  }
  const title = attentionTitle(before?.status, after.status);
  if (!title) return;
  // Every window hears every change; one of them speaks.
  if (!(await isPrimaryWindow().catch(() => true))) return;
  // The same switches as the agents' own runs (Preferences → Modo IA).
  if (!(after.status === "failed" ? appConfigStore.aiNotifyError : appConfigStore.aiNotifyFinished)) return;
  if (!(await canNotify())) return;
  const note = after.note.replace(/\s+/g, " ").trim();
  void notify(
    t(title, { name: `${after.icon} ${after.name}`.trim() }),
    note.length > MAX_BODY ? `${note.slice(0, MAX_BODY)}…` : note,
    target,
  );
}
