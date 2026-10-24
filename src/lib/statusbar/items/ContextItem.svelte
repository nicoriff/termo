<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { activeConsole } from "../active.svelte";
  import { clampPercent, formatTokens } from "../format";
  import { optionValue, type StatusItemRef } from "../layout";

  let { item }: { item: StatusItemRef } = $props();

  const percent = $derived(activeConsole.agent?.contextUsedPercentage ?? null);
  const tokens = $derived(activeConsole.agent?.contextTokens ?? null);
  const windowTokens = $derived(activeConsole.agent?.contextWindowTokens ?? null);
  /** Tokens when asked for and reported; the share otherwise. */
  const asTokens = $derived(optionValue(item, "format") === "tokens" && tokens !== null);
  const title = $derived(
    tokens !== null && windowTokens !== null
      ? `${t("statusBar.contextHint")}\n${formatTokens(tokens)} / ${formatTokens(windowTokens)}`
      : t("statusBar.contextHint"),
  );
</script>

{#if appConfigStore.aiModeEnabled && percent !== null}
  <span class="status-item status-context" {title}>
    {t("statusBar.context")}
    {#if asTokens && tokens !== null}
      <span>{formatTokens(tokens)}{#if windowTokens !== null}<span class="status-dim"> / {formatTokens(windowTokens)}</span>{/if}</span>
    {:else}
      <span class="plan-gauge" style="--fill:{clampPercent(percent)}%">
        <span class="plan-gauge-text">{Math.round(percent)}%</span>
      </span>
    {/if}
  </span>
{/if}
