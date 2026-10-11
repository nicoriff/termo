<script lang="ts">
  import { openNewWindow, openPreferencesWindow } from "$lib/core/window-client";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { formatKeybindingLabel, type ShortcutAction } from "$lib/core/shortcuts";
  import {
    clearConsole,
    copySelection,
    focusedHasSelection,
    pasteClipboard,
    selectAll,
  } from "$lib/terminal/terminal-clipboard";
  import { requestQuit } from "$lib/app/quit";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { layoutsStore } from "$lib/layout/layouts-store.svelte";
  import { layoutHasSessions } from "$lib/layout/layout-snapshot";
  import type { SavedLayout } from "$lib/core/layouts-client";
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { favoriteLabel, favoritesStore } from "$lib/ai-mode/favorites-store.svelte";
  import { canRestore, restoreSession } from "$lib/ai-mode/restore-session";
  import type { AgentFavorite } from "$lib/core/agent-favorites-client";

  let { anchor, onclose }: { anchor?: HTMLElement; onclose: () => void } = $props();

  // Fixed widths so the position math can clamp against the window without
  // waiting on a post-render measurement — same approach as the shell picker.
  const MENU_WIDTH = 230;
  const SUBMENU_WIDTH = 240;
  const SUBMENU_MAX_HEIGHT = 260;
  /** Four rows and a divider — enough to keep the flyout on screen. */
  const EDIT_SUBMENU_HEIGHT = 140;
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

  let editOpen = $state(false);
  let editPos = $state({ top: 0, left: 0 });
  let editRowEl = $state<HTMLButtonElement | undefined>(undefined);

  let favoritesOpen = $state(false);
  let favoritesPos = $state({ top: 0, left: 0 });
  let favoritesRowEl = $state<HTMLButtonElement | undefined>(undefined);

  /** Only one flyout at a time, and every other row closes them. */
  function closeSubmenus() {
    restoreOpen = false;
    editOpen = false;
    favoritesOpen = false;
  }

  /** Where a flyout for `row` fits: beside it, flipped to its left when the
   * window edge is too close — up in the title bar's right corner, the usual
   * case. */
  function flyoutPos(row: HTMLButtonElement, height: number) {
    const box = row.getBoundingClientRect();
    const toTheRight = box.right + 4;
    const left =
      toTheRight + SUBMENU_WIDTH + MARGIN > window.innerWidth
        ? Math.max(MARGIN, box.left - 4 - SUBMENU_WIDTH)
        : toTheRight;
    return { top: Math.min(box.top, window.innerHeight - height - MARGIN), left };
  }

  function openRestore() {
    if (!restoreRowEl) return;
    editOpen = false;
    favoritesOpen = false;
    restorePos = flyoutPos(restoreRowEl, SUBMENU_MAX_HEIGHT);
    restoreOpen = true;
  }

  function openEdit() {
    if (!editRowEl) return;
    restoreOpen = false;
    favoritesOpen = false;
    editPos = flyoutPos(editRowEl, EDIT_SUBMENU_HEIGHT);
    editOpen = true;
  }

  function openFavorites() {
    if (!favoritesRowEl) return;
    restoreOpen = false;
    editOpen = false;
    favoritesPos = flyoutPos(favoritesRowEl, SUBMENU_MAX_HEIGHT);
    favoritesOpen = true;
  }

  /** Opens it in the focused tile, as reopening a recent session does. */
  function openFavorite(favorite: AgentFavorite) {
    onclose();
    void restoreSession(favorite);
  }

  function manageFavorites() {
    onclose();
    uiState.openFavoritesManager();
  }

  function preferences() {
    onclose();
    void openPreferencesWindow();
  }

  function newWindow() {
    onclose();
    void openNewWindow();
  }

  /** Read once: the menu is mounted fresh on every open, and clicking it
   * doesn't change the console's selection (the title bar never takes focus
   * from the terminal). */
  const canCopy = focusedHasSelection();

  function hint(action: ShortcutAction): string {
    return formatKeybindingLabel(appConfigStore.getEffectiveShortcut(action), isMac);
  }

  function runEdit(action: () => void | Promise<unknown>) {
    onclose();
    void action();
  }

  function saveLayout() {
    onclose();
    uiState.openSaveLayoutPrompt();
  }

  function manageLayouts() {
    onclose();
    uiState.openLayoutManager();
  }

  function browseExtensions() {
    onclose();
    uiState.openMarketplace();
  }

  function showDiagnostics() {
    onclose();
    uiState.toggleDiagnostics();
  }

  function showCompanion() {
    onclose();
    uiState.toggleCompanion();
  }

  function showAgents() {
    onclose();
    agentsStore.setOpen(true);
  }

  /** Asks first: restoring closes every console that's open. */
  function restore(layout: SavedLayout) {
    onclose();
    uiState.requestRestoreLayout(layout);
  }

  function about() {
    onclose();
    uiState.openAbout();
  }

  function checkUpdates() {
    onclose();
    uiState.openUpdates();
  }

  function quit() {
    onclose();
    // Asks first if anything is still running (see app/quit.ts).
    void requestQuit();
  }
