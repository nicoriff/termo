<script lang="ts">
  import { sectionHeader } from "$lib/app/side-panel-host.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { boardActivity } from "./kanban-model";
  import { kanbanStore } from "./kanban-store.svelte";

  /** Every kanban board: whether it's on, what's running, what waits for the
   * user. A click opens the board in a tab. */

  const colors = $derived(themeStore.current.colors);
  /** The board in the tab in focus, highlighted in the list. */
  const current = $derived(workspaceStore.focusedSession?.kanbanView?.boardId ?? null);

  sectionHeader({ actions: headerActions });
</script>

<aside
  class="kanban-panel"
  style="--live:{colors.blue};--ask:{colors.yellow};--on:{colors.green}"
  aria-label={t("kanban.title")}
>
  <div class="kanban-inner">

    <div class="kanban-body">
      {#each kanbanStore.boards as board (board.id)}
        {@const activity = boardActivity(board)}
        <button class="board-row" class:current={current === board.id} onclick={() => kanbanStore.openBoard(board)}>
          <span class="board-icon" aria-hidden="true">{board.icon || "▦"}</span>
          <span class="board-text">
            <span class="board-name">{board.name || t("kanban.unnamed")}</span>
            <span class="board-meta">
              <span class="power" class:on={board.enabled}>{board.enabled ? t("kanban.row.on") : t("kanban.row.off")}</span>
              {#if activity.total > 0}
                · {t("kanban.row.progress", { done: String(activity.done), total: String(activity.total) })}
              {/if}
            </span>
          </span>
          {#if activity.needsYou > 0}
            <span class="badge asking" title={activity.needsYou === 1 ? t("kanban.row.needsYouOne") : t("kanban.row.needsYou", { n: String(activity.needsYou) })}>{activity.needsYou}</span>
          {:else if activity.running > 0}
            <span class="dot working" title={t("kanban.row.running", { n: String(activity.running) })}></span>
          {/if}
        </button>
      {:else}
        <div class="empty">
          <p>{t("kanban.empty")}</p>
          <p class="empty-hint">{t("kanban.emptyHint")}</p>
          <button class="new-button" onclick={() => kanbanStore.newBoard()}>{t("kanban.new")}</button>
        </div>
      {/each}
    </div>
  </div>
</aside>

<!-- Its header's buttons, shown in the side panel's header for it
     (see SidePanelColumn). -->

{#snippet headerActions()}
      <button class="icon-button" title={t("kanban.new")} aria-label={t("kanban.new")} onclick={() => kanbanStore.newBoard()}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 0v10M0 5h10" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
{/snippet}

<style>
  .kanban-panel {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .kanban-inner {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .icon-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    opacity: 0.7;
    cursor: pointer;
  }

  .icon-button svg {
    width: 12px;
    height: 12px;
  }

  .icon-button:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .kanban-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    padding: 4px 0;
  }

  .board-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: calc(100% - 12px);
    margin: 1px 6px;
    padding: 6px 8px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .board-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .board-row.current {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.18));
  }

  .board-icon {
    width: 18px;
    flex-shrink: 0;
    text-align: center;
    font-size: 14px;
  }

  .board-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .board-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .board-meta {
    font-size: 11px;
    opacity: 0.65;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .power::before {
    content: "";
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-right: 4px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.45;
    vertical-align: 1px;
  }

  .power.on::before {
    background: var(--on, #4ec27a);
    opacity: 1;
  }

  .dot {
    width: 8px;
    height: 8px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--live, #3a8ef0);
  }

  .working {
    animation: pulse 1.4s ease-in-out infinite;
  }

  .badge {
    min-width: 16px;
    padding: 0 5px;
    border-radius: 8px;
    font-size: 10.5px;
    line-height: 16px;
    text-align: center;
    font-weight: 600;
  }

  .badge.asking {
    background: var(--ask, #d7ba7d);
    color: #1e1e1e;
  }

  .empty {
    padding: 12px;
    line-height: 1.45;
  }

  .empty p {
    margin: 0 0 6px;
    opacity: 0.75;
  }

  .empty-hint {
    font-size: 11.5px;
    opacity: 0.6 !important;
  }

  .new-button {
    margin-top: 6px;
    padding: 5px 12px;
    border: 1px solid var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .new-button:hover {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.2));
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
