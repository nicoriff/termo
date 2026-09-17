import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { busyConsoles } from "$lib/core/pty-client";
import { uiState } from "$lib/core/ui-state.svelte";
import { quitApp } from "$lib/core/window-client";

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
  const consoles = await busyConsoles().catch(() => 0);
  const agents = appConfigStore.aiModeEnabled
    ? aiStatusStore.counts.running + aiStatusStore.counts.awaiting
    : 0;

  if (consoles === 0 && agents === 0) {
    void quitApp();
    return;
  }
  uiState.askBeforeQuit({ consoles, agents });
}
