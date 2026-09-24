<script lang="ts">
  import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
  import type { AiSessionStatus } from "$lib/ai-mode/classifier";
  import { workspaceStore } from "./workspace-store.svelte";
  import { listShells } from "$lib/core/pty-client";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { resolveShellArgs } from "$lib/core/shell-args";
  import { resolveDefaultShell } from "$lib/core/default-shell";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import { tabCwd } from "$lib/terminal/tab-cwd.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { ShellInfo } from "$lib/core/types";
  import type { PaneInsets, PaneRect, TileNode } from "./split-tree";
  import { t } from "$lib/i18n/i18n.svelte";
  import { agentSessionsStore } from "$lib/ai-mode/agent-sessions-store.svelte";
  import { canRestore, restoreSession, sessionLabel } from "$lib/ai-mode/restore-session";
  import type { AgentSession } from "$lib/core/agent-sessions-client";
  import { marketplaceStatus } from "$lib/core/marketplace-client";
  import { uiState } from "$lib/core/ui-state.svelte";
  import type { ToolId } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";

  // No CSS custom properties exist for the ANSI palette (theme-loader.ts
  // only exposes chrome colors like --titlebar-bg) — read straight from the
  // active theme instead, same convention AppearanceSection's swatch dots use.
  function aiDotColor(status: AiSessionStatus): string {
    const colors = themeStore.current.colors;
    switch (status) {
      case "running":
        return colors.blue;
      case "awaiting":
        return colors.yellow;
      case "error":
        return colors.red;
      case "finished":
        return colors.green;
    }
  }

  let { tile, rect, insets }: { tile: TileNode; rect: PaneRect; insets: PaneInsets } = $props();

  // Fixed (not just min-) width so the click-time position math below can
  // clamp against the window bounds deterministically, without waiting on a
  // post-render measurement of the panel's actual size.
  const SHELL_PICKER_WIDTH = 200;
  const SHELL_PICKER_MAX_HEIGHT = 300;
  const SHELL_PICKER_MARGIN = 8;
  // Wider than the picker: these rows carry an agent name plus a folder.
  const RECENT_SUBMENU_WIDTH = 240;

  let shellPickerOpen = $state(false);
  let shellPickerPos = $state({ top: 0, left: 0 });
  let shells = $state<ShellInfo[]>([]);
  let shellsRequested = false;
  let addButtonEl: HTMLButtonElement | undefined;

  function toggleShellPicker() {
    shellPickerOpen = !shellPickerOpen;
    recentOpen = false;
    if (shellPickerOpen && addButtonEl) {
      // Rendered via position:fixed (not nested/absolute) because
      // .tile-tabstrip sets overflow-x:auto, which per the CSS spec forces
      // overflow-y to auto too — a nested absolutely-positioned panel got
      // silently clipped by that, even though it opened correctly.
      //
      // Anchored right-aligned to the button by default, then clamped into
      // the window on both axes — in a narrow split tile (e.g. the left
      // pane of a side-by-side split), right-aligning unclamped could push
      // the panel's left edge past the window's left edge entirely.
      const box = addButtonEl.getBoundingClientRect();
      const left = Math.max(
        SHELL_PICKER_MARGIN,
        Math.min(box.right - SHELL_PICKER_WIDTH, window.innerWidth - SHELL_PICKER_WIDTH - SHELL_PICKER_MARGIN),
      );
      const top = Math.min(box.bottom + 4, window.innerHeight - SHELL_PICKER_MAX_HEIGHT - SHELL_PICKER_MARGIN);
      shellPickerPos = { top, left };
    }
    if (shellPickerOpen && !shellsRequested) {
      shellsRequested = true;
      listShells()
        .then((list) => (shells = list))
        .catch(() => (shellsRequested = false));
    }
    // Checked on every open, not once: installing Claude or Codex while
    // Termini runs should light its entry up without a restart.
    if (shellPickerOpen) {
      marketplaceStatus()
        .then((status) => (agentClis = { claude: !!status.claudeCli, codex: !!status.codexCli }))
        .catch(() => {});
    }
  }

  /** Which agent CLIs are installed; null until the first check answers,
   * during which both entries stay usable rather than flicker off. */
  let agentClis = $state<Record<ToolId, boolean> | null>(null);
  const AGENTS: ToolId[] = ["claude", "codex"];

  /** Asks where to run it first (see AgentLaunchDialog), offering the folder
   * this tile's console is in right now. */
  function pickAgent(tool: ToolId) {
    const active = tile.sessions.find((session) => session.id === tile.activeSessionId);
    const here = active ? (tabCwd.byTab[active.id] ?? active.cwd ?? null) : null;
    closeShellPicker();
    uiState.openAgentLaunch(tool, tile.id, here);
  }

  function closeShellPicker() {
    shellPickerOpen = false;
    recentOpen = false;
  }

  function pickShell(shell: ShellInfo) {
    const args = resolveShellArgs(shell.id, shell.args, appConfigStore.psreadlineMenuComplete);
    // Opens where this tile's active console is now, like the new-tab
    // shortcut does (see workspaceStore.newTabLikeFocused).
    const active = tile.sessions.find((session) => session.id === tile.activeSessionId);
    const cwd = active ? (tabCwd.byTab[active.id] ?? active.cwd) : undefined;
    workspaceStore.addTab(tile.id, shell.path, args, shell.display_name, cwd);
    closeShellPicker();
  }

  /** Past agent conversations, reopened in THIS tile rather than the focused
   * one — the picker belongs to this tile's "+" button. Only offered while AI
   * Mode is on (it's what records them), and only those whose id is safe to
   * type into a shell. */
  const recentSessions = $derived(
    appConfigStore.aiModeEnabled ? agentSessionsStore.sessions.filter(canRestore) : [],
  );

  let recentOpen = $state(false);
  let recentPos = $state({ top: 0, left: 0 });
  // $state (unlike addButtonEl): this row lives inside an {#if}, so bind:this
  // reassigns it as the row appears and disappears.
  let recentRowEl = $state<HTMLButtonElement | undefined>(undefined);

  /** Flies out beside the picker, flipping to its left when the window edge is
   * too close — same clamping idea as the picker itself. */
  function openRecent() {
    if (!recentRowEl) return;
    const box = recentRowEl.getBoundingClientRect();
    const toTheRight = box.right + 4;
    const left =
      toTheRight + RECENT_SUBMENU_WIDTH + SHELL_PICKER_MARGIN > window.innerWidth
        ? Math.max(SHELL_PICKER_MARGIN, box.left - 4 - RECENT_SUBMENU_WIDTH)
        : toTheRight;
    const top = Math.min(box.top, window.innerHeight - SHELL_PICKER_MAX_HEIGHT - SHELL_PICKER_MARGIN);
    recentPos = { top, left };
    recentOpen = true;
  }

  function pickRecent(session: AgentSession) {
    closeShellPicker();
    void restoreSession(session, tile.id);
  }

  async function split(direction: "row" | "col") {
    workspaceStore.splitTile(tile.id, direction, await resolveDefaultShell());
  }

  // Manual pointer-based drag instead of native HTML5 DnD: on Windows, native
  // drag-and-drop goes through the OS's OLE drag session, which doesn't behave
  // consistently (or automate the same way) across platforms/webviews. Reusing
  // the same pointer-capture technique as the split-divider resize keeps tab
  // dragging fully in our control and consistent everywhere.
  function onTabPointerDown(event: PointerEvent, sessionId: string) {
    if (event.button !== 0) return;
    const startX = event.clientX;
    const startY = event.clientY;
    const sourceTileId = tile.id;
    const sourceTab = event.currentTarget as HTMLElement;
    let dragging = false;
    let ghost: HTMLElement | null = null;
    // Where inside the tab it was grabbed, so the ghost stays under the
    // pointer at that same spot instead of jumping to its corner.
    let grabX = 0;
    let grabY = 0;

    /** A copy of the tab that follows the pointer. It keeps the tab's own
     * classes (Svelte's scoping included) so it looks like the tab itself,
     * and ignores the pointer so hit-testing sees what's underneath it. */
    function createGhost() {
      const box = sourceTab.getBoundingClientRect();
      grabX = startX - box.left;
      grabY = startY - box.top;
      ghost = sourceTab.cloneNode(true) as HTMLElement;
      ghost.classList.add("tab-drag-ghost");
      ghost.removeAttribute("data-session-id");
      ghost.style.width = `${box.width}px`;
      ghost.style.height = `${box.height}px`;
      document.body.appendChild(ghost);
      sourceTab.classList.add("drag-source");
    }

    function moveGhost(e: PointerEvent) {
      if (ghost) ghost.style.transform = `translate(${e.clientX - grabX}px, ${e.clientY - grabY}px)`;
    }

    /** The tile under the pointer, by geometry rather than hit-testing: most of
     * a tile is its console, which belongs to SplitView rather than to this
     * chrome, so the whole tile counts as a drop target this way. */
    function currentDropTile(e: PointerEvent): HTMLElement | null {
      for (const chrome of document.querySelectorAll<HTMLElement>(".tile-chrome")) {
        const box = chrome.getBoundingClientRect();
        if (
          box.width > 0 &&
          e.clientX >= box.left &&
          e.clientX < box.right &&
          e.clientY >= box.top &&
          e.clientY < box.bottom
        ) {
          return chrome;
        }
      }
      return null;
    }

    function currentDropTab(e: PointerEvent): HTMLElement | null {
      const el = document.elementFromPoint(e.clientX, e.clientY);
      return el instanceof Element ? el.closest<HTMLElement>(".tile-tab") : null;
    }

    function clearHighlight() {
      document.querySelectorAll(".tile-chrome.drag-over").forEach((el) => el.classList.remove("drag-over"));
      document
        .querySelectorAll(".tile-tab.drag-insert-before, .tile-tab.drag-insert-after")
        .forEach((el) => el.classList.remove("drag-insert-before", "drag-insert-after"));
    }

    // Which existing tab (if any) the pointer is currently over, and whether
    // it should land before or after that tab — used both to preview the
    // drop position and, on release, to compute moveSession's beforeSessionId.
    function hoveredInsertion(e: PointerEvent): { tab: HTMLElement; before: boolean } | null {
      const overTab = currentDropTab(e);
      if (!overTab || overTab.dataset.sessionId === sessionId) return null;
      const box = overTab.getBoundingClientRect();
      return { tab: overTab, before: e.clientX < box.left + box.width / 2 };
    }

    function onMove(e: PointerEvent) {
      if (!dragging && Math.hypot(e.clientX - startX, e.clientY - startY) > 4) {
        dragging = true;
        document.body.classList.add("termini-dragging-tab");
        createGhost();
      }
      if (!dragging) return;
      moveGhost(e);
      clearHighlight();
      currentDropTile(e)?.classList.add("drag-over");
      const insertion = hoveredInsertion(e);
      insertion?.tab.classList.add(insertion.before ? "drag-insert-before" : "drag-insert-after");
    }

    function onUp(e: PointerEvent) {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      document.body.classList.remove("termini-dragging-tab");
      clearHighlight();
      ghost?.remove();
      ghost = null;
      sourceTab.classList.remove("drag-source");

      if (dragging) {
        const targetTileId = currentDropTile(e)?.dataset.tileId;
        const insertion = hoveredInsertion(e);
        const overStrip = !!document.elementFromPoint(e.clientX, e.clientY)?.closest(".tile-tabstrip");
        // Let go over itself, or over its own tile's console, the tab stays
        // where it was: only the tab strip reorders a tile's own tabs.
        const staysPut = currentDropTab(e) === sourceTab || (targetTileId === sourceTileId && !overStrip);
        if (targetTileId && !staysPut) {
          let beforeSessionId: string | undefined;
          if (insertion) {
            beforeSessionId = insertion.before
              ? insertion.tab.dataset.sessionId
              : insertion.tab.nextElementSibling instanceof HTMLElement
                ? insertion.tab.nextElementSibling.dataset.sessionId
                : undefined;
          }
          workspaceStore.moveSession(sourceTileId, targetTileId, sessionId, beforeSessionId);
        }
      } else {
        workspaceStore.setActiveSession(sourceTileId, sessionId);
      }
    }

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  function closeTab(event: Event, sessionId: string) {
    event.stopPropagation();
    workspaceStore.closeTab(tile.id, sessionId);
  }

  // Moves the shell picker to <body> so it escapes .tile-chrome's stacking
  // context — position:fixed alone doesn't do this: an ancestor with
  // position + z-index (needed so tiles/dividers layer correctly among
  // themselves) still confines fixed-positioned descendants to its own
  // stacking context, so the picker was rendering under sibling dividers.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="tile-chrome"
  data-tile-id={tile.id}
  class:focused={workspaceStore.focusedTileId === tile.id}
  style="left:calc({rect.left * 100}% + {insets.left}px); top:calc({rect.top * 100}% + {insets.top}px); width:calc({rect.width *
    100}% - {insets.left + insets.right}px); height:calc({rect.height * 100}% - {insets.top + insets.bottom}px)"
