<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { favoriteLabel, favoritesStore } from "./favorites-store.svelte";
  import { canRestore, restoreSession, sessionLabel } from "./restore-session";
  import { TOOL_DISPLAY_NAMES } from "./tool-names";

  /** The favorites, to open, rename or remove — the layouts manager's shape. */
  let selected = $state<string | null>(null);
  /** Set to the id awaiting a second click: removing can't be undone. */
  let pendingRemove = $state<string | null>(null);
  let listEl: HTMLDivElement | undefined = $state();

  const favorites = $derived(favoritesStore.favorites);
  const current = $derived(favorites.find((favorite) => favorite.agentSessionId === selected) ?? null);

  // Opens on the first one so the buttons are usable straight away, and never
  // stays pointing at one that was just removed.
  $effect(() => {
    if (favorites.length === 0) selected = null;
    else if (!favorites.some((favorite) => favorite.agentSessionId === selected)) selected = favorites[0].agentSessionId;
  });

  $effect(() => {
    void selected;
    pendingRemove = null;
  });

  // On opening, and back from renaming in the dialog on top.
  $effect(() => {
    if (!uiState.favoriteDialog) listEl?.focus();
  });

  function close() {
    uiState.closeFavoritesManager();
    // The window took focus away from the terminal; hand it back.
    uiState.refocusTerminals();
  }

  function open() {
    if (!current || !canRestore(current)) return;
    const favorite = current;
    close();
    void restoreSession(favorite);
  }

  /** The same dialog that adds them, over this window. */
  function rename() {
    if (current) uiState.openFavoriteDialog(current);
  }

  async function remove() {
    if (!current) return;
    if (pendingRemove !== current.agentSessionId) {
      pendingRemove = current.agentSessionId;
      return;
    }
    await favoritesStore.remove(current.agentSessionId);
  }

  function move(delta: number) {
    if (favorites.length === 0) return;
    const index = favorites.findIndex((favorite) => favorite.agentSessionId === selected);
    const next = Math.min(favorites.length - 1, Math.max(0, (index === -1 ? 0 : index) + delta));
    selected = favorites[next].agentSessionId;
  }

  function onKeydown(event: KeyboardEvent) {
    // While renaming, the name dialog on top has the keyboard.
    if (uiState.favoriteDialog) return;
    // This window owns the keyboard while it's open (uiState.overlayOpen keeps
    // the window-level handlers out).
    event.stopPropagation();
    if (event.key === "Escape") {
      if (pendingRemove) pendingRemove = null;
      else close();
    } else if (event.key === "ArrowDown") {
      move(1);
    } else if (event.key === "ArrowUp") {
      move(-1);
    } else if (event.key === "Enter") {
      open();
    } else if (event.key === "F2") {
      rename();
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
    aria-label={t("favorites.manageTitle")}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title"><span class="star" aria-hidden="true">★</span> {t("favorites.manageTitle")}</h2>

    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="favorite-list" role="listbox" tabindex="0" bind:this={listEl}>
      {#each favorites as favorite (favorite.agentSessionId)}
        <button
          class="favorite-row"
          class:selected={favorite.agentSessionId === selected}
          role="option"
          aria-selected={favorite.agentSessionId === selected}
          title={favorite.cwd ?? favorite.projectDir ?? ""}
          onclick={() => (selected = favorite.agentSessionId)}
          ondblclick={open}
        >
          <ConsoleIcon kind={favorite.tool} size={14} />
          <span class="favorite-row-name">{favoriteLabel(favorite)}</span>
          <!-- Named ones also say which agent and folder they are. -->
          <span class="favorite-row-detail">
            {favorite.name ? sessionLabel(favorite) : TOOL_DISPLAY_NAMES[favorite.tool]}
          </span>
        </button>
      {:else}
        <p class="favorite-empty">{t("favorites.empty")}</p>
      {/each}
    </div>

    <p class="dialog-hint" class:warning={pendingRemove}>
      {pendingRemove && current
        ? t("favorites.removeWarning", { name: favoriteLabel(current) })
        : t("favorites.manageHint")}
    </p>

    <div class="dialog-actions">
      <button class="dialog-button" onclick={close}>{t("favorites.close")}</button>
      <span class="spacer"></span>
      <button class="dialog-button danger" disabled={!current} onclick={() => void remove()}>
        {pendingRemove && pendingRemove === current?.agentSessionId
          ? t("favorites.removeConfirm")
          : t("favorites.removeShort")}
      </button>
      <button class="dialog-button" disabled={!current} onclick={rename}>{t("favorites.rename")}</button>
      <button class="dialog-button primary" disabled={!current} onclick={open}>{t("favorites.open")}</button>
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
    width: min(520px, 92%);
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

  .star {
    color: #e5c07b;
  }

  .favorite-list {
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

  .favorite-list:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .favorite-row {
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

  .favorite-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .favorite-row.selected {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.35));
  }

  .favorite-row-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .favorite-row-detail {
    flex-shrink: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    opacity: 0.6;
  }

  .favorite-empty {
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
    align-items: center;
    gap: 8px;
  }

  .spacer {
    flex: 1;
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
