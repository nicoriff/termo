import { sendNotification } from "@tauri-apps/plugin-notification";
import { onAgentRun, type RunRecord } from "$lib/core/agents-client";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { isPrimaryWindow } from "$lib/core/window-client";
import { t } from "$lib/i18n/i18n.svelte";
import { collectSessions } from "$lib/layout/split-tree";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { canNotify } from "$lib/ai-mode/attention";
import { playSound } from "$lib/ai-mode/sounds";
import { agentsStore } from "./agents-store.svelte";

/** How much of the answer goes in the notification. */
const MAX_BODY = 140;

/** Whether the user is already looking at this agent's runs. */
function watching(agentId: string): boolean {
  if (!document.hasFocus() || !workspaceStore.root) return false;
  const tab = collectSessions(workspaceStore.root).find((loc) => loc.session.agentRun?.agentId === agentId);
  return tab ? workspaceStore.isSessionVisible(tab.session.id) : false;
}

async function announce(run: RunRecord) {
  // Every console window hears every run; one of them speaks, or each would
  // play the sound and post the notice again.
  if (!(await isPrimaryWindow().catch(() => true))) return;
  const agent = agentsStore.agents.find((candidate) => candidate.id === run.agentId);
  // Tests from the wizard are watched as they happen; stopping one was the
  // user's own doing.
  if (!agent || run.trigger === "test" || run.status === "stopped" || watching(run.agentId)) return;
  const ok = run.status === "success";
  // Same switches and sounds as the engine's console sessions (Preferences →
  // Modo IA → Notifications).
  if (ok && appConfigStore.aiSoundsEnabled[agent.engine]) {
    playSound(appConfigStore.aiSounds[agent.engine].finished, appConfigStore.aiSoundVolume);
  }
  if (!(ok ? appConfigStore.aiNotifyFinished : appConfigStore.aiNotifyError)) return;
  if (!(await canNotify())) return;
  const summary = run.summary.replace(/\s+/g, " ").trim();
  sendNotification({
    title: t(ok ? "agents.notify.success" : "agents.notify.failed", { name: `${agent.icon} ${agent.name}` }),
    body: summary.length > MAX_BODY ? `${summary.slice(0, MAX_BODY)}…` : summary,
  });
}

/** Tells the user when an agent that ran on its own is done. Returns a
 * teardown function. */
export function startAgentRunAttention(): () => void {
  let unlisten: (() => void) | null = null;
  let stopped = false;
  void onAgentRun((run) => {
    if (run.status !== "running") void announce(run);
  }).then((stop) => {
    if (stopped) stop();
    else unlisten = stop;
  });
  return () => {
    stopped = true;
    unlisten?.();
  };
}
