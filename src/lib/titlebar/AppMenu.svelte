<script lang="ts">
  import { openPreferencesWindow } from "$lib/core/window-client";
  import { requestQuit } from "$lib/app/quit";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { layoutsStore } from "$lib/layout/layouts-store.svelte";
  import { layoutHasSessions } from "$lib/layout/layout-snapshot";
  import type { SavedLayout } from "$lib/core/layouts-client";

  let { anchor, onclose }: { anchor?: HTMLElement; onclose: () => void } = $props();

  // Fixed widths so the position math can clamp against the window without
  // waiting on a post-render measurement — same approach as the shell picker.
  const MENU_WIDTH = 230;
  const SUBMENU_WIDTH = 240;
  const SUBMENU_MAX_HEIGHT = 260;
  const MARGIN = 8;

  // Right-aligned under the button (it sits at the window's right edge), then
  // clamped so a narrow window can't push it off screen.
  const pos = $derived.by(() => {
    if (!anchor) return { top: 0, left: 0 };
    const box = anchor.getBoundingClientRect();
    return {
      top: box.bottom + 2,
      left: Math.max(MARGIN, Math.min(box.right - MENU_WIDTH, window.innerWidth - MENU_WIDTH - MARGIN)),
    };
  });

  let restoreOpen = $state(false);
  let restorePos = $state({ top: 0, left: 0 });
  let restoreRowEl = $state<HTMLButtonElement | undefined>(undefined);

  /** Flies out beside the menu, flipping to its left when the window edge is
   * too close — which, up in the title bar's right corner, is the usual case. */
  function openRestore() {
    if (!restoreRowEl) return;
    const box = restoreRowEl.getBoundingClientRect();
    const toTheRight = box.right + 4;
    const left =
      toTheRight + SUBMENU_WIDTH + MARGIN > window.innerWidth
        ? Math.max(MARGIN, box.left - 4 - SUBMENU_WIDTH)
        : toTheRight;
    const top = Math.min(box.top, window.innerHeight - SUBMENU_MAX_HEIGHT - MARGIN);
    restorePos = { top, left };
    restoreOpen = true;
  }

  function preferences() {
    onclose();
    void openPreferencesWindow();
  }

  function saveLayout() {
    onclose();
    uiState.openSaveLayoutPrompt();
  }

  function manageLayouts() {
    onclose();
    uiState.openLayoutManager();
  }

  /** Asks first: restoring closes every console that's open. */
  function restore(layout: SavedLayout) {
    onclose();
    uiState.requestRestoreLayout(layout);
  }

  function quit() {
    onclose();
    // Asks first if anything is still running (see app/quit.ts).
    void requestQuit();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="app-menu-backdrop" onpointerdown={onclose}></div>

<div class="app-menu" style="top:{pos.top}px; left:{pos.left}px" role="menu">
  <button class="app-menu-item" role="menuitem" onmouseenter={() => (restoreOpen = false)} onclick={preferences}>
    <!-- Sliders: the app's established settings glyph (same as the title bar button). -->
    <svg class="app-menu-icon" viewBox="0 0 14 14" aria-hidden="true">
      <line x1="1" y1="3" x2="13" y2="3" stroke="currentColor" stroke-width="1" />
      <circle cx="9" cy="3" r="1.5" fill="currentColor" />
      <line x1="1" y1="7" x2="13" y2="7" stroke="currentColor" stroke-width="1" />
      <circle cx="5" cy="7" r="1.5" fill="currentColor" />
      <line x1="1" y1="11" x2="13" y2="11" stroke="currentColor" stroke-width="1" />
      <circle cx="10" cy="11" r="1.5" fill="currentColor" />
    </svg>
    <span class="app-menu-text">{t("titlebar.systemPreferences")}</span>
  </button>

  <div class="app-menu-divider"></div>

  <button class="app-menu-item" role="menuitem" onmouseenter={() => (restoreOpen = false)} onclick={saveLayout}>
    <!-- Floppy: save. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M1.5 1.5h6.5l2.5 2.5v6.5h-9z" fill="none" stroke="currentColor" stroke-width="1" />
      <path d="M3.5 1.5v3h4v-3" fill="none" stroke="currentColor" stroke-width="1" />
      <rect x="3.5" y="7" width="5" height="3.5" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("layouts.save")}</span>
  </button>

  <button
    class="app-menu-item"
    class:open={restoreOpen}
    role="menuitem"
    bind:this={restoreRowEl}
    aria-haspopup="menu"
    aria-expanded={restoreOpen}
    onmouseenter={openRestore}
    onclick={() => (restoreOpen ? (restoreOpen = false) : openRestore())}
  >
    <!-- A split pane: what a layout is. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <rect x="1" y="1" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1" />
      <line x1="6" y1="1" x2="6" y2="11" stroke="currentColor" stroke-width="1" />
      <line x1="6" y1="6" x2="11" y2="6" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("layouts.restore")}</span>
    <svg class="app-menu-chevron" viewBox="0 0 6 10" aria-hidden="true">
      <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
    </svg>
  </button>

  <button class="app-menu-item" role="menuitem" onmouseenter={() => (restoreOpen = false)} onclick={manageLayouts}>
    <!-- A stack: the saved ones, several of them. Not three lines — that's
         the menu button's own hamburger. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <rect x="3.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
      <path d="M8.5 10.5h-7v-7" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("layouts.manage")}</span>
  </button>

  <div class="app-menu-divider"></div>

  <button class="app-menu-item danger" role="menuitem" onmouseenter={() => (restoreOpen = false)} onclick={quit}>
    <!-- Power. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M3.2 3.4a4 4 0 1 0 5.6 0" fill="none" stroke="currentColor" stroke-width="1" />
      <line x1="6" y1="1" x2="6" y2="5.5" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("titlebar.quit")}</span>
  </button>
