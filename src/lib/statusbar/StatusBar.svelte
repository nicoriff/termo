<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { getPtyMemory } from "$lib/core/pty-client";
  import { collectTiles } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { getPtyId } from "$lib/terminal/pty-registry";
  import { t } from "$lib/i18n/i18n.svelte";
  import { agentStatusStore } from "$lib/ai-mode/agent-status-store.svelte";

  const MEMORY_POLL_MS = 2000;

  const activeTabId = $derived.by(() => {
    if (!workspaceStore.root || !workspaceStore.focusedTileId) return null;
    const tile = collectTiles(workspaceStore.root).find((t) => t.id === workspaceStore.focusedTileId);
    return tile?.activeSessionId ?? null;
  });

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

  // Claude Code's plan windows for the active console, when it reports them.
  const planUsage = $derived.by(() => {
    const status = activeTabId ? agentStatusStore.byTab[activeTabId] : null;
    if (!status) return null;
    const parts: string[] = [];
    if (status.fiveHourPercentage !== null) {
      parts.push(t("statusBar.fiveHour", { percent: Math.round(status.fiveHourPercentage) }));
    }
    if (status.sevenDayPercentage !== null) {
      parts.push(t("statusBar.sevenDay", { percent: Math.round(status.sevenDayPercentage) }));
    }
    return parts.length > 0 ? t("statusBar.plan", { usage: parts.join(" · ") }) : null;
  });

  function formatBytes(bytes: number): string {
    const mb = bytes / (1024 * 1024);
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
  }
</script>

<div class="status-bar">
  <span class="status-item">
    {t("statusBar.aiMode", { state: t(appConfigStore.aiModeEnabled ? "statusBar.aiOn" : "statusBar.aiOff") })}
  </span>
  {#if planUsage}
    <span class="status-item status-plan" title={t("statusBar.planHint")}>{planUsage}</span>
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

  .status-plan {
    margin-left: 12px;
  }

  .status-memory {
    margin-left: auto;
  }
</style>
