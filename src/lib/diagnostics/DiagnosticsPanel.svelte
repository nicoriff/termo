<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { killPtyTree } from "$lib/core/pty-client";
  import { collectSessions, countSessions } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { getTabId } from "$lib/terminal/pty-registry";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import { diagnosticsStore } from "./diagnostics-store.svelte";
  import {
    cpuOfMachine,
    formatBytes,
    formatPercent,
    formatUptime,
    isHeavy,
    sortByUsage,
    type SortMode,
  } from "./diagnostics";
  import Sparkline from "./Sparkline.svelte";

  let panelEl = $state<HTMLDivElement | undefined>(undefined);
  let sort = $state<SortMode>("cpu");
  /** The tab whose "Kill" is waiting for a yes. */
  let confirming = $state<string | null>(null);

  const report = $derived(diagnosticsStore.latest);
  const tabCount = $derived(workspaceStore.root ? countSessions(workspaceStore.root) : 0);

  /** This window's consoles, matched to their tabs. The backend reports
   * every window's; the others have no tab here to go to or close. */
  const rows = $derived.by(() => {
    if (!report || !workspaceStore.root) return [];
    const tabs = collectSessions(workspaceStore.root);
    const found = [];
    for (const usage of report.consoles) {
      const tabId = getTabId(usage.id);
      const tab = tabId ? tabs.find((entry) => entry.session.id === tabId) : undefined;
      if (!tab) continue;
      found.push({
        ...usage,
        tabId: tab.session.id,
        tileId: tab.tileId,
        title: tabTitleStore.titleFor(tab.session.id, tab.session.title),
      });
    }
    return sortByUsage(found, sort);
  });
  const elsewhere = $derived(report ? report.consoles.length - rows.length : 0);

  onMount(() => {
    diagnosticsStore.start();
    panelEl?.focus();
  });
  onDestroy(() => diagnosticsStore.stop());

  function close() {
    uiState.closeDiagnostics();
  }

  function onKeydown(event: KeyboardEvent) {
    // The panel owns the keyboard while it's open (MainWindow's window-level
    // handler bails out on overlayOpen), so nothing here reaches the console.
    event.stopPropagation();
    if (event.key !== "Escape") return;
    event.preventDefault();
    if (confirming) confirming = null;
    else close();
  }

  function goTo(tabId: string) {
    workspaceStore.focusSession(tabId);
    close();
  }

  function closeTab(row: { tileId: string; tabId: string }) {
    workspaceStore.closeTab(row.tileId, row.tabId);
  }

  async function kill(row: { id: string; tileId: string; tabId: string }) {
    confirming = null;
    await killPtyTree(row.id).catch(() => {});
    // Closing ends the shell too; the only tab can't close, so it keeps it.
    workspaceStore.closeTab(row.tileId, row.tabId);
  }

  const peak = (series: readonly number[]) => (series.length ? Math.max(...series) : 0);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dg-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="dg-panel"
  role="dialog"
  tabindex="-1"
  aria-label={t("diagnostics.title")}
  bind:this={panelEl}
  onpointerdown={(event) => event.stopPropagation()}
  onkeydown={onKeydown}
  transition:fly={{ x: 420, duration: 180, easing: cubicOut }}