</script>

<!-- The menu and its flyouts are placed once, from the window's size then: a
     resize (the backdrop never sees it — the window's edge or the OS's
     zoom button handles it) would leave them hanging where they were. -->
<svelte:window onresize={onclose} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="app-menu-backdrop" onpointerdown={onclose}></div>

<div class="app-menu" style="top:{pos.top}px; left:{pos.left}px" role="menu">
  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={newWindow}>
    <!-- Two overlapping windows: one more of them. -->
    <svg class="app-menu-icon" viewBox="0 0 14 14" aria-hidden="true">
      <rect x="1.5" y="3.5" width="8" height="8" rx="1" fill="none" stroke="currentColor" stroke-width="1" />
      <path d="M4.5 3.5V1.5h8v8h-3" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("shortcuts.labels.newWindow")}</span>
    <span class="app-menu-hint">{hint("newWindow")}</span>
  </button>

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={preferences}>
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

  <button
    class="app-menu-item"
    class:open={editOpen}
    role="menuitem"
    bind:this={editRowEl}
    aria-haspopup="menu"
    aria-expanded={editOpen}
    onmouseenter={openEdit}
    onclick={() => (editOpen ? (editOpen = false) : openEdit())}
  >
    <!-- Two sheets: the copy glyph everywhere, standing in for editing. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <rect x="1.5" y="1.5" width="6" height="7" fill="none" stroke="currentColor" stroke-width="1" />
      <path d="M4.5 10.5h6v-7" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("edit.title")}</span>
    <svg class="app-menu-chevron" viewBox="0 0 6 10" aria-hidden="true">
      <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
    </svg>
  </button>

  <div class="app-menu-divider"></div>

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={saveLayout}>
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

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={manageLayouts}>
    <!-- A stack: the saved ones, several of them. Not three lines — that's
         the menu button's own hamburger. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <rect x="3.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
      <path d="M8.5 10.5h-7v-7" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("layouts.manage")}</span>
  </button>

  <!-- Extensions, Agents and favorites are Modo IA's: only with it on. -->
  {#if appConfigStore.aiModeEnabled}
  <button
    class="app-menu-item"
    class:open={favoritesOpen}
    role="menuitem"
    bind:this={favoritesRowEl}
    aria-haspopup="menu"
    aria-expanded={favoritesOpen}
    onmouseenter={openFavorites}
    onclick={() => (favoritesOpen ? (favoritesOpen = false) : openFavorites())}
  >
    <!-- A star: the favorite conversations. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path
        d="M6 1.2l1.45 3 3.3.45-2.4 2.3.58 3.25L6 8.65 3.07 10.2l.58-3.25-2.4-2.3 3.3-.45z"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
        stroke-linejoin="round"
      />
    </svg>
    <span class="app-menu-text">{t("favorites.title")}</span>
    <svg class="app-menu-chevron" viewBox="0 0 6 10" aria-hidden="true">
      <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
    </svg>
  </button>

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={manageFavorites}>
    <!-- The star again, with the list it belongs to. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M1.5 3h4M1.5 6h3M1.5 9h4" stroke="currentColor" stroke-width="1" stroke-linecap="round" />
      <path
        d="M8.5 3.5l.9 1.8 2 .3-1.45 1.4.35 2-1.8-.95-1.8.95.35-2L5.6 5.6l2-.3z"
        fill="none"
        stroke="currentColor"
        stroke-width="0.9"
        stroke-linejoin="round"
      />
    </svg>
    <span class="app-menu-text">{t("favorites.manage")}</span>
  </button>

  <div class="app-menu-divider"></div>

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={browseExtensions}>
    <!-- A puzzle piece: the usual mark for an extension. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path
        d="M4.5 1.5h3v1.2a1 1 0 1 0 2 0V1.5h1v3h-1.2a1 1 0 1 0 0 2h1.2v3h-3v-1.2a1 1 0 1 0-2 0v1.2h-3v-3h1.2a1 1 0 1 0 0-2H1.5v-3h3z"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
        stroke-linejoin="round"
      />
    </svg>
    <span class="app-menu-text">{t("marketplace.open")}</span>
    <span class="app-menu-hint">{hint("marketplace")}</span>
  </button>

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={showAgents}>
    <!-- A robot's head: the agents the user built. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <rect x="1.5" y="3.5" width="9" height="7" rx="1.8" fill="none" stroke="currentColor" stroke-width="1" />
      <path d="M6 3.5V1.5M4.3 7h.01M7.7 7h.01" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
    </svg>
    <span class="app-menu-text">{t("agents.title")}</span>
    <span class="app-menu-hint">{hint("toggleAgents")}</span>
  </button>

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={showCompanion}>
    <!-- A coin: what the sessions cost. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <circle cx="6" cy="6" r="4.5" fill="none" stroke="currentColor" stroke-width="1" />
      <path d="M6 3.4v5.2M7.4 4.6c-.3-.5-.8-.7-1.4-.7-.8 0-1.4.4-1.4 1s.5.9 1.4 1.1 1.4.5 1.4 1.1-.6 1-1.4 1c-.6 0-1.1-.2-1.4-.7" fill="none" stroke="currentColor" stroke-width="0.9" stroke-linecap="round" />
    </svg>
    <span class="app-menu-text">{t("companion.open")}</span>
    <span class="app-menu-hint">{hint("companion")}</span>
  </button>
  {/if}

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={showDiagnostics}>
    <!-- A pulse line: the usual mark for a live reading. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path
        d="M1 6.5h2.2l1.5-3.5 2.6 6 1.4-2.5H11"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
    <span class="app-menu-text">{t("diagnostics.open")}</span>
    <span class="app-menu-hint">{hint("diagnostics")}</span>
  </button>

  <div class="app-menu-divider"></div>

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={about}>
    <!-- An "i" in a circle: information about the app. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <circle cx="6" cy="6" r="5" fill="none" stroke="currentColor" stroke-width="1" />
      <line x1="6" y1="5" x2="6" y2="9" stroke="currentColor" stroke-width="1" />
      <circle cx="6" cy="3.3" r="0.7" fill="currentColor" />
    </svg>
    <span class="app-menu-text">{t("about.open")}</span>
  </button>

  <button class="app-menu-item" role="menuitem" onmouseenter={closeSubmenus} onclick={checkUpdates}>
    <!-- An arrow down into a tray: a new version coming in. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M6 1.5v6M3.5 5l2.5 2.5L8.5 5" fill="none" stroke="currentColor" stroke-width="1" />
      <path d="M1.5 8v2.5h9V8" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("updates.check")}</span>
  </button>

  <button class="app-menu-item danger" role="menuitem" onmouseenter={closeSubmenus} onclick={quit}>
    <!-- Power. -->
    <svg class="app-menu-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M3.2 3.4a4 4 0 1 0 5.6 0" fill="none" stroke="currentColor" stroke-width="1" />
      <line x1="6" y1="1" x2="6" y2="5.5" stroke="currentColor" stroke-width="1" />
    </svg>
    <span class="app-menu-text">{t("titlebar.quit")}</span>
  </button>
