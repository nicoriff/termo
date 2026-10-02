<script lang="ts">
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { formatKeybindingLabel, type ShortcutAction } from "$lib/core/shortcuts";
  import { t } from "$lib/i18n/i18n.svelte";
  import { clearConsole, copySelection, focusedHasSelection, pasteClipboard, selectAll } from "./terminal-clipboard";
  import { favoriteDraftForFocusedTab, favoritesStore } from "$lib/ai-mode/favorites-store.svelte";

  // Fixed size so the position math can clamp against the window without
  // waiting on a post-render measurement — same approach as the app menu.
  const WIDTH = 210;
  const BASE_HEIGHT = 140;
  const FAVORITE_ROW_HEIGHT = 38;
  const MARGIN = 8;

  const at = $derived(uiState.terminalContextMenu);

  /** The conversation in the console that was right-clicked (the focused
   * tab: the click focused it), as a favorite to add or edit. Modo IA only. */
  const favorite = $derived(appConfigStore.aiModeEnabled ? favoriteDraftForFocusedTab() : null);
  const isFavorite = $derived(favorite !== null && favoritesStore.byId.has(favorite.agentSessionId));
  const HEIGHT = $derived(BASE_HEIGHT + (favorite ? FAVORITE_ROW_HEIGHT : 0));

  const pos = $derived.by(() => {
    if (!at) return { top: 0, left: 0 };
    return {
      top: Math.min(at.y, window.innerHeight - HEIGHT - MARGIN),
      left: Math.min(at.x, window.innerWidth - WIDTH - MARGIN),
    };
  });

  /** Read when the menu opens: right-clicking doesn't change the selection. */
  const canCopy = focusedHasSelection();

  function hint(action: ShortcutAction): string {
    return formatKeybindingLabel(appConfigStore.getEffectiveShortcut(action), isMac);
  }

  function run(action: () => void | Promise<unknown>) {
    uiState.closeTerminalContextMenu();
    void action();
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === "Escape") uiState.closeTerminalContextMenu();
  }}
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="context-backdrop" onpointerdown={() => uiState.closeTerminalContextMenu()}></div>

<div class="context-menu" style="top:{pos.top}px; left:{pos.left}px" role="menu">
  <button class="context-item" role="menuitem" disabled={!canCopy} onclick={() => run(copySelection)}>
    <span class="context-text">{t("edit.copy")}</span>
    <span class="context-hint">{hint("copy")}</span>
  </button>
  <button class="context-item" role="menuitem" onclick={() => run(pasteClipboard)}>
    <span class="context-text">{t("edit.paste")}</span>
    <span class="context-hint">{hint("paste")}</span>
  </button>
  <button class="context-item" role="menuitem" onclick={() => run(selectAll)}>
    <span class="context-text">{t("edit.selectAll")}</span>
    <span class="context-hint">{hint("selectAll")}</span>
  </button>
  <div class="context-divider"></div>
  <button class="context-item" role="menuitem" onclick={() => run(clearConsole)}>
    <span class="context-text">{t("edit.clear")}</span>
  </button>
  {#if favorite}
    <div class="context-divider"></div>
    <button class="context-item" role="menuitem" onclick={() => run(() => uiState.openFavoriteDialog(favorite))}>
      <span class="context-text">{t(isFavorite ? "favorites.edit" : "favorites.add")}</span>
      <span class="context-hint">{hint("favoriteTab")}</span>
      <span class="context-star" class:on={isFavorite} aria-hidden="true">{isFavorite ? "★" : "☆"}</span>
    </button>
  {/if}
</div>

<style>
  .context-backdrop {
    position: fixed;
    inset: 0;
    z-index: 499;
  }

  .context-menu {
    position: fixed;
    z-index: 500;
    width: 210px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    color: var(--titlebar-fg, #d4d4d4);
  }

  .context-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .context-item:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  /* Copy with nothing selected. */
  .context-item:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .context-text {
    flex: 1;
  }

  .context-star {
    font-size: 12px;
    opacity: 0.6;
  }

  .context-star.on {
    color: #e5c07b;
    opacity: 1;
  }

  .context-hint {
    font-size: 10px;
    opacity: 0.5;
    white-space: nowrap;
  }

  .context-divider {
    height: 1px;
    margin: 4px 2px;
    background: var(--tabbar-border, #2a2a2a);
  }
</style>
