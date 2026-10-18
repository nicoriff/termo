<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import {
    agentsHostState,
    onAgentsHost,
    rotateConsoleToken,
    setAgentsInEveryConsole,
    setBackgroundAgents,
    type AgentsHostState,
    type ConsoleRegistration,
  } from "$lib/core/agents-client";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { onMount } from "svelte";

  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleAgents"), isMac));

  /** Where the agents run, as the backend last said. */
  let host = $state<AgentsHostState | null>(null);
  let moving = $state(false);
  let moveError = $state<string | null>(null);

  onMount(() => {
    void agentsHostState().then((state) => (host = state));
    const unlisten = onAgentsHost((state) => (host = state));
    return () => void unlisten.then((stop) => stop());
  });

  async function setBackground(enabled: boolean) {
    moving = true;
    moveError = null;
    try {
      await setBackgroundAgents(enabled);
      appConfigStore.backgroundAgentsMoved(enabled);
    } catch (reason) {
      moveError = String(reason);
    } finally {
      moving = false;
      host = await agentsHostState().catch(() => host);
    }
  }

  /** What the process has on its hands, in a line. */
  const hostSummary = $derived.by(() => {
    const status = host?.status;
    if (!status) return "";
    const parts = [
      status.runningRuns + status.runningOrchestrations > 0
        ? t("preferences.agents.hostWorking", { count: String(status.runningRuns + status.runningOrchestrations) })
        : null,
      status.scheduled > 0 ? t("preferences.agents.hostScheduled", { count: String(status.scheduled) }) : null,
      status.watching > 0 ? t("preferences.agents.hostWatching", { count: String(status.watching) }) : null,
      status.onGit > 0 ? t("preferences.agents.hostGit", { count: String(status.onGit) }) : null,
      status.onWebhook > 0 ? t("preferences.agents.hostWebhook", { count: String(status.onWebhook) }) : null,
    ].filter((part): part is string => part !== null);
    return parts.length > 0 ? parts.join(" · ") : t("preferences.agents.hostIdle");
  });

  /** How registering in each CLI's settings went, after a change. */
  let results = $state<ConsoleRegistration[] | null>(null);
  let working = $state(false);

  /** A new token: the one written in the CLIs' files stops working. */
  async function rotate() {
    working = true;
    results = null;
    try {
      results = (await rotateConsoleToken()).filter((result) => !result.skipped);
      rotated = true;
    } catch (reason) {
      results = [{ engine: "claude", skipped: false, ok: false, message: String(reason) }];
    } finally {
      working = false;
    }
  }

  let rotated = $state(false);

  async function setEveryConsole(enable: boolean) {
    appConfigStore.setAgentsFromAnyConsole(enable);
    working = true;
    results = null;
    try {
      results = (await setAgentsInEveryConsole(enable)).filter((result) => !result.skipped);
    } catch (reason) {
      results = [{ engine: "claude", skipped: false, ok: false, message: String(reason) }];
    } finally {
      working = false;
    }
  }
</script>

<section>
  <h3>{t("preferences.agents.access")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.agents.inActivityBar")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.agentsInActivityBar}
      onchange={(e) => appConfigStore.setAgentsInActivityBar(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.agents.inActivityBarHint", { shortcut })}</p>
</section>

<section>
  <h3>{t("preferences.agents.background")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.agents.keepRunning")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.backgroundAgents}
      disabled={moving}
      onchange={(e) => void setBackground(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.agents.keepRunningHint")}</p>
  {#if moving}
    <p class="setting-hint">{t("preferences.agents.switching")}</p>
  {:else if moveError}
    <p class="setting-hint error">{moveError}</p>
  {:else if appConfigStore.backgroundAgents}
    {#if !appConfigStore.aiModeEnabled}
      <p class="setting-hint">{t("preferences.agents.hostAiOff")}</p>
    {:else if host?.connected}
      <p class="host-state">
        <span class="dot ok" aria-hidden="true"></span>{t("preferences.agents.hostConnected")}
        <span class="host-summary">{hostSummary}</span>
      </p>
      {#if host.status?.retiring}
        <p class="setting-hint">{t("preferences.agents.hostRetiring")}</p>
      {/if}
    {:else if host?.enabled}
      <p class="host-state"><span class="dot" aria-hidden="true"></span>{t("preferences.agents.hostStarting")}</p>
    {:else}
      <p class="host-state"><span class="dot bad" aria-hidden="true"></span>{t("preferences.agents.hostDown")}</p>
    {/if}
  {/if}
  <label class="toggle-row login-row" class:disabled={!appConfigStore.backgroundAgents}>
    <span>{t("preferences.agents.atLogin")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.backgroundAgentsAtLogin}
      disabled={!appConfigStore.backgroundAgents}
      onchange={(e) => appConfigStore.setBackgroundAgentsAtLogin(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.agents.atLoginHint")}</p>
</section>

<section>
  <h3>{t("preferences.agents.consoles")}</h3>
  <p class="setting-hint first">{t("preferences.agents.consolesHint")}</p>
  <label class="toggle-row">
    <span>{t("preferences.agents.everyConsole")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.agentsFromAnyConsole}
      disabled={working}
      onchange={(e) => void setEveryConsole(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.agents.everyConsoleHint")}</p>
  {#if appConfigStore.agentsFromAnyConsole}
    <div class="rotate-row">
      <button class="rotate" disabled={working} onclick={() => void rotate()}>{t("preferences.agents.rotate")}</button>
      <span class="setting-hint inline">{rotated ? t("preferences.agents.rotated") : t("preferences.agents.rotateHint")}</span>
    </div>
  {/if}
  {#if working}
    <p class="setting-hint">{t("preferences.agents.registering")}</p>
  {:else if results}
    <ul class="results">
      {#each results as result (result.engine)}
        <li class:bad={!result.ok}>
          {result.ok ? "✓" : "✕"} {TOOL_DISPLAY_NAMES[result.engine]}{#if result.message}: {result.message}{/if}
        </li>
      {:else}
        <li>{t("preferences.agents.noClis")}</li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  section {
    margin-bottom: 20px;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    margin: 0 0 8px;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 6px;
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .setting-hint.first {
    margin: 0 0 8px;
    line-height: 1.5;
  }

  .rotate-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }

  .rotate {
    flex-shrink: 0;
    padding: 3px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .rotate:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .setting-hint.inline {
    margin: 0;
  }

  .results {
    margin: 6px 0 0;
    padding: 0;
    list-style: none;
    font-size: 11px;
  }

  .results .bad {
    color: var(--error-fg, #f48771);
  }

  .setting-hint.error {
    color: var(--error-fg, #f48771);
    opacity: 1;
  }

  .login-row {
    margin-top: 12px;
  }

  .login-row.disabled {
    opacity: 0.5;
    cursor: default;
  }

  .host-state {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 8px 0 0;
    font-size: 11px;
  }

  .host-summary {
    opacity: 0.6;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warning-fg, #d7ba7d);
    flex-shrink: 0;
  }

  .dot.ok {
    background: var(--success-fg, #89d185);
  }

  .dot.bad {
    background: var(--error-fg, #f48771);
  }
</style>
