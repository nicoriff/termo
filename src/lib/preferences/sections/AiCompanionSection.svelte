<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { TOOL_IDS } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { NUDGE_KINDS, nudgeTitle } from "$lib/companion/companion";
  import SettingDropdown from "../SettingDropdown.svelte";

  const panelShortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("companion"), isMac));
  const quickShortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("quickAsk"), isMac));

  /** Stands for "whichever is installed" in the dropdown, whose values
   * can't be empty. */
  const ANY_ENGINE = "any";
  const engineOptions = $derived([
    { value: ANY_ENGINE, label: t("preferences.companion.quickEngineAny") },
    ...TOOL_IDS.map((tool) => ({ value: tool, label: TOOL_DISPLAY_NAMES[tool] })),
  ]);
</script>

<section>
  <h3>{t("companion.title")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.companion.enabled")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.companionEnabled}
      onchange={(e) => appConfigStore.setCompanionEnabled(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.companion.enabledHint", { shortcut: panelShortcut })}</p>
</section>

{#if appConfigStore.companionEnabled}
  <section>
    <h3>{t("preferences.companion.notices")}</h3>
    <label class="toggle-row">
      <span>{t("preferences.companion.nudges")}</span>
      <input
        type="checkbox"
        checked={appConfigStore.companionNudges}
        onchange={(e) => appConfigStore.setCompanionNudges(e.currentTarget.checked)}
      />
    </label>
    <p class="setting-hint">{t("preferences.companion.nudgesHint")}</p>
    {#if appConfigStore.companionNudges}
      <div class="group">
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
      </div>
    {/if}
  </section>

  <section>
    <h3>{t("companion.quick.title")}</h3>
    <label class="toggle-row">
      <span>{t("preferences.companion.quick")}</span>
      <input
        type="checkbox"
        checked={appConfigStore.quickAskEnabled}
        onchange={(e) => appConfigStore.setQuickAskEnabled(e.currentTarget.checked)}
      />
    </label>
    <p class="setting-hint">{t("preferences.companion.quickHint", { shortcut: quickShortcut })}</p>
    {#if appConfigStore.quickAskEnabled}
      <div class="group">
        <div class="row">
          <span>{t("preferences.companion.quickEngine")}</span>
          <SettingDropdown
            options={engineOptions}
            value={appConfigStore.quickAskEngine || ANY_ENGINE}
            onpick={(value) => appConfigStore.setQuickAskEngine(value === ANY_ENGINE ? "" : value)}
          />
        </div>
        <p class="setting-hint">{t("preferences.companion.quickEngineHint")}</p>
        <label class="toggle-row">
          <span>{t("preferences.companion.quickWithExchange")}</span>
          <input
            type="checkbox"
            checked={appConfigStore.quickAskWithExchange}
            onchange={(e) => appConfigStore.setQuickAskWithExchange(e.currentTarget.checked)}
          />
        </label>
        <p class="setting-hint">{t("preferences.companion.quickWithExchangeHint")}</p>
      </div>
    {/if}
  </section>
{/if}

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

  .toggle-row,
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 6px;
  }

  .toggle-row {
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
    line-height: 1.5;
  }

  /* What a switch above turns on, set in from it. */
  .group {
    margin-top: 10px;
    padding-left: 10px;
    border-left: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .group .setting-hint {
    margin: 0 0 10px;
  }
</style>