>
  <header class="dg-header">
    <h1>{t("diagnostics.title")}</h1>
    <button class="icon" onclick={close} aria-label={t("diagnostics.close")}>✕</button>
  </header>

  <div class="dg-body">
    {#if diagnosticsStore.failed && !report}
      <p class="note error" role="status">{t("diagnostics.failed")}</p>
    {:else if !report}
      <p class="note" role="status">{t("diagnostics.loading")}</p>
    {:else}
      <section class="card">
        <h2>{t("diagnostics.appLabel")}</h2>
        <div class="charts">
          <div class="chart">
            <div class="chart-head">
              <span>{t("diagnostics.cpu")}</span>
              <span class="value"
                >{formatPercent(cpuOfMachine(report.app.cpu, report.cores))}</span
              >
            </div>
            <div class="plot">
              <Sparkline values={diagnosticsStore.appCpu} ceiling={100} label={t("diagnostics.chartCpu")} />
            </div>
            <div class="chart-foot">
              {t("diagnostics.peak", { value: formatPercent(peak(diagnosticsStore.appCpu)) })}
            </div>
          </div>
          <div class="chart">
            <div class="chart-head">
              <span>{t("diagnostics.memory")}</span>
              <span class="value">{formatBytes(report.app.memory)}</span>
            </div>
            <div class="plot">
              <Sparkline values={diagnosticsStore.appMemory} label={t("diagnostics.chartMemory")} />
            </div>
            <div class="chart-foot">
              {t("diagnostics.peak", { value: formatBytes(peak(diagnosticsStore.appMemory)) })}
            </div>
          </div>
        </div>
        <p class="meta">
          {t("diagnostics.processes", { count: String(report.app.processes) })} ·
          {t("diagnostics.uptime", { time: formatUptime(report.app.uptimeSecs) })}
        </p>
      </section>

      <section class="card">
        <h2>{t("diagnostics.system")}</h2>
        <div class="charts">
          <div class="chart">
            <div class="chart-head">
              <span>{t("diagnostics.cpu")}</span>
              <span class="value">{formatPercent(report.system.cpu)}</span>
            </div>
            <div class="plot">
              <Sparkline values={diagnosticsStore.systemCpu} ceiling={100} label={t("diagnostics.chartCpu")} />
            </div>
            <div class="chart-foot">
              {t("diagnostics.peak", { value: formatPercent(peak(diagnosticsStore.systemCpu)) })}
            </div>
          </div>
          <div class="chart">
            <div class="chart-head">
              <span>{t("diagnostics.memory")}</span>
              <span class="value">{formatPercent((report.system.memoryUsed / report.system.memoryTotal) * 100)}</span>
            </div>
            <div class="plot">
              <Sparkline
                values={diagnosticsStore.systemMemory}
                ceiling={report.system.memoryTotal}
                label={t("diagnostics.chartMemory")}
              />
            </div>
            <div class="chart-foot">
              {t("diagnostics.systemMemory", {
                used: formatBytes(report.system.memoryUsed),
                total: formatBytes(report.system.memoryTotal),
              })}
            </div>
          </div>
        </div>
        <p class="meta">{t("diagnostics.cpuHint", { cores: String(report.cores) })}</p>
      </section>

      <section class="tabs">
        <div class="tabs-head">
          <h2>{t("diagnostics.tabs")}</h2>
          <div class="sort" role="group" aria-label={t("diagnostics.sortBy")}>
            <button class="chip" class:on={sort === "cpu"} onclick={() => (sort = "cpu")}>
              {t("diagnostics.sortCpu")}
            </button>
            <button class="chip" class:on={sort === "memory"} onclick={() => (sort = "memory")}>
              {t("diagnostics.sortMemory")}
            </button>
          </div>
        </div>

        {#if rows.length === 0}
          <p class="note">{t("diagnostics.empty")}</p>
        {/if}

        {#each rows as row (row.id)}
          {@const history = diagnosticsStore.byConsole[row.id]}
          <article class="tab" class:heavy={isHeavy(row)}>
            <div class="tab-top">
              <span class="tab-title" title={row.title}>{row.title}</span>
              {#if isHeavy(row)}<span class="badge">{t("diagnostics.heavy")}</span>{/if}
            </div>
            <div class="tab-stats">
              <div class="figures">
                <span><b>{formatPercent(cpuOfMachine(row.cpu, report.cores))}</b> {t("diagnostics.cpu")}</span>
                <span><b>{formatBytes(row.memory)}</b></span>
              </div>
              <div class="mini">
                <Sparkline values={history?.cpu ?? []} ceiling={100} label={t("diagnostics.chartCpu")} />
              </div>
            </div>
            <p class="meta">
              {t("diagnostics.running", { name: row.running })} ·
              {t("diagnostics.processes", { count: String(row.processes) })} ·
              {t("diagnostics.uptime", { time: formatUptime(row.uptimeSecs) })}
            </p>

            {#if confirming === row.id}
              <div class="confirm" role="alert">
                <span>{tabCount > 1 ? t("diagnostics.killConfirm") : t("diagnostics.killConfirmLast")}</span>
                <div class="actions">
                  <button onclick={() => (confirming = null)}>{t("diagnostics.cancel")}</button>
                  <button class="danger" onclick={() => void kill(row)}>{t("diagnostics.killYes")}</button>
                </div>
              </div>
            {:else}
              <div class="actions">
                <button onclick={() => goTo(row.tabId)}>{t("diagnostics.goTo")}</button>
                <button
                  disabled={tabCount <= 1}
                  title={tabCount <= 1 ? t("diagnostics.onlyTab") : undefined}
                  onclick={() => closeTab(row)}>{t("diagnostics.closeTab")}</button
                >
                <button class="danger" onclick={() => (confirming = row.id)}>{t("diagnostics.kill")}</button>
              </div>
            {/if}
          </article>
        {/each}

        {#if elsewhere > 0}
          <p class="meta">{t("diagnostics.otherWindows", { count: String(elsewhere) })}</p>
        {/if}
      </section>
    {/if}
  </div>
</div>

<style>
  .dg-backdrop {
    position: absolute;
    inset: 0;
    z-index: 599;
    background: rgba(0, 0, 0, 0.35);
  }

  .dg-panel {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 600;
    width: min(420px, 100%);
    display: flex;
    flex-direction: column;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    border-left: 1px solid var(--tabbar-border, #2a2a2a);
    box-shadow: -8px 0 32px rgba(0, 0, 0, 0.45);
    overflow: hidden;
    outline: none;
  }

  .dg-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  h1 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  h2 {
    margin: 0;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
  }

  .dg-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  button {
    font: inherit;
    font-size: 12px;
    color: inherit;
    background: transparent;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    padding: 3px 9px;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  button.icon {
    border: none;
    padding: 2px 6px;
  }

  button.danger {
    color: #f14c4c;
    border-color: color-mix(in srgb, #f14c4c 45%, transparent);
  }

  .card,
  .tab {
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tab.heavy {
    border-color: color-mix(in srgb, #f5a623 60%, transparent);
  }

  .charts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .chart {
    min-width: 0;
  }

  .chart-head {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    opacity: 0.8;
  }

  .value {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    opacity: 1;
  }

  .plot {
    height: 40px;
    margin: 4px 0;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .chart-foot,
  .meta {
    font-size: 11px;
    opacity: 0.6;
    margin: 0;
  }

  .tabs {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tabs-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .sort {
    display: flex;
    gap: 4px;
  }

  .chip.on {
    background: var(--pane-active-border, #3a6df0);
    border-color: transparent;
    color: #fff;
  }

  .tab-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .tab-title {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 8px;
    color: #f5a623;
    border: 1px solid color-mix(in srgb, #f5a623 60%, transparent);
    white-space: nowrap;
  }

  .tab-stats {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .figures {
    display: flex;
    flex-direction: column;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    min-width: 96px;
  }

  .mini {
    flex: 1;
    height: 28px;
  }

  .actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }

  .confirm {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
  }

  .note {
    font-size: 12px;
    opacity: 0.7;
    margin: 0;
  }

  .note.error {
    color: #f14c4c;
    opacity: 1;
  }
</style>