>
  <div class="tile-tabstrip" data-tile-id={tile.id}>
    {#each tile.sessions as session (session.id)}
      {@const aiStatus = aiStatusStore.byId[session.id]}
      <div
        class="tile-tab"
        class:active={tile.activeSessionId === session.id}
        data-session-id={session.id}
        role="tab"
        tabindex="0"
        aria-selected={tile.activeSessionId === session.id}
        onpointerdown={(e) => onTabPointerDown(e, session.id)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") workspaceStore.setActiveSession(tile.id, session.id);
        }}
      >
        {#if aiStatus?.tool && aiStatus.status}
          <span class="tile-tab-ai-dot" style="background:{aiDotColor(aiStatus.status)}"></span>
        {/if}
        <span class="tile-tab-title" title={tabTitleStore.titleFor(session.id, session.title)}>
          {tabTitleStore.titleFor(session.id, session.title)}
        </span>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <span
          class="tile-tab-close"
          role="button"
          tabindex="-1"
          aria-label={t("tabs.close")}
          onclick={(e) => closeTab(e, session.id)}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") closeTab(e, session.id);
          }}
        >
          <svg viewBox="0 0 10 10" width="8" height="8">
            <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" />
          </svg>
        </span>
      </div>
    {/each}
    <button
      class="tile-tab-add"
      aria-label={t("tabs.new")}
      bind:this={addButtonEl}
      onclick={toggleShellPicker}
    >
      <svg viewBox="0 0 10 10" width="10" height="10">
        <path d="M5 0v10M0 5h10" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
  </div>

  {#if shellPickerOpen}
    <div use:portal>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="shell-picker-backdrop" onpointerdown={closeShellPicker}></div>
      <div class="shell-picker-panel" style="top:{shellPickerPos.top}px; left:{shellPickerPos.left}px" role="listbox">
        {#each shells as shell (shell.id)}
          <button
            class="shell-picker-item"
            onmouseenter={() => (recentOpen = false)}
            onclick={() => pickShell(shell)}
          >
            <ConsoleIcon kind={shell.id} />
            <span class="shell-picker-label">{shell.display_name}</span>
          </button>
        {/each}
        {#if shells.length === 0}
          <p class="shell-picker-empty">{t("tabs.loadingShells")}</p>
        {/if}
        <div class="shell-picker-divider"></div>
        {#each AGENTS as tool (tool)}
          {@const installed = agentClis?.[tool] ?? true}
          <button
            class="shell-picker-item"
            disabled={!installed}
            onmouseenter={() => (recentOpen = false)}
            onclick={() => pickAgent(tool)}
          >
            <ConsoleIcon kind={tool} />
            <span class="shell-picker-label">{TOOL_DISPLAY_NAMES[tool]}</span>
            {#if !installed}
              <span class="shell-picker-note">{t("tabs.notInstalled")}</span>
            {/if}
          </button>
        {/each}
        {#if recentSessions.length > 0}
          <div class="shell-picker-divider"></div>
          <button
            class="shell-picker-item shell-picker-submenu-row"
            class:open={recentOpen}
            bind:this={recentRowEl}
            aria-haspopup="menu"
            aria-expanded={recentOpen}
            onmouseenter={openRecent}
            onclick={() => (recentOpen ? (recentOpen = false) : openRecent())}
          >
            <span>{t("aiMode.recent")}</span>
            <svg viewBox="0 0 6 10" width="6" height="10" aria-hidden="true">
              <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
            </svg>
          </button>
        {/if}
      </div>
      {#if recentOpen}
        <div class="recent-submenu" style="top:{recentPos.top}px; left:{recentPos.left}px" role="menu">
          {#each recentSessions as session (session.agentSessionId)}
            <button
              class="shell-picker-item"
              role="menuitem"
              title={session.cwd ?? session.projectDir ?? ""}
              onclick={() => pickRecent(session)}
            >
              {sessionLabel(session)}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <div class="tile-toolbar">
    <button title={t("tabs.splitVertical")} onclick={() => void split("row")}>
      <svg viewBox="0 0 12 12" width="11" height="11">
        <rect x="0.5" y="0.5" width="11" height="11" fill="none" stroke="currentColor" />
        <line x1="6" y1="0.5" x2="6" y2="11.5" stroke="currentColor" />
      </svg>
    </button>
    <button title={t("tabs.splitHorizontal")} onclick={() => void split("col")}>
      <svg viewBox="0 0 12 12" width="11" height="11">
        <rect x="0.5" y="0.5" width="11" height="11" fill="none" stroke="currentColor" />
        <line x1="0.5" y1="6" x2="11.5" y2="6" stroke="currentColor" />
      </svg>
    </button>
    <button
      title={t(workspaceStore.zoomedTileId === tile.id ? "tabs.restore" : "tabs.zoom")}
      aria-label={t(workspaceStore.zoomedTileId === tile.id ? "tabs.restore" : "tabs.zoom")}
      onclick={() => workspaceStore.toggleZoom(tile.id)}
    >
      {#if workspaceStore.zoomedTileId === tile.id}
        <svg viewBox="0 0 12 12" width="11" height="11">
          <path d="M7.5 1v3.5H11M4.5 11V7.5H1" fill="none" stroke="currentColor" />
          <path d="M11 1L7.5 4.5M1 11l3.5-3.5" stroke="currentColor" />
        </svg>
      {:else}
        <svg viewBox="0 0 12 12" width="11" height="11">
          <path d="M1 4.5V1h3.5M11 7.5V11H7.5" fill="none" stroke="currentColor" />
          <path d="M1 1l3.5 3.5M11 11l-3.5-3.5" stroke="currentColor" />
        </svg>
      {/if}
    </button>
  </div>
</div>

<style>
  /* Every element, not just body: the console under the pointer sets its own
     text cursor (xterm's), which would otherwise win over the dragging hand. */
  :global(body.termini-dragging-tab),
  :global(body.termini-dragging-tab *) {
    cursor: grabbing !important;
  }

  .tile-chrome {
    position: absolute;
    display: flex;
    flex-direction: column;
    pointer-events: none;
    box-sizing: border-box;
    z-index: 8;
    transition:
      left 140ms ease,
      top 140ms ease,
      width 140ms ease,
      height 140ms ease;
  }

  /* Suppressed during a divider drag (SplitView's startDrag toggles this
     body class) — same reasoning as .session-slot's identical rule. */
  :global(body.termini-dragging-divider) .tile-chrome {
    transition: none;
  }

  .tile-tabstrip {
    pointer-events: auto;
    display: flex;
    align-items: stretch;
    height: 28px;
    flex-shrink: 0;
    background: var(--tabbar-bg, #181818);
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    overflow-x: auto;
    scrollbar-width: none;
  }

  /* Toggled imperatively via classList (not a Svelte class: directive) since the
     drop target is discovered by geometry during a manual pointer drag. The
     whole tile lights up: dropping anywhere on it moves the tab there. */
  :global(.tile-chrome.drag-over)::after {
    content: "";
    position: absolute;
    inset: 0;
    border: 2px dashed var(--pane-active-border, #3a6df0);
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 8%, transparent);
    pointer-events: none;
  }

  /* The tab being dragged, left behind as a placeholder. */
  :global(.tile-tab.drag-source) {
    opacity: 0.35;
  }

  /* The copy of the tab that follows the pointer (see createGhost). Moved
     with transform, so its left/top stay at the origin. */
  :global(.tile-tab.tab-drag-ghost) {
    position: fixed;
    left: 0;
    top: 0;
    z-index: 1000;
    pointer-events: none;
    box-sizing: border-box;
    opacity: 0.92;
    border-radius: 4px;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.45);
    cursor: grabbing;
  }

  :global(.tile-tab.drag-insert-before) {
    box-shadow: inset 2px 0 0 var(--pane-active-border, #3a6df0);
  }

  :global(.tile-tab.drag-insert-after) {
    box-shadow: inset -2px 0 0 var(--pane-active-border, #3a6df0);
  }

  .tile-tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    max-width: 160px;
    min-width: 70px;
    border: none;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: var(--tabbar-fg, #9a9a9a);
    font-size: 11px;
    cursor: pointer;
    user-select: none;
    touch-action: none;
  }

  .tile-tab.active {
    background: var(--tabbar-active-bg, #1e1e1e);
    color: var(--tabbar-active-fg, #f0f0f0);
  }

  /* Distinguishes the active tab of the currently-focused tile (the one
     receiving keyboard input) from active tabs sitting in other, unfocused
     split panes — otherwise every pane's active tab looks identical. */
  .tile-chrome.focused .tile-tab.active {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.35));
    box-shadow: inset 0 -2px 0 0 var(--tab-focus-border, #3a6df0);
  }

  .tile-tab-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }

  .tile-tab-ai-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .tile-tab-close {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: 3px;
    opacity: 0.7;
    flex-shrink: 0;
  }

  .tile-tab-close:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.12);
  }

  .tile-tab-add {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    flex-shrink: 0;
    border: none;
    background: transparent;
    color: var(--tabbar-fg, #9a9a9a);
    cursor: pointer;
  }

  .tile-tab-add:hover {
    color: var(--tabbar-active-fg, #f0f0f0);
  }

  /* position:fixed with JS-computed coordinates, rendered as a sibling of
     .tile-tabstrip rather than nested/absolute inside it — .tile-tabstrip
     sets overflow-x:auto, which per the CSS spec forces overflow-y to auto
     too, silently clipping a nested absolutely-positioned panel. */
  .shell-picker-backdrop {
    position: fixed;
    inset: 0;
    pointer-events: auto;
    z-index: 199;
  }

  .shell-picker-panel {
    position: fixed;
    pointer-events: auto;
    z-index: 200;
    width: 200px;
    max-height: 300px;
    overflow-y: auto;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    display: flex;
    flex-direction: column;
    padding: 4px;
  }

  .shell-picker-item {
    /* Past the list's max-height the panel has to scroll, not squeeze its
       rows: overflow:hidden lets a flex item shrink below its content, so
       without this a long "Recent" list crushed every row flat. */
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
    padding: 7px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--tabbar-active-fg, #f0f0f0);
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .shell-picker-item:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .shell-picker-item:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .shell-picker-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .shell-picker-note {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 10px;
    opacity: 0.8;
  }

  .shell-picker-empty {
    padding: 8px 10px;
    font-size: 11px;
    opacity: 0.6;
    margin: 0;
  }

  .shell-picker-divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--tabbar-border, #2a2a2a);
    flex-shrink: 0;
  }

  .shell-picker-submenu-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .shell-picker-submenu-row.open {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  /* Above the picker (200), which is above the backdrop (199). */
  .recent-submenu {
    position: fixed;
    pointer-events: auto;
    z-index: 201;
    width: 240px;
    max-height: 240px;
    overflow-y: auto;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    display: flex;
    flex-direction: column;
    padding: 4px;
  }

  .tile-toolbar {
    pointer-events: auto;
    position: absolute;
    top: 32px;
    right: 4px;
    z-index: 10;
    display: flex;
    gap: 2px;
    background: rgba(20, 20, 20, 0.7);
    border-radius: 4px;
    padding: 2px;
    opacity: 0.4;
    transition: opacity 0.1s;
  }

  .tile-toolbar:hover {
    opacity: 1;
  }

  .tile-toolbar button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border: none;
    background: transparent;
    color: #cfcfcf;
    cursor: pointer;
    border-radius: 3px;
  }

  .tile-toolbar button:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #fff;
  }
</style>
