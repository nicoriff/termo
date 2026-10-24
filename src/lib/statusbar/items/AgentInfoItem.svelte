<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { activeConsole } from "../active.svelte";
  import { formatDuration, formatUsd } from "../format";
  import type { StatusItemId } from "../layout";

  /** The things an agent reports that are a single reading: one component
   * for all of them, since none keeps anything of its own. */
  let { id }: { id: StatusItemId } = $props();

  const agent = $derived(appConfigStore.aiModeEnabled ? activeConsole.agent : null);
</script>

{#if agent}
  {#if id === "agent.model" && agent.model}
    <span class="status-item" title={t("statusBar.items.modelHint")}>{agent.model}</span>
  {:else if id === "agent.cost" && agent.costUsd != null}
    <!-- Claude only: Codex reports no cost for a subscription, so the item
         simply isn't there for it. -->
    <span class="status-item" title={t("statusBar.costHint")}>{t("statusBar.cost", { value: formatUsd(agent.costUsd) })}</span>
  {:else if id === "agent.spendLimit" && agent.spendLimitPercentage != null}
    <span class="status-item" title={t("statusBar.items.spendLimitHint")}>
      {t("statusBar.spendLimit", { value: `${Math.round(agent.spendLimitPercentage)}%` })}
    </span>
  {:else if id === "agent.lines" && (agent.linesAdded != null || agent.linesRemoved != null)}
    <span class="status-item" title={t("statusBar.items.linesHint")}>
      <span class="lines-added">+{agent.linesAdded ?? 0}</span>
      <span class="lines-removed">−{agent.linesRemoved ?? 0}</span>
    </span>
  {:else if id === "agent.duration" && agent.durationMs != null}
    <span
      class="status-item"
      title={agent.apiDurationMs != null
        ? t("statusBar.durationApiHint", { value: formatDuration(agent.apiDurationMs) })
        : t("statusBar.items.durationHint")}
    >
      ⏱ {formatDuration(agent.durationMs)}
    </span>
  {:else if id === "agent.version" && agent.agentVersion}
    <span class="status-item" title={agent.outputStyle ? t("statusBar.outputStyle", { name: agent.outputStyle }) : t("statusBar.items.versionHint")}>
      v{agent.agentVersion}
    </span>
  {/if}
{/if}
