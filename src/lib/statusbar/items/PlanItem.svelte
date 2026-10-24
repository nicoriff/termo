<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { getCopilotPlanUsage, type CopilotPlanUsage } from "$lib/core/copilot-usage-client";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { planWindowLabel } from "$lib/ai-mode/plan-window-label";
  import { REPORTS_PLAN_USAGE } from "$lib/ai-mode/tool-names";
  import { activeConsole } from "../active.svelte";
  import { clampPercent, formatDuration, resetsInMs } from "../format";
  import { optionOn, type StatusItemRef } from "../layout";
  import { ticker } from "../ticker.svelte";

  let { item }: { item: StatusItemRef } = $props();

  /** A plan's monthly count barely moves; the backend keeps it as long. */
  const COPILOT_POLL_MS = 5 * 60 * 1000;

  const showResets = $derived(optionOn(item, "resets"));
  const tool = $derived(activeConsole.tool);

  $effect(() => {
    if (showResets) return ticker.hold();
  });

  // The active console's plan windows, whichever agent reported them and
  // whatever windows its plan has — named after how long each one runs.
  const planWindows = $derived(
    (activeConsole.agent?.planWindows ?? []).map((usage) => {
      const label = planWindowLabel(usage.windowMinutes);
      return { label: t(label.key, label.params), percent: usage.percentage, resetsAt: usage.resetsAt };
    }),
  );

  // Copilot's plan usage comes from GitHub rather than from the console
  // (see src-tauri/src/copilot_quota.rs): asked while a Copilot console is
  // the active one.
  const showCopilotPlan = $derived(appConfigStore.aiModeEnabled && tool === "copilot");
  let copilotUsage = $state<CopilotPlanUsage | null>(null);

  $effect(() => {
    if (!showCopilotPlan) return;
    let cancelled = false;
    const load = () =>
      void getCopilotPlanUsage()
        .then((usage) => {
          if (!cancelled) copilotUsage = usage;
        })
        .catch(() => {});
    load();
    const timer = setInterval(load, COPILOT_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  });

  const copilotWindows = $derived(
    copilotUsage?.kind === "ok"
      ? copilotUsage.quotas.map((quota) => ({
          label: t(quota.id === "chat" ? "statusBar.copilotChat" : "statusBar.copilotPremium"),
          percent: quota.percentUsed,
          resetsAt: null,
        }))
      : [],
  );

  /** "2026-11-01" as a date in the current language, read as a local date. */
  const copilotResets = $derived.by(() => {
    if (copilotUsage?.kind !== "ok" || !copilotUsage.resetsOn) return null;
    const [year, month, day] = copilotUsage.resetsOn.split("-").map(Number);
    if (!year || !month || !day) return null;
    return new Date(year, month - 1, day).toLocaleDateString(i18n.locale, { day: "numeric", month: "long" });
  });

  const planTitle = $derived(
    showCopilotPlan
      ? copilotUsage?.kind === "noGh"
        ? t("statusBar.copilotNeedsGhHint")
        : copilotResets
          ? t("statusBar.copilotPlanHint", { date: copilotResets })
          : t("statusBar.planHint")
      : t("statusBar.planHint"),
  );

  const shownWindows = $derived(showCopilotPlan ? copilotWindows : planWindows);
  const visible = $derived(
    appConfigStore.aiModeEnabled && (showCopilotPlan || (tool && REPORTS_PLAN_USAGE[tool]) || planWindows.length > 0),
  );
</script>

<!-- Plan usage is Modo IA's: only with it on. -->
{#if visible}
  <span class="status-item status-plan" title={planTitle}>
    {t("statusBar.plan")}
    {#each shownWindows as usage (usage.label)}
      {@const resets = showResets ? resetsInMs(usage.resetsAt, ticker.now) : null}
      <span class="plan-window">
        <span class="plan-label">{usage.label}</span>
        <span class="plan-gauge" style="--fill:{clampPercent(usage.percent)}%">
          <span class="plan-gauge-text">{Math.round(usage.percent)}%</span>
        </span>
        {#if resets !== null}
          <span class="status-dim" title={t("statusBar.resetsHint")}>↻ {formatDuration(resets)}</span>
        {/if}
      </span>
    {:else}
      <!-- No agent reports usage until its first response, and Claude's
           windows need a Pro/Max plan. Saying so beats the reading
           vanishing from the bar as if the console had none. -->
      <span class="plan-pending">
        {t(showCopilotPlan && copilotUsage?.kind === "noGh" ? "statusBar.copilotNeedsGh" : "statusBar.planPending")}
      </span>
    {/each}
  </span>
{/if}
