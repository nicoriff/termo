<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { rotateConsoleToken, setAgentsInEveryConsole, type ConsoleRegistration } from "$lib/core/agents-client";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { NUDGE_KINDS, nudgeTitle } from "$lib/companion/companion";

  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleAgents"), isMac));

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
  <h3>{t("companion.title")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.agents.companionNudges")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.companionNudges}
      onchange={(e) => appConfigStore.setCompanionNudges(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.agents.companionNudgesHint")}</p>
  {#if appConfigStore.companionNudges}
    {#each NUDGE_KINDS as kind (kind)}
      <label class="toggle-row">
        <span>{t(nudgeTitle(kind))}</span>
        <input
          type="checkbox"
          checked={!appConfigStore.companionMuted.includes(kind)}
          onchange={(e) => appConfigStore.setCompanionNotice(kind, e.currentTarget.checked)}
        />
      </label>
    {/each}
  {/if}
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
</style>