</div>

{#if editOpen}
  <div class="app-submenu" style="top:{editPos.top}px; left:{editPos.left}px" role="menu">
    <button class="app-menu-item" role="menuitem" disabled={!canCopy} onclick={() => runEdit(copySelection)}>
      <span class="app-menu-text">{t("edit.copy")}</span>
      <span class="app-menu-hint">{hint("copy")}</span>
    </button>
    <button class="app-menu-item" role="menuitem" onclick={() => runEdit(pasteClipboard)}>
      <span class="app-menu-text">{t("edit.paste")}</span>
      <span class="app-menu-hint">{hint("paste")}</span>
    </button>
    <button class="app-menu-item" role="menuitem" onclick={() => runEdit(selectAll)}>
      <span class="app-menu-text">{t("edit.selectAll")}</span>
      <span class="app-menu-hint">{hint("selectAll")}</span>
    </button>
    <div class="app-menu-divider"></div>
    <button class="app-menu-item" role="menuitem" onclick={() => runEdit(clearConsole)}>
      <span class="app-menu-text">{t("edit.clear")}</span>
    </button>
  </div>
{/if}

{#if favoritesOpen}
  <div class="app-submenu" style="top:{favoritesPos.top}px; left:{favoritesPos.left}px" role="menu">
    {#each favoritesStore.favorites.filter(canRestore) as favorite (favorite.agentSessionId)}
      <button
        class="app-menu-item"
        role="menuitem"
        title={favorite.cwd ?? favorite.projectDir ?? ""}
        onclick={() => openFavorite(favorite)}
      >
        <span class="app-menu-agent"><ConsoleIcon kind={favorite.tool} size={12} /></span>
        <span class="app-menu-text">{favoriteLabel(favorite)}</span>
      </button>
    {:else}
      <p class="app-menu-empty">{t("favorites.empty")}</p>
    {/each}
  </div>
{/if}

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
     z-index 8, the palette at 400, the marketplace panel at 599/600). */
  .app-menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 601;
  }

  /* Copy with nothing selected. */
  .app-menu-item:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .app-menu-hint {
    margin-left: auto;
    padding-left: 12px;
    font-size: 10px;
    opacity: 0.5;
    white-space: nowrap;
  }

  .app-menu {
    position: fixed;
    z-index: 602;
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
    z-index: 603;
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

  /* The agent's own icon in place of a menu icon. */
  .app-menu-agent {
    display: flex;
    flex-shrink: 0;
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
