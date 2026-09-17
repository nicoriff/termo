<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { formatElapsed } from "$lib/ai-mode/elapsed";
  import { layoutHasSessions } from "./layout-snapshot";
  import { layoutsStore } from "./layouts-store.svelte";

  let selected = $state<string | null>(null);
  /** Set to the name awaiting a second click: deleting a layout can't be undone. */
  let pendingDelete = $state<string | null>(null);
  let now = $state(Date.now());
  let listEl: HTMLDivElement | undefined = $state();

  const layouts = $derived(layoutsStore.layouts);
  const current = $derived(layouts.find((layout) => layout.name === selected) ?? null);

  // Opens on the first layout so the buttons are usable straight away, and
  // never stays pointing at one that was just deleted.
  $effect(() => {
    if (layouts.length === 0) {
      selected = null;
    } else if (!layouts.some((layout) => layout.name === selected)) {
      selected = layouts[0].name;
    }
  });

  $effect(() => {
    void selected;
    pendingDelete = null;
  });

  // Keeps "saved 3 minutes ago" honest while the window sits open.
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 30000);
    return () => clearInterval(timer);
  });

  $effect(() => {
    listEl?.focus();
  });

  function close() {
    uiState.closeLayoutManager();
    // The window took focus away from the terminal; hand it back.
    uiState.refocusTerminals();
  }

  function restore() {
    if (!current) return;
    close();
    // Goes through the same confirmation as every other way of restoring.
    uiState.requestRestoreLayout(current);
  }

  async function remove() {
    if (!current) return;
    if (pendingDelete !== current.name) {
      pendingDelete = current.name;
      return;
    }
    await layoutsStore.remove(current.name);
  }

  function move(delta: number) {
    if (layouts.length === 0) return;
    const index = layouts.findIndex((layout) => layout.name === selected);
    const next = Math.min(layouts.length - 1, Math.max(0, (index === -1 ? 0 : index) + delta));
    selected = layouts[next].name;
  }

  function onKeydown(event: KeyboardEvent) {
    // This window owns the keyboard while it's open (uiState.overlayOpen keeps
    // the window-level handlers out).
    event.stopPropagation();
    if (event.key === "Escape") {
      if (pendingDelete) pendingDelete = null;
      else close();
    } else if (event.key === "ArrowDown") {
      move(1);
    } else if (event.key === "ArrowUp") {
      move(-1);
    } else if (event.key === "Enter") {
      restore();
    } else if (event.key === "Delete") {
      void remove();
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="dialog-anchor">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-label={t("layouts.manageTitle")}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">{t("layouts.manageTitle")}</h2>

    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="layout-list" role="listbox" tabindex="0" bind:this={listEl}>
      {#each layouts as layout (layout.name)}
        <button
          class="layout-row"
          class:selected={layout.name === selected}
          role="option"
          aria-selected={layout.name === selected}
          onclick={() => (selected = layout.name)}
          ondblclick={restore}
        >
          <span class="layout-row-name">{layout.name}</span>
          {#if layoutHasSessions(layout.root)}
            <span class="layout-pill">{t("layouts.aiPill")}</span>
          {/if}
          <span class="layout-row-time">
            {t("layouts.savedAgo", { time: formatElapsed(now - layout.savedAt * 1000) })}
          </span>
        </button>
      {:else}
        <p class="layout-empty">{t("layouts.empty")}</p>
      {/each}
    </div>

    <p class="dialog-hint" class:warning={pendingDelete}>
      {pendingDelete ? t("layouts.deleteWarning", { name: pendingDelete }) : t("layouts.manageHint")}
    </p>

    <div class="dialog-actions">
      <button class="dialog-button" onclick={close}>{t("layouts.close")}</button>
      <button class="dialog-button danger" disabled={!current} onclick={() => void remove()}>
        {pendingDelete === current?.name ? t("layouts.deleteConfirm") : t("layouts.delete")}
      </button>
      <button class="dialog-button primary" disabled={!current} onclick={restore}>
        {t("layouts.restoreConfirm")}
      </button>
    </div>
  </div>
</div>

<style>
  .dialog-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.35);
  }

  /* The panel carries the fly transition, which animates transform — so the
     centering lives on this wrapper instead, same as the command palette. */
  .dialog-anchor {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }

  .dialog {
    pointer-events: auto;
    width: min(480px, 92%);
    margin-top: 10vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
    color: var(--titlebar-fg, #d4d4d4);
  }

  .dialog-title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .layout-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 320px;
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: var(--tabbar-bg, #181818);
  }

  .layout-list:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .layout-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    padding: 7px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .layout-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .layout-row.selected {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.35));
  }

  .layout-row-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .layout-row-time {
    flex-shrink: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  /* Same marker the title bar menu uses for a layout that carries agent
     conversations. */
  .layout-pill {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.03em;
  }

  .layout-empty {
    margin: 0;
    padding: 10px 8px;
    font-size: 12px;
    opacity: 0.6;
  }

  .dialog-hint {
    margin: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .dialog-hint.warning {
    color: #ff6b6b;
    opacity: 0.9;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .dialog-button {
    padding: 6px 14px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .dialog-button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .dialog-button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .dialog-button.danger:not(:disabled) {
    color: #ff6b6b;
    border-color: rgba(255, 107, 107, 0.4);
  }

  .dialog-button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }
</style>