</div>

{#if restoreOpen}
  <div class="app-submenu" style="top:{restorePos.top}px; left:{restorePos.left}px" role="menu">
    {#each layoutsStore.layouts as layout (layout.name)}
      <button class="app-menu-item" role="menuitem" onclick={() => restore(layout)}>
        <span class="app-menu-text">{layout.name}</span>
        {#if layoutHasSessions(layout.root)}
          <span class="app-menu-pill">{t("layouts.aiPill")}</span>
        {/if}
      </button>
    {:else}
      <p class="app-menu-empty">{t("layouts.empty")}</p>
    {/each}
  </div>
{/if}

<style>
  /* position:fixed, above everything the content area draws (tiles sit at
     z-index 8, the palette at 400). */
  .app-menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 499;
  }

  .app-menu {
    position: fixed;
    z-index: 500;
    width: 230px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    color: var(--titlebar-fg, #d4d4d4);
  }

  .app-submenu {
    position: fixed;
    z-index: 501;
    width: 240px;
    max-height: 260px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: 4px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    color: var(--titlebar-fg, #d4d4d4);
  }

  .app-menu-item {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .app-menu-item:hover,
  .app-menu-item.open {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .app-menu-item.danger:hover {
    background: var(--titlebar-close-hover-bg, #e81123);
    color: #fff;
  }

  /* Fixed slot so every label starts on the same vertical line. */
  .app-menu-icon {
    width: 13px;
    height: 13px;
    flex-shrink: 0;
    opacity: 0.85;
  }

  .app-menu-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .app-menu-chevron {
    width: 6px;
    height: 10px;
    flex-shrink: 0;
    opacity: 0.7;
  }

  /* Marks a layout that also carries agent conversations. */
  .app-menu-pill {
    flex-shrink: 0;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.03em;
  }

  .app-menu-divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--tabbar-border, #2a2a2a);
    flex-shrink: 0;
  }

  .app-menu-empty {
    margin: 0;
    padding: 8px 10px;
    font-size: 11px;
    opacity: 0.6;
  }
</style>
