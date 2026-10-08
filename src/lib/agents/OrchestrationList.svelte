<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import type { AgentTone } from "./agent-spec";
  import { orchestrationStore } from "./orchestration-store.svelte";
  import { STATUS_KEYS, progressOf, statusTone } from "./orchestration-spec";
  import { stepLine } from "./planning-progress";

  /** What the orchestrator is doing now, while it plans. */
  const lastStep = $derived.by(() => {
    const steps = orchestrationStore.planningSteps;
    return steps.length > 0 ? stepLine(steps[steps.length - 1], t) : null;
  });

  /** Finished ones out of the way: the list is about what's left to do.
   * Remembered per computer. */
  const HIDE_KEY = "termo.orchestration.hideDone";
  function loadHide(): boolean {
    try {
      return localStorage.getItem(HIDE_KEY) === "1";
    } catch {
      return false;
    }
  }
  let hideDone = $state(loadHide());

  function setHideDone(hide: boolean) {
    hideDone = hide;
    try {
      localStorage.setItem(HIDE_KEY, hide ? "1" : "0");
    } catch {
      // Only a convenience.
    }
  }

  const doneCount = $derived(orchestrationStore.all.filter((item) => item.status === "done").length);
  const shown = $derived(
    hideDone ? orchestrationStore.all.filter((item) => item.status !== "done") : orchestrationStore.all,
  );

  function toneColor(tone: AgentTone): string {
    const c = themeStore.current.colors;
    if (tone === "running") return c.blue;
    if (tone === "success") return c.green;
    if (tone === "error") return c.red;
    if (tone === "paused") return c.yellow;
    return c.brightBlack;
  }
</script>

{#if doneCount > 0}
  <label class="filter">
    <input type="checkbox" checked={hideDone} onchange={(e) => setHideDone(e.currentTarget.checked)} />
    {t("agents.orchestration.list.hideDone", { n: String(doneCount) })}
  </label>
{/if}

<div class="list">
  {#if orchestrationStore.planning}
    <div class="row planning" aria-live="polite">
      <span class="icon">🧭</span>
      <span class="text">
        <span class="state">{t("agents.orchestration.dialog.planning")}</span>
        {#if lastStep}<span class="step-now">{lastStep.icon} {lastStep.text}</span>{/if}
      </span>
    </div>
  {/if}
  {#each shown as orchestration (orchestration.id)}
    {@const progress = progressOf(orchestration.plan)}
    <button
      class="row"
      title={orchestration.prompt}
      onclick={() => workspaceStore.openOrchestration(orchestration.id, orchestration.name)}
    >
      <span class="icon">{orchestration.icon || "🧭"}</span>
      <span class="text">
        <span class="name">{orchestration.name}</span>
        <span class="state">
          <span
            class="dot"
            class:working={orchestration.status === "running"}
            style="background:{toneColor(statusTone(orchestration.status))}"
          ></span>
          {t(STATUS_KEYS[orchestration.status])}{#if orchestration.status !== "draft"}{" · "}{t("agents.orchestration.progress", {
              done: String(progress.done),
              total: String(progress.total),
            })}{/if}
        </span>
      </span>
    </button>
  {:else}
    {#if hideDone && doneCount > 0}
      <p class="all-hidden">{t("agents.orchestration.list.allHidden", { n: String(doneCount) })}</p>
    {:else if !orchestrationStore.planning}
      <div class="empty">
        <div class="empty-icon">🧭</div>
        <p class="empty-title">{t("agents.orchestration.empty.title")}</p>
        <p class="empty-text">{t("agents.orchestration.empty.text")}</p>
        <button class="empty-button" onclick={() => uiState.openOrchestrationDialog()}>
          {t("agents.orchestration.empty.create")}
        </button>
      </div>
    {/if}
  {/each}
</div>

<style>
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    font-size: 11.5px;
    opacity: 0.8;
    cursor: pointer;
    user-select: none;
  }

  .filter input {
    margin: 0;
    accent-color: var(--pane-active-border, #3a6df0);
  }

  .all-hidden {
    margin: 0;
    padding: 18px 12px;
    font-size: 11.5px;
    text-align: center;
    opacity: 0.6;
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 0;
    scrollbar-width: thin;
  }

  .row {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    width: 100%;
    padding: 8px 8px 8px 12px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  button.row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .row.planning {
    cursor: default;
    animation: pulse 1.4s ease-in-out infinite;
  }

  .icon {
    font-size: 16px;
    line-height: 1.2;
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    opacity: 0.65;
  }

  .step-now {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    opacity: 0.55;
  }

  .planning .state {
    display: block;
    line-height: 1.45;
  }

  .dot {
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    border-radius: 50%;
  }

  .working {
    animation: pulse 1.4s ease-in-out infinite;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 28px 18px;
    text-align: center;
  }

  .empty-icon {
    font-size: 26px;
  }

  .empty-title {
    margin: 8px 0 4px;
    font-weight: 600;
  }

  .empty-text {
    margin: 0 0 14px;
    font-size: 11.5px;
    line-height: 1.5;
    opacity: 0.65;
  }

  .empty-button {
    padding: 6px 14px;
    border: none;
    border-radius: 5px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font: inherit;
    cursor: pointer;
  }

  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }
</style>
