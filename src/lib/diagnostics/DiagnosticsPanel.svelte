<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { killPtyTree } from "$lib/core/pty-client";
  import { collectSessions, countSessions } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { getTabId } from "$lib/terminal/pty-registry";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import { diagnosticsStore, PANEL_WIDTH } from "./diagnostics-store.svelte";
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
        // What the tab itself shows: the agent running in it, else its shell.
        kind: tabTitleStore.kindFor(tab.session),
        type: tabTitleStore.typeFor(tab.session),
      });
    }
    return sortByUsage(found, sort);
  });
  const elsewhere = $derived(report ? report.consoles.length - rows.length : 0);

  // Nothing takes focus on opening: the panel sits beside the consoles, which
  // keep the keyboard until the user clicks into it.
  onMount(() => diagnosticsStore.start());
  onDestroy(() => diagnosticsStore.stop());

  let resizing = $state(false);

  /** Dragging the left edge: the width follows the pointer, and is
   * remembered once it's let go. */
  function startResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startWidth = diagnosticsStore.width;
    resizing = true;
    const move = (e: PointerEvent) => diagnosticsStore.setWidth(startWidth - (e.clientX - startX));
    const end = () => {
      resizing = false;
      diagnosticsStore.setWidth(diagnosticsStore.width, true);
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  }

  function onResizeKey(event: KeyboardEvent) {
    const step = event.shiftKey ? 40 : 10;
    if (event.key === "ArrowLeft") diagnosticsStore.setWidth(diagnosticsStore.width + step, true);
    else if (event.key === "ArrowRight") diagnosticsStore.setWidth(diagnosticsStore.width - step, true);
    else return;
    event.preventDefault();
  }

  function close() {
    uiState.closeDiagnostics();
  }

  /** Only while focus is inside the panel: Escape backs out of a pending
   * "Kill", then closes it. */
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    if (confirming) confirming = null;
    else close();
  }

  /** The panel stays: it's there to watch while working in the tab. */
  function goTo(tabId: string) {
    workspaceStore.focusSession(tabId);
    uiState.refocusTerminals();
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

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<aside
  class="dg-panel"
  class:resizing
  style="--dg-width:{diagnosticsStore.width}px"
  aria-label={t("diagnostics.title")}
  onkeydown={onKeydown}
  transition:slide={{ axis: "x", duration: 180, easing: cubicOut }}
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="resize-handle"
    role="separator"
    aria-orientation="vertical"
    aria-label={t("diagnostics.resize")}
    aria-valuemin={PANEL_WIDTH.min}
    aria-valuemax={PANEL_WIDTH.max}
    aria-valuenow={diagnosticsStore.width}
    title={t("diagnostics.resize")}
    tabindex="0"
    onpointerdown={startResize}
    ondblclick={() => diagnosticsStore.setWidth(PANEL_WIDTH.initial, true)}
    onkeydown={onResizeKey}
  ></div>

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
              <span class="tab-kind" title={row.type}><ConsoleIcon kind={row.kind} size={16} /></span>
              <span class="tab-title" title={row.title}>{row.title}</span>
              <span class="tab-type">{row.type}</span>
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
</aside>

<style>
  .dg-panel {
    position: relative;
    flex-shrink: 0;
    width: var(--dg-width, 380px);
    display: flex;
    flex-direction: column;
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    border-left: 1px solid var(--tabbar-border, #2a2a2a);
    overflow: hidden;
    font-size: 12px;
  }

  .resize-handle {
    position: absolute;
    top: 0;
    left: 0;
    bottom: 0;
    z-index: 25;
    width: 6px;
    cursor: ew-resize;
    outline: none;
  }

  .resize-handle:hover,
  .resize-handle:focus-visible,
  .dg-panel.resizing .resize-handle {
    background: var(--divider-hover-bg, #3a6df0);
    opacity: 0.6;
  }

  /* While dragging, the pointer shouldn't select text or flicker back to an
     arrow over the consoles. */
  .dg-panel.resizing {
    user-select: none;
  }

  :global(body:has(.dg-panel.resizing)) {
    cursor: ew-resize;
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

  .tab-kind {
    flex-shrink: 0;
    display: inline-flex;
  }

  .tab-type {
    flex-shrink: 0;
    font-size: 11px;
    opacity: 0.6;
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
