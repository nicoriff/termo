<script lang="ts">
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import { orchestrationStore } from "$lib/agents/orchestration-store.svelte";
  import { t } from "$lib/i18n/i18n.svelte";

  /** The user's own agents at work, and how many of them are waiting on an
   * answer — their own runs' and the orchestrations' — ; a click opens the
   * panel where it's answered. */
  const running = $derived(Object.values(agentsStore.lastRun).filter((run) => run.status === "running").length);
  const waiting = $derived(agentsStore.questions.length + orchestrationStore.waitingQuestions);
  /** Only orchestrations asking: their section, where they're answered. */
  const section = $derived(
    agentsStore.questions.length === 0 && orchestrationStore.waitingQuestions > 0 ? "orchestration" : "local",
  );
</script>

{#if running > 0 || waiting > 0}
  <button class="status-item" title={t("statusBar.items.agentsRunningHint")} onclick={() => agentsStore.toggleSection(section)}>
    <span class="status-dot" style="background:{waiting > 0 ? '#e3b341' : '#3a6df0'}" aria-hidden="true"></span>
    {waiting > 0 ? t("statusBar.agentsWaiting", { n: String(waiting) }) : t("statusBar.agentsRunning", { n: String(running) })}
  </button>
{/if}
