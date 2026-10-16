import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { busyConsoles } from "$lib/core/pty-client";
import { uiState } from "$lib/core/ui-state.svelte";
import { closeWindow, quitApp } from "$lib/core/window-client";
import { sessionKeeper } from "$lib/layout/session-keeper.svelte";

/** What closing Termo would cut short: consoles with a command running under
 * them, and agent sessions (Modo IA) mid-turn or waiting on the user. Also
 * what installing an update would (see updater.svelte.ts). With
 * `thisWindow`, only this window's consoles (what closing it would end).
 * Agent sessions are this window's either way: each window tracks its own. */
export async function runningWork(thisWindow = false): Promise<{ consoles: number; agents: number }> {
  const consoles = await busyConsoles(thisWindow).catch(() => 0);
  const agents = appConfigStore.aiModeEnabled ? aiStatusStore.counts.running + aiStatusStore.counts.awaiting : 0;
  return { consoles, agents };
}

/**
 * Quitting kills every console, so it asks first — but only when there is
 * something to lose: a shell with a command running under it, or an agent
 * mid-turn. A window full of idle prompts closes without a dialog, because a
 * confirmation that always appears stops being read.
 *
 * If the check itself fails, it is treated as "nothing running": a broken
 * count must not turn Quit into a dead button.
 */
export async function requestQuit(): Promise<void> {
  // The consoles that outlive Termo come back as they are now (see
  // session-keeper), not as they were a moment ago.
  await sessionKeeper.save();
  const { consoles, agents } = await runningWork();
  if (consoles === 0 && agents === 0) {
    void quitApp();
    return;
  }
  uiState.askBeforeQuit({ consoles, agents, scope: "app" });
}

/** The same for closing this window (its X): only its own consoles count,
 * and the others stay. Closing the last window ends Termo
 * (terminal_windows::on_destroyed). */
export async function requestCloseWindow(): Promise<void> {
  await sessionKeeper.save();
  const { consoles, agents } = await runningWork(true);
  if (consoles === 0 && agents === 0) {
    void closeWindow();
    return;
  }
  uiState.askBeforeQuit({ consoles, agents, scope: "window" });
}
