import { onAgentQuestion, onAgentRun, type RunQuestion, type RunRecord, type RunTrigger } from "$lib/core/agents-client";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { notify } from "$lib/core/notification-client";
import { isPrimaryWindow } from "$lib/core/window-client";
import { t } from "$lib/i18n/i18n.svelte";
import { collectSessions } from "$lib/layout/split-tree";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { canNotify } from "$lib/ai-mode/attention";
import { playSound } from "$lib/ai-mode/sounds";
import { agentsStore } from "./agents-store.svelte";

/** How much of the answer goes in the notification. */
const MAX_BODY = 140;

/** Runs the user started, and is waiting on. */
const ASKED: ReadonlySet<RunTrigger> = new Set(["manual", "console"]);

/** Agents whose run on their own failed and already said so. */
const failing = new Set<string>();

/** Whether an ended run is worth telling. One the user started, always; one
 * that ran on its own (a schedule, a folder, git, a webhook, another agent)
 * only when it fails, and once: a broken agent on a schedule says it the
 * first time, not on every run, until it works again. Keeps `failed` up to
 * date either way. */
export function worthTelling(run: RunRecord, failed: Set<string>): boolean {
  if (run.status === "success") {
    failed.delete(run.agentId);
    return ASKED.has(run.trigger);
  }
  if (ASKED.has(run.trigger)) return true;
  if (failed.has(run.agentId)) return false;
  failed.add(run.agentId);
  return true;
}

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
  // user's own doing; a task of an orchestration is announced by its
  // orchestration, not one by one (see orchestration-attention.ts).
  const quiet = run.trigger === "test" || run.trigger === "orchestrator" || run.status === "stopped";
  if (!agent || quiet || !worthTelling(run, failing) || watching(run.agentId)) return;
  const ok = run.status === "success";
  // Same switches and sounds as the engine's console sessions (Preferences →
  // Modo IA → Notifications).
  if (ok && appConfigStore.aiSoundsEnabled[agent.engine]) {
    playSound(appConfigStore.aiSounds[agent.engine].finished, appConfigStore.aiSoundVolume);
  }
  if (!(ok ? appConfigStore.aiNotifyFinished : appConfigStore.aiNotifyError)) return;
  if (!(await canNotify())) return;
  const summary = run.summary.replace(/\s+/g, " ").trim();
  // Clicked, either notice leads to the run.
  void notify(
    t(ok ? "agents.notify.success" : "agents.notify.failed", { name: `${agent.icon} ${agent.name}` }),
    summary.length > MAX_BODY ? `${summary.slice(0, MAX_BODY)}…` : summary,
    { kind: "agentRun", agentId: agent.id, runId: run.id, title: agent.name },
  );
}

/** A run is waiting on the user: it stands still until it's answered. */
async function announceQuestion(question: RunQuestion) {
  if (!(await isPrimaryWindow().catch(() => true))) return;
  const agent = agentsStore.agents.find((candidate) => candidate.id === question.agentId);
  if (!agent) return;
  // The switches and sounds of a console session that needs an answer.
  if (appConfigStore.aiSoundsEnabled[agent.engine]) {
    playSound(appConfigStore.aiSounds[agent.engine].awaiting, appConfigStore.aiSoundVolume);
  }
  if (!appConfigStore.aiNotifyAwaiting || watching(question.agentId)) return;
  if (!(await canNotify())) return;
  const text = question.question.replace(/\s+/g, " ").trim();
  void notify(
    t("agents.notify.question", { name: `${agent.icon} ${agent.name}` }),
    text.length > MAX_BODY ? `${text.slice(0, MAX_BODY)}…` : text,
    { kind: "agentRun", agentId: agent.id, runId: question.runId, title: agent.name },
  );
}

/** Tells the user when an agent's run they started is done, when one that
 * ran on its own fails, and when one is waiting on them. Returns a teardown
 * function. */
export function startAgentRunAttention(): () => void {
  const unlisten: (() => void)[] = [];
  let stopped = false;
  const keep = (stop: () => void) => {
    if (stopped) stop();
    else unlisten.push(stop);
  };
  void onAgentRun((run) => {
    if (run.status !== "running") void announce(run);
  }).then(keep);
  void onAgentQuestion((question) => {
    if (question.state === "pending") void announceQuestion(question);
  }).then(keep);
  return () => {
    stopped = true;
    for (const stop of unlisten) stop();
  };
}
