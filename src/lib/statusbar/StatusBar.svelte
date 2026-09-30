<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { getPtyMemory } from "$lib/core/pty-client";
  import { collectTiles, isConsoleTab } from "$lib/layout/split-tree";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { getPtyId } from "$lib/terminal/pty-registry";
  import { t } from "$lib/i18n/i18n.svelte";
  import { agentStatusStore } from "$lib/ai-mode/agent-status-store.svelte";
  import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
  import { planWindowLabel } from "$lib/ai-mode/plan-window-label";
  import { REPORTS_PLAN_USAGE, TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";

  const MEMORY_POLL_MS = 2000;

  const activeSession = $derived.by(() => {
    if (!workspaceStore.root || !workspaceStore.focusedTileId) return null;
    const tile = collectTiles(workspaceStore.root).find((t) => t.id === workspaceStore.focusedTileId);
    return tile?.sessions.find((session) => session.id === tile.activeSessionId) ?? null;
  });
  const activeTabId = $derived(activeSession?.id ?? null);
  /** The active tab's console, when it is one (not a file or an agent's runs). */
  const activeConsole = $derived(activeSession && isConsoleTab(activeSession) ? activeSession : null);

  let memoryBytes = $state<number | null>(null);

  $effect(() => {
    const tabId = activeTabId;
    memoryBytes = null;
    if (!tabId) return;

    let cancelled = false;
    async function sample(tabId: string) {
      // No PTY id yet while the tab's shell is still spawning — the next
      // poll picks it up.
      const ptyId = getPtyId(tabId);
      if (!ptyId) return;
      try {
        const bytes = await getPtyMemory(ptyId);
        if (!cancelled) memoryBytes = bytes;
      } catch {
        if (!cancelled) memoryBytes = null;
      }
    }

    void sample(tabId);
    const timer = setInterval(() => void sample(tabId), MEMORY_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  });

  // Which agent Modo IA detected in the active console, by hooks or by
  // reading the screen. Null when there's none (or Modo IA is off, which is
  // what stops both sources from reporting).
  const activeTool = $derived(activeTabId ? (aiStatusStore.byId[activeTabId]?.tool ?? null) : null);

  // The active console's plan windows, whichever agent reported them and
  // whatever windows its plan has — named after how long each one runs.
  const planWindows = $derived.by(() => {
    const status = activeTabId ? agentStatusStore.byTab[activeTabId] : null;
    return (status?.planWindows ?? []).map((usage) => {
      const label = planWindowLabel(usage.windowMinutes);
      return { label: t(label.key, label.params), percent: usage.percentage };
    });
  });

  // Claude only: Codex reports no cost for a subscription, so the item
  // simply isn't there for it.
  const costUsd = $derived(activeTabId ? (agentStatusStore.byTab[activeTabId]?.costUsd ?? null) : null);

  /** Cents are enough for a session total, but a session that has only
   * just started would read "$0.00" as if it were free. */
  function formatUsd(value: number): string {
    return value > 0 && value < 0.01 ? "<$0.01" : `$${value.toFixed(2)}`;
  }

  /** The gauge can't run past its track; the number still shows what was
   * reported (a spend limit can go over 100%). */
  function clamp(percent: number): number {
    return Math.min(100, Math.max(0, percent));
  }

  function formatBytes(bytes: number): string {
    const mb = bytes / (1024 * 1024);
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
  }
</script>

<div class="status-bar">
  <span class="status-item">
    {t("statusBar.aiMode", { state: t(appConfigStore.aiModeEnabled ? "statusBar.aiOn" : "statusBar.aiOff") })}
  </span>
  {#if activeConsole}
    <!-- The console's type: the tab shows it only as an icon. -->
    <span class="status-item status-engine" title={activeTool ? t("statusBar.engineHint") : t("statusBar.consoleHint")}>
      <ConsoleIcon kind={tabTitleStore.kindFor(activeConsole)} size={11} />
      {activeTool ? TOOL_DISPLAY_NAMES[activeTool] : tabTitleStore.typeFor(activeConsole)}
    </span>
  {/if}
  <!-- Plan usage and cost are Modo IA's: only with it on. -->
  {#if appConfigStore.aiModeEnabled && ((activeTool && REPORTS_PLAN_USAGE[activeTool]) || planWindows.length > 0)}
    <span class="status-item status-plan" title={t("statusBar.planHint")}>
      {t("statusBar.plan")}
      {#each planWindows as usage (usage.label)}
        <span class="plan-window">
          <span class="plan-label">{usage.label}</span>
          <span class="plan-gauge" style="--fill:{clamp(usage.percent)}%">
            <span class="plan-gauge-text">{Math.round(usage.percent)}%</span>
          </span>
        </span>
      {:else}
        <!-- No agent reports usage until its first response, and Claude's
             windows need a Pro/Max plan. Saying so beats the reading
             vanishing from the bar as if the console had none. -->
        <span class="plan-pending">{t("statusBar.planPending")}</span>
      {/each}
    </span>
  {/if}
  {#if appConfigStore.aiModeEnabled && costUsd !== null}
    <span class="status-item status-cost" title={t("statusBar.costHint")}>
      {t("statusBar.cost", { value: formatUsd(costUsd) })}
    </span>
  {/if}
  {#if memoryBytes !== null}
    <span class="status-item status-memory" title={t("statusBar.memoryHint")}>
      {t("statusBar.memory", { value: formatBytes(memoryBytes) })}
    </span>
  {/if}
</div>

<style>
  .status-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    height: 22px;
    padding: 0 10px;
    font-size: 11px;
    background: var(--titlebar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  /* Dimmed text only — dimming the whole bar would also lighten its
     background below the opacity setting. */
  .status-item {
    white-space: nowrap;
    opacity: 0.85;
  }

  .status-engine {
    margin-left: 12px;
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .status-plan {
    margin-left: 12px;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .plan-window {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .plan-label {
    opacity: 0.75;
  }

  .plan-pending {
    opacity: 0.6;
  }

  .plan-gauge {
    position: relative;
    display: block;
    width: 52px;
    height: 13px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.12);
    overflow: hidden;
  }

  /* The fill sits behind the number, which stays centered on the whole
     track so it doesn't move as the bar grows. */
  .plan-gauge::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: var(--fill);
    background: var(--pane-active-border, #3a6df0);
    opacity: 0.55;
  }

  .plan-gauge-text {
    position: relative;
    display: block;
    text-align: center;
    font-size: 10px;
    line-height: 13px;
  }

  .status-cost {
    margin-left: 12px;
  }

  .status-memory {
    margin-left: auto;
  }
</style>
