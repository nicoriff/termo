<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { tabAgentSessions } from "$lib/ai-mode/tab-sessions.svelte";
  import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { companionStore, PANEL_WIDTH } from "./companion-store.svelte";
  import {
    dearestTurns,
    FINDING_TEXT,
    findingAmount,
    findingParams,
    formatSpan,
    formatTokens,
    formatUsd,
    sessionCost,
    TOKEN_PARTS,
    tokenShares,
    totalTokens,
  } from "./companion";

  /** How many of the costliest turns are listed. */
  const DEAREST = 5;

  const report = $derived(companionStore.report);
  const session = $derived(report?.session ?? null);
  const cost = $derived(session ? sessionCost(session) : null);
  const shares = $derived(session ? tokenShares(session.tokens) : null);
  const dearest = $derived(session ? dearestTurns(session, DEAREST) : []);
  const typedTurns = $derived(session ? session.turns.filter((turn) => !turn.automatic).length : 0);

  /** The agent conversation in the tab being looked at, if it has one. */
  const target = $derived.by(() => {
    const tabId = workspaceStore.focusedSessionId;
    const current = tabId ? tabAgentSessions.byTab[tabId] : undefined;
    return current ? { tool: current.tool, sessionId: current.agentSessionId } : null;
  });
  const targetKey = $derived(target ? `${target.tool}:${target.sessionId}` : "");

  /** Whether that tab's agent is in the middle of answering. */
  const working = $derived.by(() => {
    const tabId = workspaceStore.focusedSessionId;
    return tabId ? aiStatusStore.byId[tabId]?.status === "running" : false;
  });

  // Read again when the tab looked at changes, and each time its agent
  // finishes a turn: that's when the session's file has something new.
  $effect(() => {
    void targetKey;
    if (working) return;
    void companionStore.load(target);
  });

  let resizing = $state(false);

  /** Dragging the left edge: the width follows the pointer, and is
   * remembered once it's let go. */
  function startResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startWidth = companionStore.width;
    resizing = true;
    const move = (e: PointerEvent) => companionStore.setWidth(startWidth - (e.clientX - startX));
    const end = () => {
      resizing = false;
      companionStore.setWidth(companionStore.width, true);
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
    if (event.key === "ArrowLeft") companionStore.setWidth(companionStore.width + step, true);
    else if (event.key === "ArrowRight") companionStore.setWidth(companionStore.width - step, true);
    else return;
    event.preventDefault();
  }

  function close() {
    uiState.closeCompanion();
  }

  /** Only while focus is inside the panel. */
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    close();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<aside
  class="cp-panel"
  class:resizing
  style="--cp-width:{companionStore.width}px"
  aria-label={t("companion.title")}
  onkeydown={onKeydown}
  transition:slide={{ axis: "x", duration: 180, easing: cubicOut }}
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="resize-handle"
    role="separator"
    aria-orientation="vertical"
    aria-label={t("companion.resize")}
    aria-valuemin={PANEL_WIDTH.min}
    aria-valuemax={PANEL_WIDTH.max}
    aria-valuenow={companionStore.width}
    title={t("companion.resize")}
    tabindex="0"
    onpointerdown={startResize}
    ondblclick={() => companionStore.setWidth(PANEL_WIDTH.initial, true)}
    onkeydown={onResizeKey}
  ></div>

  <header class="cp-header">
    <h1>{t("companion.title")}</h1>
    <div class="header-actions">
      <button onclick={() => void companionStore.load(target)} disabled={companionStore.loading}>
        {t("companion.refresh")}
      </button>
      <button class="icon" onclick={close} aria-label={t("companion.close")}>✕</button>
    </div>
  </header>

  <div class="cp-body" aria-busy={companionStore.loading}>
    {#if companionStore.failed && !session}
      <p class="note error" role="status">{t("companion.failed")}</p>
    {:else if !session && companionStore.empty}
      <p class="note" role="status">{t("companion.empty")}</p>
    {:else if !session}
      <p class="note" role="status">{t("companion.loading")}</p>
    {:else}
      {#if companionStore.latest}
        <p class="note">{t("companion.latestNote")}</p>
      {/if}

      <section class="card">
        <div class="session-top">
          <span class="session-kind" title={TOOL_DISPLAY_NAMES[session.tool]}><ConsoleIcon kind={session.tool} size={16} /></span>
          <span class="session-title" title={session.title ?? ""}>{session.title ?? t("companion.untitled")}</span>
        </div>
        {#if session.cwd}
          <p class="meta path" title={session.cwd}>{session.cwd}</p>
        {/if}

        <div class="figures">
          <div class="figure">
            <span class="label">{t("companion.cost")}</span>
            <span class="big">{cost ? formatUsd(cost.usd) : "—"}</span>
          </div>
          <div class="figure">
            <span class="label">{t("companion.tokens")}</span>
            <span class="big">{formatTokens(totalTokens(session.tokens))}</span>
          </div>
        </div>
        <p class="meta" title={t("companion.costHint")}>
          {#if cost?.source === "reported"}
            {t("companion.costReported", { tool: TOOL_DISPLAY_NAMES[session.tool] })}
          {:else if cost}
            {t("companion.costEstimated", { date: report?.pricesAsOf ?? "" })}
          {:else}
            {t("companion.costUnknown")}
          {/if}
        </p>

        {#if shares}
          <div class="bar" aria-hidden="true">
            {#each TOKEN_PARTS as part (part)}
              <span class="segment {part}" style="width:{shares[part]}%"></span>
            {/each}
          </div>
          <ul class="legend">
            {#each TOKEN_PARTS as part (part)}
              <li>
                <span class="swatch {part}" aria-hidden="true"></span>
                <span class="legend-label">{t(`companion.parts.${part}`)}</span>
                <span class="value">{formatTokens(session.tokens[part])}</span>
              </li>
            {/each}
          </ul>
        {/if}

        <p class="meta">
          {t("companion.turns")}: {typedTurns} · {t("companion.duration")}: {formatSpan(session.endedAt - session.startedAt)}
          {#if session.compactions.length > 0}
            · {t("companion.compactions")}: {session.compactions.length}
          {/if}
          {#if totalTokens(session.subagentTokens) > 0}
            · {t("companion.subagents")}: {formatTokens(totalTokens(session.subagentTokens))}
          {/if}
        </p>
      </section>

      <section class="list">
        <h2>{t("companion.findingsTitle")}</h2>
        {#if report && report.findings.length === 0}
          <p class="note">{t("companion.noFindings")}</p>
        {/if}
        {#each report?.findings ?? [] as finding (finding.kind)}
          {@const text = FINDING_TEXT[finding.kind]}
          {@const amount = findingAmount(finding)}
          <article class="finding {finding.severity}">
            <div class="finding-top">
              <span class="finding-title">{t(text.title)}</span>
              <span class="badge {finding.severity}">{t(`companion.severity.${finding.severity}`)}</span>
            </div>
            <p class="finding-text">{t(text.text, findingParams(finding))}</p>
            {#if amount}
              <p class="amount">
                {t(amount.kind === "wasted" ? "companion.wasted" : "companion.spent", { amount: formatUsd(amount.usd) })}
              </p>
            {/if}
            <p class="tip"><span class="tip-label">{t("companion.tip")}:</span> {t(text.tip)}</p>
          </article>
        {/each}
      </section>

      {#if dearest.length > 0}
        <section class="list">
          <h2>{t("companion.turnsTitle")}</h2>
          {#each dearest as { index, turn } (index)}
            <article class="turn">
              <div class="turn-top">
                <span class="turn-prompt" class:automatic={turn.automatic} title={turn.prompt}>
                  {turn.automatic ? t("companion.automaticTurn") : turn.prompt}
                </span>
                <span class="value">{turn.costUsd != null ? formatUsd(turn.costUsd) : "—"}</span>
              </div>
              <p class="meta">
                {t("companion.turnMeta", {
                  requests: String(turn.requestCount),
                  tools: String(turn.toolCalls),
                  tokens: formatTokens(totalTokens(turn.tokens)),
                })}
                {#if turn.model}· {turn.model}{/if}
              </p>
            </article>
          {/each}
        </section>
      {/if}
    {/if}
  </div>
</aside>

<style>
  .cp-panel {
    position: relative;
    flex-shrink: 0;
    width: var(--cp-width, 400px);
    display: flex;
    flex-direction: column;
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    border-left: 1px solid var(--tabbar-border, #2a2a2a);
    overflow: hidden;
    font-size: 12px;
    /* What each kind of token is drawn in, in the bar and its legend. */
    --cp-input: #4aa3ff;
    --cp-cache-write: #f5a623;
    --cp-cache-read: #5fb878;
    --cp-output: #c586c0;
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
  .cp-panel.resizing .resize-handle {
    background: var(--divider-hover-bg, #3a6df0);
    opacity: 0.6;
  }

  /* While dragging, the pointer shouldn't select text or flicker back to an
     arrow over the consoles. */
  .cp-panel.resizing {
    user-select: none;
  }

  :global(body:has(.cp-panel.resizing)) {
    cursor: ew-resize;
  }

  .cp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 6px;
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

  .cp-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* Still showing the last reading while the next one comes in. */
  .cp-body[aria-busy="true"] {
    opacity: 0.75;
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

  .card,
  .finding,
  .turn {
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .session-top,
  .finding-top,
  .turn-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .session-kind {
    flex-shrink: 0;
    display: inline-flex;
  }

  .session-title,
  .finding-title {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .session-title,
  .turn-prompt,
  .path {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .figures {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .figure {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .label {
    font-size: 11px;
    opacity: 0.8;
  }

  .big {
    font-size: 20px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .value {
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .meta {
    font-size: 11px;
    opacity: 0.6;
    margin: 0;
  }

  .bar {
    display: flex;
    height: 8px;
    border-radius: 4px;
    overflow: hidden;
    background: var(--tabbar-border, #2a2a2a);
  }

  .segment.input,
  .swatch.input {
    background: var(--cp-input);
  }

  .segment.cacheWrite,
  .swatch.cacheWrite {
    background: var(--cp-cache-write);
  }

  .segment.cacheRead,
  .swatch.cacheRead {
    background: var(--cp-cache-read);
  }

  .segment.output,
  .swatch.output {
    background: var(--cp-output);
  }

  .legend {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px 12px;
  }

  .legend li {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .legend-label {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    opacity: 0.8;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .swatch {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }

  .finding.warning {
    border-color: color-mix(in srgb, #f5a623 60%, transparent);
  }

  .finding.high {
    border-color: color-mix(in srgb, #f14c4c 60%, transparent);
  }

  .finding-text,
  .tip,
  .amount {
    margin: 0;
    line-height: 1.45;
  }

  .amount {
    font-weight: 600;
  }

  .tip {
    opacity: 0.85;
  }

  .tip-label {
    font-weight: 600;
  }

  .badge {
    flex-shrink: 0;
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    white-space: nowrap;
  }

  .badge.warning {
    color: #f5a623;
    border-color: color-mix(in srgb, #f5a623 60%, transparent);
  }

  .badge.high {
    color: #f14c4c;
    border-color: color-mix(in srgb, #f14c4c 60%, transparent);
  }

  .turn-prompt {
    flex: 1;
    min-width: 0;
  }

  .turn-prompt.automatic {
    opacity: 0.6;
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
