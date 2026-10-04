import { sendNotification } from "@tauri-apps/plugin-notification";
import type { Orchestration, OrchestrationStatus } from "$lib/core/orchestration-client";
import { appConfigStore } from "$lib/core/app-config.svelte";
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

export async function announceOrchestration(before: Orchestration | undefined, after: Orchestration): Promise<void> {
  const title = attentionTitle(before?.status, after.status);
  if (!title) return;
  // Every window hears every change; one of them speaks.
  if (!(await isPrimaryWindow().catch(() => true))) return;
  // The same switches as the agents' own runs (Preferences → Modo IA).
  if (!(after.status === "failed" ? appConfigStore.aiNotifyError : appConfigStore.aiNotifyFinished)) return;
  if (!(await canNotify())) return;
  const note = after.note.replace(/\s+/g, " ").trim();
  sendNotification({
    title: t(title, { name: `${after.icon} ${after.name}`.trim() }),
    body: note.length > MAX_BODY ? `${note.slice(0, MAX_BODY)}…` : note,
  });
}
