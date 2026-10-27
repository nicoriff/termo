import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { busyConsoles } from "$lib/core/pty-client";
import { agentsHostState, runningAgents } from "$lib/core/agents-client";
import { uiState } from "$lib/core/ui-state.svelte";
import { closeWindow, quitApp } from "$lib/core/window-client";
import { sessionKeeper } from "$lib/layout/session-keeper.svelte";
import { tabGroupsStore } from "$lib/tab-groups/tab-groups-store.svelte";
import { ptyIdsByTab } from "$lib/terminal/pty-registry";

/** What closing Termo would cut short: consoles with a command running under
 * them, and agent sessions (Modo IA) mid-turn or waiting on the user. Also
 * what installing an update would (see updater.svelte.ts). And apart, the
 * same of what it leaves running: consoles in the sessions daemon ("Keep
 * consoles running", see src-tauri/src/mux/) keep going, their agents too.
 * With `thisWindow`, only this window's consoles (what closing it would end).
 * Agent sessions are this window's either way: each window tracks its own.
 * The runs of the user's agents count for Termo as a whole: cut short when
 * they run in the app, kept going when they run in their own process (see
 * src-tauri/src/agents/host/). */
export async function runningWork(thisWindow = false): Promise<{
  consoles: number;
  agents: number;
  keptConsoles: number;
  keptAgents: number;
  runs: number;
  keptRuns: number;
}> {
  const busy = await busyConsoles(thisWindow).catch(() => ({ lost: 0, kept: 0, keptIds: [] }));
  const kept = new Set(busy.keptIds);
  const ptyIds = ptyIdsByTab();
  let agents = 0;
  let keptAgents = 0;
  if (appConfigStore.aiModeEnabled) {
    for (const [tabId, entry] of Object.entries(aiStatusStore.byId)) {
      if (entry.status !== "running" && entry.status !== "awaiting") continue;
      const ptyId = ptyIds[tabId];
      if (ptyId && kept.has(ptyId)) keptAgents++;
      else agents++;
    }
  }
  let runs = 0;
  let keptRuns = 0;
  if (!thisWindow && appConfigStore.aiModeEnabled) {
    const host = await agentsHostState().catch(() => null);
    if (host?.enabled) {
      keptRuns = (host.status?.runningRuns ?? 0) + (host.status?.runningOrchestrations ?? 0);
    } else {
      runs = (await runningAgents().catch(() => [])).length;
    }
  }
  return { consoles: busy.lost, agents, keptConsoles: busy.kept, keptAgents, runs, keptRuns };
}

function anything(work: Awaited<ReturnType<typeof runningWork>>): boolean {
  return work.consoles + work.agents + work.keptConsoles + work.keptAgents + work.runs + work.keptRuns > 0;
}

/**
 * Quitting kills every console, so it asks first — but only when there is
 * something to lose: a shell with a command running under it, or an agent
 * mid-turn. A window full of idle prompts closes without a dialog, because a
 * confirmation that always appears stops being read. Work that keeps running
 * in the background is said too, but as what it is: nothing is lost.
 *
 * If the check itself fails, it is treated as "nothing running": a broken
 * count must not turn Quit into a dead button.
 */
export async function requestQuit(): Promise<void> {
  // The consoles that outlive Termo come back as they are now (see
  // session-keeper), not as they were a moment ago. So do the tab groups'.
  await sessionKeeper.save();
  await tabGroupsStore.flush();
  const work = await runningWork();
  if (!anything(work)) {
    void quitApp();
    return;
  }
  uiState.askBeforeQuit({ ...work, scope: "app" });
}

/** The same for closing this window (its X): only its own consoles count,
 * and the others stay. Closing the last window ends Termo
 * (terminal_windows::on_destroyed). */
export async function requestCloseWindow(): Promise<void> {
  await sessionKeeper.save();
  await tabGroupsStore.flush();
  const work = await runningWork(true);
  if (!anything(work)) {
    void closeWindow();
    return;
  }
  uiState.askBeforeQuit({ ...work, scope: "window" });
}
