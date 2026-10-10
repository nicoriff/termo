<script lang="ts">
  import type { AgentEngine } from "$lib/core/agents-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "$lib/preferences/SettingDropdown.svelte";
  import { aiEngine } from "./ai-engine.svelte";
  import { ENGINE_NAMES } from "./orchestration-spec";

  /** Which engine drafts with AI, wherever Termo asks one (see
   * ai-engine.svelte.ts). Nothing to pick with a single one installed. */
  let {
    id = "ai-engine",
    label,
    hint,
    disabled = false,
  }: {
    id?: string;
    label?: string;
    /** What this pick is for, where it could be taken for something else. */
    hint?: string;
    disabled?: boolean;
  } = $props();

  const engine = $derived(aiEngine.value);
  const options = $derived(aiEngine.installed.map((option) => ({ value: option, label: ENGINE_NAMES[option] })));
</script>

{#if engine}
  <span class="ai-engine-label" id="{id}-label">{label ?? t("agents.aiEngine.label")}</span>
  <SettingDropdown
    {options}
    value={engine}
    width="min(320px, 100%)"
    labelledby="{id}-label"
    disabled={disabled || options.length < 2}
    onpick={(value) => aiEngine.pick(value as AgentEngine)}
  />
  {#if hint}
    <p class="ai-engine-note">{hint}</p>
  {/if}
  {#if aiEngine.isChecked(engine)}
    <p class="ai-engine-note">{t("agents.aiEngine.checked", { engine: ENGINE_NAMES[engine] })}</p>
  {/if}
{/if}

<style>
  /* The same as a dialog field's own label. */
  .ai-engine-label {
    display: block;
    margin: 14px 0 5px;
    font-weight: 600;
  }

  .ai-engine-note {
    margin: 5px 0 0;
    font-size: 11.5px;
    line-height: 1.5;
    opacity: 0.6;
  }
</style>
