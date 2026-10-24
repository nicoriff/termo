<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { activeConsole } from "../active.svelte";

  /** What the console's agent is doing, as Modo IA sees it. */
  const entry = $derived(appConfigStore.aiModeEnabled ? activeConsole.aiEntry : null);
  const COLORS = { running: "#3a6df0", awaiting: "#e3b341", error: "#e05a5a", finished: "#3fb950" } as const;
</script>

{#if entry?.tool && entry.status}
  <span class="status-item" title={t("statusBar.items.stateHint")}>
    <span class="status-dot" style="background:{COLORS[entry.status]}" aria-hidden="true"></span>
    {t(`aiMode.status.${entry.status}`)}
  </span>
{/if}
