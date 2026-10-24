<script lang="ts">
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import { t } from "$lib/i18n/i18n.svelte";

  /** The user's own agents at work, and how many of them are waiting on an
   * answer; a click opens their panel. */
  const running = $derived(Object.values(agentsStore.lastRun).filter((run) => run.status === "running").length);
  const waiting = $derived(agentsStore.questions.length);
</script>

{#if running > 0 || waiting > 0}
  <button class="status-item" title={t("statusBar.items.agentsRunningHint")} onclick={() => agentsStore.toggleSection("local")}>
    <span class="status-dot" style="background:{waiting > 0 ? '#e3b341' : '#3a6df0'}" aria-hidden="true"></span>
    {waiting > 0 ? t("statusBar.agentsWaiting", { n: String(waiting) }) : t("statusBar.agentsRunning", { n: String(running) })}
  </button>
{/if}
