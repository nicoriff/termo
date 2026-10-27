<script lang="ts">
  import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
  import type { AiSessionStatus } from "$lib/ai-mode/classifier";
  import { sessionFolder, workspaceStore } from "./workspace-store.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { resolveShellArgs } from "$lib/core/shell-args";
  import { resolveDefaultShell } from "$lib/core/default-shell";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";

  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { ShellInfo } from "$lib/core/types";
  import {
    dropZoneAt,
    fileName,
    isConsoleTab,
    type DropZone,
    type PaneInsets,
    type PaneRect,
    type TileNode,
  } from "./split-tree";
  import { GROUP_COLORS, stripItems } from "$lib/tab-groups/tab-groups";
  import { tabGroupsStore } from "$lib/tab-groups/tab-groups-store.svelte";
  import { fileTabs } from "$lib/editor/file-tabs.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { agentSessionsStore } from "$lib/ai-mode/agent-sessions-store.svelte";
  import { canRestore, restoreSession, sessionLabel } from "$lib/ai-mode/restore-session";
  import type { AgentSession } from "$lib/core/agent-sessions-client";
  import type { AgentFavorite } from "$lib/core/agent-favorites-client";
  import { favoriteDraftForTab, favoriteLabel, favoritesStore } from "$lib/ai-mode/favorites-store.svelte";
  import { backgroundLabel, newTabOptions, programName } from "./new-tab-options.svelte";
  import { killKeptConsole, type KeptConsole } from "$lib/core/pty-client";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { requestAgentLaunch, requestLaunch } from "$lib/agent-launch/request-launch";
  import { TOOL_IDS, type ToolId } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { menuKeys } from "$lib/core/menu-keys";
  import { untrack } from "svelte";

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
  /** The "+ ˅" pair: the menu opens under it, right-aligned to it. */
  let addGroupEl: HTMLDivElement | undefined;

  /** The "+" half: one click, a console like the one this tile is showing —
   * the same thing Ctrl+T does for the focused tile. */
  function newTabHere() {
    closeShellPicker();
    workspaceStore.newTabLikeActive(tile.id);
  }

  function toggleShellPicker() {
    shellPickerOpen = !shellPickerOpen;
    submenu = null;
    if (shellPickerOpen && addGroupEl) {
      // Rendered via position:fixed (not nested/absolute) because
      // .tile-tabstrip sets overflow-x:auto, which per the CSS spec forces
      // overflow-y to auto too — a nested absolutely-positioned panel got
      // silently clipped by that, even though it opened correctly.
      //
      // Anchored right-aligned to the button by default, then clamped into
      // the window on both axes — in a narrow split tile (e.g. the left
      // pane of a side-by-side split), right-aligning unclamped could push
      // the panel's left edge past the window's left edge entirely.
      const box = addGroupEl.getBoundingClientRect();
      const left = Math.max(
        SHELL_PICKER_MARGIN,
        Math.min(box.right - SHELL_PICKER_WIDTH, window.innerWidth - SHELL_PICKER_WIDTH - SHELL_PICKER_MARGIN),
      );
      const top = Math.min(box.bottom + 4, window.innerHeight - SHELL_PICKER_MAX_HEIGHT - SHELL_PICKER_MARGIN);
      shellPickerPos = { top, left };
    }
    // Already loaded at startup; this only picks up changes, in the background.
    if (shellPickerOpen) newTabOptions.refresh();
  }

  // The shortcut for this menu (see uiState.openTabMenu): the focused tile's
  // opens. Only a change counts, not the value it had when this tile mounted.
  let seenTabMenuNonce = uiState.tabMenuNonce;
  $effect(() => {
    const nonce = uiState.tabMenuNonce;
    if (nonce === seenTabMenuNonce) return;
    seenTabMenuNonce = nonce;
    if (untrack(() => workspaceStore.focusedTileId) === tile.id) untrack(toggleShellPicker);
  });

  const shells = $derived(newTabOptions.shells);
  const agentClis = $derived(newTabOptions.agentClis);
  const AGENTS = TOOL_IDS;

  /** Asks where to run it first, in this tile rather than the focused one. */
  function pickAgent(tool: ToolId) {
    closeShellPicker();
    requestAgentLaunch(tool, tile.id);
  }

  function closeShellPicker() {
    shellPickerOpen = false;
    submenu = null;
  }

  /** Shift+click is the keyboard-hand's way to the folder icon. */
  function onShellClick(event: MouseEvent, shell: ShellInfo) {
    if (event.shiftKey) pickShellElsewhere(shell);
    else pickShell(shell);
  }

  /** The row's folder icon: pick a folder first (see AgentLaunchDialog), then
   * open this shell there. */
  function pickShellElsewhere(shell: ShellInfo) {
    closeShellPicker();
    requestLaunch(
      {
        kind: "shell",
        path: shell.path,
        args: resolveShellArgs(shell.id, shell.args, appConfigStore.psreadlineMenuComplete),
        name: shell.display_name,
        iconId: shell.id,
      },
      tile.id,
    );
  }

  function pickShell(shell: ShellInfo) {
    const args = resolveShellArgs(shell.id, shell.args, appConfigStore.psreadlineMenuComplete);
    // Opens where this tile's active console is now, like the new-tab
    // shortcut does (see workspaceStore.newTabLikeFocused).
    const active = tile.sessions.find((session) => session.id === tile.activeSessionId);
    workspaceStore.addTab(tile.id, shell.path, args, shell.display_name, active ? sessionFolder(active) : undefined);
    closeShellPicker();
  }

  /** Past agent conversations, reopened in THIS tile rather than the focused
   * one — the picker belongs to this tile's "+" button. Only offered while AI
   * Mode is on (it's what records them), and only those whose id is safe to
   * type into a shell. */
  const recentSessions = $derived(
    appConfigStore.aiModeEnabled ? agentSessionsStore.sessions.filter(canRestore) : [],
  );

  /** The conversations marked as favorites, reopened the same way. */
  const favoriteSessions = $derived(
    appConfigStore.aiModeEnabled ? favoritesStore.favorites.filter(canRestore) : [],
  );

  /** Which of the picker's flyouts is open. */
  type Submenu = "recent" | "favorites" | "background" | "zellij";
  let submenu = $state<Submenu | null>(null);
  const recentOpen = $derived(submenu === "recent");
  let recentPos = $state({ top: 0, left: 0 });
  // $state (unlike addButtonEl): these rows live inside an {#if}, so bind:this
  // reassigns them as the rows appear and disappear.
  let recentRowEl = $state<HTMLButtonElement | undefined>(undefined);
  let favoritesRowEl = $state<HTMLButtonElement | undefined>(undefined);
  let backgroundRowEl = $state<HTMLButtonElement | undefined>(undefined);
  let zellijRowEl = $state<HTMLButtonElement | undefined>(undefined);

  /** A Zellij session in a new tab: an existing one (`name`), or a new one. */
  function pickZellij(name?: string) {
    const zellij = newTabOptions.zellij;
    if (!zellij) return;
    closeShellPicker();
    const title = name ? `${t("tabs.zellij")} · ${name}` : t("tabs.zellij");
    workspaceStore.addTab(tile.id, zellij.path, name ? ["attach", name] : [], title);
  }

  /** Consoles still running that no window shows (see src-tauri/src/mux/). */
  const backgroundConsoles = $derived(newTabOptions.background);

  function pickBackground(kept: KeptConsole) {
    closeShellPicker();
    workspaceStore.attachTab(tile.id, kept.id, backgroundLabel(kept), kept.cwd ?? undefined);
  }

  async function closeBackground(kept: KeptConsole) {
    await killKeptConsole(kept.id).catch(() => {});
    await newTabOptions.refreshBackground();
    if (newTabOptions.background.length === 0) submenu = null;
  }

  /** Flies out beside the picker, flipping to its left when the window edge is
   * too close — same clamping idea as the picker itself. */
  function openSubmenu(kind: Submenu) {
    const rows = { recent: recentRowEl, favorites: favoritesRowEl, background: backgroundRowEl, zellij: zellijRowEl };
    const row = rows[kind];
    if (!row) return;
    const box = row.getBoundingClientRect();
    const toTheRight = box.right + 4;
    const left =
      toTheRight + RECENT_SUBMENU_WIDTH + SHELL_PICKER_MARGIN > window.innerWidth
        ? Math.max(SHELL_PICKER_MARGIN, box.left - 4 - RECENT_SUBMENU_WIDTH)
        : toTheRight;
    const top = Math.min(box.top, window.innerHeight - SHELL_PICKER_MAX_HEIGHT - SHELL_PICKER_MARGIN);
    recentPos = { top, left };
    submenu = kind;
  }

  function toggleSubmenu(kind: Submenu) {
    if (submenu === kind) submenu = null;
    else openSubmenu(kind);
  }

  function pickFavorite(favorite: AgentFavorite) {
    closeShellPicker();
    void restoreSession(favorite, tile.id);
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
  //
  // A group's chip drags the same way, carrying all of the group's tabs in
  // this tile with it; let go without moving, it folds or unfolds the group.
  type Dragged = { kind: "tab"; sessionId: string } | { kind: "group"; groupId: string };

  function onTabPointerDown(event: PointerEvent, sessionId: string) {
    startDrag(event, { kind: "tab", sessionId });
  }

  function startDrag(event: PointerEvent, dragged: Dragged) {
    if (event.button !== 0) return;
    const startX = event.clientX;
    const startY = event.clientY;
    const sourceTileId = tile.id;
    const groupId = dragged.kind === "group" ? dragged.groupId : null;
    const sessionId = dragged.kind === "tab" ? dragged.sessionId : null;
    // Whether splitting its own tile would leave something beside it.
    const leavesSomething = groupId
      ? tile.sessions.some((session) => session.groupId !== groupId)
      : tile.sessions.length >= 2;
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
      ghost.removeAttribute("data-group-id");
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

    /** The tab — or group chip — under the pointer. */
    function currentDropTab(e: PointerEvent): HTMLElement | null {
      const el = document.elementFromPoint(e.clientX, e.clientY);
      return el instanceof Element ? el.closest<HTMLElement>(".tile-tab, .tile-group-chip") : null;
    }

    /** Over the dragged group itself: its chip or one of its tabs. */
    function overOwnGroup(target: HTMLElement | null): boolean {
      return !!groupId && !!target && target.dataset.groupId === groupId;
    }

    function clearHighlight() {
      document
        .querySelectorAll(".drag-insert-before, .drag-insert-after")
        .forEach((el) => el.classList.remove("drag-insert-before", "drag-insert-after"));
      document.querySelectorAll(".join-target").forEach((el) => el.classList.remove("join-target"));
    }

    /** The group a dragged tab would join if let go here: the one whose tab
     * is under the pointer, as Chrome does. Not for a group being dragged. */
    function joinTarget(e: PointerEvent): { groupId: string; tab: HTMLElement } | null {
      if (!sessionId) return null;
      const over = currentDropTab(e);
      const group = over?.classList.contains("tile-tab") ? over.dataset.groupId : undefined;
      return over && group ? { groupId: group, tab: over } : null;
    }

    /** Shows the join coming: the dragged tab takes the group's color, and
     * the group's chip and tabs in that strip light up. Off a group, on a
     * strip, it shows ungrouped — where it'd land outside any. */
    function showJoin(e: PointerEvent, drop: Drop) {
      const join = joinTarget(e);
      if (join) {
        const color = join.tab.style.getPropertyValue("--group-color");
        ghost?.style.setProperty("--group-color", color);
        ghost?.classList.add("in-group");
        join.tab
          .closest(".tile-tabstrip")
          ?.querySelectorAll<HTMLElement>(`[data-group-id="${join.groupId}"]`)
          .forEach((el) => el.classList.add("join-target"));
      } else if (drop.kind === "strip") {
        ghost?.classList.remove("in-group");
      } else if (sourceTab.classList.contains("in-group")) {
        ghost?.style.setProperty("--group-color", sourceTab.style.getPropertyValue("--group-color"));
        ghost?.classList.add("in-group");
      }
    }

    /** What letting go right here would do — the preview and the drop both
     * read it, so what's shown is always what happens. */
    type Drop =
      | { kind: "none" }
      | { kind: "strip" | "tile"; tileId: string; chrome: HTMLElement }
      | { kind: "split"; tileId: string; chrome: HTMLElement; side: Exclude<DropZone, "center"> };

    function dropAt(e: PointerEvent): Drop {
      const chrome = currentDropTile(e);
      const tileId = chrome?.dataset.tileId;
      if (!chrome || !tileId) return { kind: "none" };
      if (document.elementFromPoint(e.clientX, e.clientY)?.closest(".tile-tabstrip")) {
        // Over itself it stays put; anywhere else on a strip it's inserted.
        const over = currentDropTab(e);
        return over === sourceTab || overOwnGroup(over) ? { kind: "none" } : { kind: "strip", tileId, chrome };
      }
      // Measured on the console below the strip, where the edges are.
      const box = chrome.getBoundingClientRect();
      const stripBottom = chrome.querySelector(".tile-tabstrip")?.getBoundingClientRect().bottom ?? box.top;
      const side = dropZoneAt((e.clientX - box.left) / box.width, (e.clientY - stripBottom) / (box.bottom - stripBottom));
      const own = tileId === sourceTileId;
      if (side !== "center") {
        // A tile's only tab can't split it: there'd be nothing left beside it.
        return own && !leavesSomething ? { kind: "none" } : { kind: "split", tileId, chrome, side };
      }
      // Its own console keeps it where it was: only the strip reorders.
      return own ? { kind: "none" } : { kind: "tile", tileId, chrome };
    }

    /** The box that shows where the tab would land: the whole tile, or the
     * half a new group would take. One element for the whole drag, so moving
     * between zones animates it from one shape to the next. */
    let preview: HTMLElement | null = null;

    function showPreview(drop: Drop) {
      if (drop.kind === "none") {
        preview?.classList.remove("visible");
        return;
      }
      const box = drop.chrome.getBoundingClientRect();
      let { left, top, width, height } = box;
      if (drop.kind === "split") {
        if (drop.side === "left" || drop.side === "right") width /= 2;
        else height /= 2;
        if (drop.side === "right") left += width;
        if (drop.side === "bottom") top += height;
      }
      if (!preview) {
        preview = document.createElement("div");
        preview.className = "tab-drop-preview";
        preview.appendChild(document.createElement("span")).className = "tab-drop-label";
        document.body.appendChild(preview);
      }
      // Appearing, it starts from where it's needed rather than sliding in
      // from wherever it last was.
      const appearing = !preview.classList.contains("visible");
      if (appearing) preview.style.transition = "none";
      Object.assign(preview.style, { left: `${left}px`, top: `${top}px`, width: `${width}px`, height: `${height}px` });
      preview.firstElementChild!.textContent = drop.kind === "split" ? t("tabs.newPane") : "";
      preview.classList.toggle("split", drop.kind === "split");
      if (appearing) {
        void preview.offsetWidth;
        preview.style.transition = "";
      }
      preview.classList.add("visible");
    }

    // Which existing tab (if any) the pointer is currently over, and whether
    // it should land before or after that tab — used both to preview the
    // drop position and, on release, to compute moveSession's beforeSessionId.
    // A chip only takes things ahead of it: just after it is the group's
    // first tab, which "before that tab" already covers.
    function hoveredInsertion(e: PointerEvent): { tab: HTMLElement; before: boolean } | null {
      const overTab = currentDropTab(e);
      if (!overTab || (sessionId && overTab.dataset.sessionId === sessionId) || overOwnGroup(overTab)) return null;
      if (overTab.classList.contains("tile-group-chip")) return { tab: overTab, before: true };
      const box = overTab.getBoundingClientRect();
      return { tab: overTab, before: e.clientX < box.left + box.width / 2 };
    }

    function onMove(e: PointerEvent) {
      if (!dragging && Math.hypot(e.clientX - startX, e.clientY - startY) > 4) {
        dragging = true;
        document.body.classList.add("termo-dragging-tab");
        createGhost();
      }
      if (!dragging) return;
      moveGhost(e);
      clearHighlight();
      const drop = dropAt(e);
      showPreview(drop);
      showJoin(e, drop);
      const insertion = hoveredInsertion(e);
      insertion?.tab.classList.add(insertion.before ? "drag-insert-before" : "drag-insert-after");
    }

    function onUp(e: PointerEvent) {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      document.body.classList.remove("termo-dragging-tab");
      clearHighlight();
      ghost?.remove();
      ghost = null;
      preview?.remove();
      preview = null;
      sourceTab.classList.remove("drag-source");

      if (dragging) {
        const drop = dropAt(e);
        if (drop.kind === "split") {
          if (groupId) workspaceStore.splitWithGroup(sourceTileId, drop.tileId, groupId, drop.side);
          else if (sessionId) workspaceStore.splitWithSession(sourceTileId, drop.tileId, sessionId, drop.side);
        } else if (drop.kind !== "none") {
          const insertion = hoveredInsertion(e);
          let beforeSessionId: string | undefined;
          if (insertion) {
            beforeSessionId = insertion.before
              ? insertion.tab.dataset.sessionId
              : insertion.tab.nextElementSibling instanceof HTMLElement
                ? insertion.tab.nextElementSibling.dataset.sessionId
                : undefined;
          }
          if (groupId) workspaceStore.moveGroup(sourceTileId, drop.tileId, groupId, beforeSessionId);
          else if (sessionId)
            workspaceStore.moveSession(
              sourceTileId,
              drop.tileId,
              sessionId,
              beforeSessionId,
              drop.kind === "strip",
              joinTarget(e)?.groupId,
            );
        }
      } else if (groupId) {
        tabGroupsStore.toggleCollapsed(groupId);
      } else if (sessionId) {
        workspaceStore.setActiveSession(sourceTileId, sessionId);
      }
    }

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  /** What the strip shows: each group's chip ahead of its tabs, which a
   * folded group hides (see tab-groups.ts). */
  const items = $derived(stripItems(tile.sessions, tabGroupsStore.groups, tile.activeSessionId));

  function onChipPointerDown(event: PointerEvent, groupId: string) {
    startDrag(event, { kind: "group", groupId });
  }

  function onChipContextMenu(event: MouseEvent, groupId: string) {
    event.preventDefault();
    tabGroupsStore.openEditor(groupId, event.clientX, event.clientY);
  }

  /** Right-clicking a tab: its own small menu, at the pointer. The tab becomes
   * the active one first, as a click would make it. */
  const TAB_MENU_WIDTH = 220;
  let tabMenu = $state<{ x: number; y: number; sessionId: string } | null>(null);
  const tabMenuFavorite = $derived(
    tabMenu && appConfigStore.aiModeEnabled ? favoriteDraftForTab(tabMenu.sessionId) : null,
  );
  const tabMenuSession = $derived(
    tabMenu ? (tile.sessions.find((session) => session.id === tabMenu!.sessionId) ?? null) : null,
  );
  /** The groups the tab could move to: all but its own. */
  const otherGroups = $derived(
    tabGroupsStore.groups.filter((group) => group.id !== tabMenuSession?.groupId),
  );
  let groupSubmenu = $state<{ top: number; left: number } | null>(null);
  let moveRowEl = $state<HTMLButtonElement | undefined>(undefined);

  function onTabContextMenu(event: MouseEvent, sessionId: string) {
    event.preventDefault();
    workspaceStore.setActiveSession(tile.id, sessionId);
    const x = Math.min(event.clientX, window.innerWidth - TAB_MENU_WIDTH - SHELL_PICKER_MARGIN);
    groupSubmenu = null;
    tabMenu = { x, y: event.clientY, sessionId };
  }

  function closeTabMenu() {
    tabMenu = null;
    groupSubmenu = null;
  }

  /** Keeps a menu inside the window: one opened low on the screen goes up. */
  function keepInWindow(node: HTMLElement) {
    const box = node.getBoundingClientRect();
    const overflow = box.bottom - (window.innerHeight - SHELL_PICKER_MARGIN);
    if (overflow > 0) node.style.top = `${Math.max(SHELL_PICKER_MARGIN, box.top - overflow)}px`;
  }

  function openGroupSubmenu() {
    if (!moveRowEl) return;
    const box = moveRowEl.getBoundingClientRect();
    const toTheRight = box.right + 4;
    const left =
      toTheRight + RECENT_SUBMENU_WIDTH + SHELL_PICKER_MARGIN > window.innerWidth
        ? Math.max(SHELL_PICKER_MARGIN, box.left - 4 - RECENT_SUBMENU_WIDTH)
        : toTheRight;
    groupSubmenu = { top: box.top, left };
  }

  /** A new group with this tab in it, its editor open to name it — as
   * Chrome does. */
  function addToNewGroup() {
    const menu = tabMenu;
    closeTabMenu();
    if (!menu) return;
    const groupId = tabGroupsStore.create([menu.sessionId]);
    tabGroupsStore.openEditor(groupId, menu.x, menu.y);
  }

  function moveToGroup(groupId: string) {
    const sessionId = tabMenu?.sessionId;
    closeTabMenu();
    if (sessionId) tabGroupsStore.assign(sessionId, groupId);
  }

  function removeFromGroup() {
    const sessionId = tabMenu?.sessionId;
    closeTabMenu();
    if (sessionId) tabGroupsStore.assign(sessionId, null);
  }

  /** The tab goes to a new pane on that side of its tile. When it's all the
   * tile has, the new pane gets a console like it instead, in its folder and
   * its group. */
  async function splitFromTabMenu(side: "right" | "left" | "bottom") {
    const session = tabMenuSession;
    closeTabMenu();
    if (!session) return;
    if (tile.sessions.length >= 2) {
      workspaceStore.splitWithSession(tile.id, tile.id, session.id, side);
      return;
    }
    const shell =
      isConsoleTab(session) && session.shellPath
        ? { path: session.shellPath, args: session.shellArgs, title: session.title }
        : await resolveDefaultShell();
    workspaceStore.splitTile(tile.id, side === "bottom" ? "col" : "row", shell, {
      newFirst: side === "left",
      cwd: sessionFolder(session),
      groupId: session.groupId,
    });
  }

  function favoriteFromTabMenu() {
    const draft = tabMenuFavorite;
    closeTabMenu();
    if (draft) uiState.openFavoriteDialog(draft);
  }

  function closeFromTabMenu(event: Event) {
    const sessionId = tabMenu?.sessionId;
    closeTabMenu();
    if (sessionId) closeTab(event, sessionId);
  }

  function closeTab(event: Event, sessionId: string) {
    event.stopPropagation();
    // A file with unsaved changes asks first (see CloseFileDialog).
    const session = tile.sessions.find((candidate) => candidate.id === sessionId);
    if (session?.filePath && fileTabs.isDirty(sessionId)) {
      uiState.requestCloseFile({ tileId: tile.id, sessionId, name: fileName(session.filePath) });
      return;
    }
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
  class:fill-width={appConfigStore.tabsFillWidth}
  style="left:calc({rect.left * 100}% + {insets.left}px); top:calc({rect.top * 100}% + {insets.top}px); width:calc({rect.width *
    100}% - {insets.left + insets.right}px); height:calc({rect.height * 100}% - {insets.top + insets.bottom}px)"
>
  <div class="tile-tabstrip" data-tile-id={tile.id}>
    {#each items as item (item.kind === "chip" ? `chip:${item.group.id}` : item.session.id)}
      {#if item.kind === "chip"}
        {@const name = item.group.name || t("tabGroups.unnamed")}
        {@const firstTab = tile.sessions.find((session) => session.groupId === item.group.id)}
        <div
          class="tile-group-chip"
          class:collapsed={item.group.collapsed}
          class:unnamed={!item.group.name}
          style="--group-color:{GROUP_COLORS[item.group.color]}"
          data-group-id={item.group.id}
          data-session-id={firstTab?.id}
          role="button"
          tabindex="0"
          aria-expanded={!item.group.collapsed}
          aria-label={t(item.group.collapsed ? "tabGroups.expand" : "tabGroups.collapse", { name })}
          title={t("tabGroups.chipHint", { name })}
          onpointerdown={(e) => onChipPointerDown(e, item.group.id)}
          oncontextmenu={(e) => onChipContextMenu(e, item.group.id)}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              tabGroupsStore.toggleCollapsed(item.group.id);
            }
          }}
        >
          <span class="tile-group-chip-name">{item.group.name}</span>
          {#if item.group.collapsed}
            <span class="tile-group-chip-count">{item.count}</span>
          {/if}
        </div>
      {:else}
      {@const session = item.session}
      {@const aiStatus = aiStatusStore.byId[session.id]}
      {@const favorite = appConfigStore.aiModeEnabled ? favoritesStore.forTab(session.id) : null}
      <div
        class="tile-tab"
        class:active={tile.activeSessionId === session.id}
        class:in-group={!!item.group}
        style={item.group ? `--group-color:${GROUP_COLORS[item.group.color]}` : undefined}
        data-session-id={session.id}
        data-group-id={item.group?.id}
        role="tab"
        tabindex="0"
        aria-selected={tile.activeSessionId === session.id}
        onpointerdown={(e) => onTabPointerDown(e, session.id)}
        oncontextmenu={(e) => onTabContextMenu(e, session.id)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") workspaceStore.setActiveSession(tile.id, session.id);
        }}
      >
        {#if aiStatus?.tool && aiStatus.status}
          <span class="tile-tab-ai-dot" style="background:{aiDotColor(aiStatus.status)}"></span>
        {/if}
        {#if session.filePath}
          <svg class="tile-tab-file" viewBox="0 0 12 14" aria-hidden="true">
            <path d="M1.5 1.5h6l3 3v8h-9z M7.5 1.5v3h3" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
        {:else if session.diff}
          <svg class="tile-tab-file" viewBox="0 0 14 14" aria-hidden="true">
            <path d="M4 1.5v4M2 3.5h4M2 10.5h4M9 2v10M12 5 9 2 6.5 4.5" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
        {:else if session.gitGraph}
          <svg class="tile-tab-file" viewBox="0 0 14 14" aria-hidden="true">
            <path d="M4 4.2v5.6M4 7.4c3.8 0 5.5-.6 5.5-3" fill="none" stroke="currentColor" stroke-width="1" />
            <circle cx="4" cy="3" r="1.3" fill="none" stroke="currentColor" stroke-width="1" />
            <circle cx="4" cy="11" r="1.3" fill="none" stroke="currentColor" stroke-width="1" />
            <circle cx="9.5" cy="3.4" r="1.3" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
        {:else if session.projectView}
          <svg class="tile-tab-file" viewBox="0 0 14 14" aria-hidden="true">
            <path d="M1.5 3.5h4l1.5 1.5h5.5v7h-11z" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
        {:else if session.orchestrationView?.console}
          <svg class="tile-tab-file" viewBox="0 0 14 14" aria-hidden="true">
            <rect x="1.5" y="2.5" width="11" height="9" rx="1.5" fill="none" stroke="currentColor" stroke-width="1" />
            <path d="M4 5.5l2 1.5-2 1.5M7.5 9h2.5" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
        {:else if session.orchestrationView}
          <svg class="tile-tab-file" viewBox="0 0 14 14" aria-hidden="true">
            <circle cx="7" cy="3" r="1.6" fill="none" stroke="currentColor" stroke-width="1" />
            <circle cx="3" cy="11" r="1.6" fill="none" stroke="currentColor" stroke-width="1" />
            <circle cx="11" cy="11" r="1.6" fill="none" stroke="currentColor" stroke-width="1" />
            <path d="M7 4.6v2.4M7 7L3.6 9.6M7 7l3.4 2.6" fill="none" stroke="currentColor" stroke-width="1" />
          </svg>
        {:else if !session.agentRun}
          <span class="tile-tab-kind"><ConsoleIcon kind={tabTitleStore.kindFor(session)} size={12} /></span>
        {:else if session.agentRun}
          <svg class="tile-tab-file" viewBox="0 0 14 14" aria-hidden="true">
            <rect x="2" y="4" width="10" height="8" rx="2" fill="none" stroke="currentColor" stroke-width="1" />
            <path d="M7 4V2 M5 8h.01 M9 8h.01" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
          </svg>
        {/if}
        {#if favorite}
          <span class="tile-tab-favorite" title={t("favorites.tabHint", { name: favoriteLabel(favorite) })}>★</span>
        {/if}
        <span
          class="tile-tab-title"
          title={favorite ? t("favorites.tabHint", { name: favoriteLabel(favorite) }) : session.filePath ??
            session.diff?.path ??
            (session.agentRun || session.projectView || session.orchestrationView || session.gitGraph
              ? tabTitleStore.titleFor(session.id, session.title)
              : `${tabTitleStore.typeFor(session)} · ${tabTitleStore.titleFor(session.id, session.title)}`)}
        >
          <!-- A favorite with a name of its own goes by it. -->
          {favorite?.name ?? tabTitleStore.titleFor(session.id, session.title)}
        </span>
        {#if fileTabs.dirty[session.id]}
          <span class="tile-tab-dirty" title={t("editor.unsaved")}>●</span>
        {/if}
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
      {/if}
    {/each}
    <!-- Split button: "+" opens a tab right away, "˅" the full menu. -->
    <div class="tile-tab-add-group" class:open={shellPickerOpen} bind:this={addGroupEl}>
      <button
        class="tile-tab-add"
        aria-label={t("tabs.new")}
        title={`${t("tabs.new")} (${formatKeybindingLabel(appConfigStore.getEffectiveShortcut("newTab"), isMac)})`}
        onclick={newTabHere}
      >
        <svg viewBox="0 0 10 10" width="10" height="10">
          <path d="M5 0v10M0 5h10" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button
        class="tile-tab-more"
        aria-label={t("tabs.moreOptions")}
        title={`${t("tabs.moreOptions")} (${formatKeybindingLabel(appConfigStore.getEffectiveShortcut("tabMenu"), isMac)})`}
        aria-haspopup="menu"
        aria-expanded={shellPickerOpen}
        onclick={toggleShellPicker}
      >
        <svg viewBox="0 0 10 6" width="8" height="5">
          <path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.2" />
        </svg>
      </button>
    </div>
  </div>

  {#if shellPickerOpen}
    <div use:portal>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="shell-picker-backdrop" onpointerdown={closeShellPicker}></div>
      <div
        class="shell-picker-panel"
        style="top:{shellPickerPos.top}px; left:{shellPickerPos.left}px"
        role="menu"
        use:menuKeys={{ close: closeShellPicker }}
        in:fly={{ y: -4, duration: 120, easing: cubicOut }}
      >
        {#each shells as shell (shell.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="shell-picker-row" onmouseenter={() => (submenu = null)}>
            <button
              class="shell-picker-item"
              role="menuitem"
              onclick={(e) => onShellClick(e, shell)}
              onkeydown={(e) => {
                // Shift+Enter: the keyboard's way to the folder icon.
                if (e.key === "Enter" && e.shiftKey) {
                  e.preventDefault();
                  pickShellElsewhere(shell);
                }
              }}
            >
              <ConsoleIcon kind={shell.id} />
              <span class="shell-picker-label">{shell.display_name}</span>
            </button>
            <button
              class="shell-picker-elsewhere"
              tabindex="-1"
              data-menu-skip
              title={t("tabs.openElsewhere")}
              aria-label={t("tabs.openElsewhereFor", { shell: shell.display_name })}
              onclick={() => pickShellElsewhere(shell)}
            >
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <path d="M1.5 4.5v8h13v-7h-6l-1.5-1.5h-5.5z" fill="none" stroke="currentColor" stroke-width="1.1" />
                <path d="M7 9h4.5M9.5 7l2 2-2 2" fill="none" stroke="currentColor" stroke-width="1.1" />
              </svg>
            </button>
          </div>
        {/each}
        {#if shells.length === 0}
          <p class="shell-picker-empty">{t("tabs.loadingShells")}</p>
        {/if}
        <!-- Claude Code and Codex are Modo IA's: only listed with it on. -->
        {#if appConfigStore.aiModeEnabled}
        <div class="shell-picker-divider"></div>
        {/if}
        {#each appConfigStore.aiModeEnabled ? AGENTS : [] as tool (tool)}
          {@const installed = agentClis?.[tool] ?? true}
          <button
            class="shell-picker-item"
            role="menuitem"
            disabled={!installed}
            onmouseenter={() => (submenu = null)}
            onclick={() => pickAgent(tool)}
          >
            <ConsoleIcon kind={tool} />
            <span class="shell-picker-label">{TOOL_DISPLAY_NAMES[tool]}</span>
            {#if !installed}
              <span class="shell-picker-note">{t("tabs.notInstalled")}</span>
            {/if}
          </button>
        {/each}
        {#if recentSessions.length > 0 || favoriteSessions.length > 0 || backgroundConsoles.length > 0 || newTabOptions.tailscale || newTabOptions.zellij}
          <div class="shell-picker-divider"></div>
        {/if}
        {#if newTabOptions.zellij}
          <button
            class="shell-picker-item shell-picker-submenu-row"
            class:open={submenu === "zellij"}
            role="menuitem"
            bind:this={zellijRowEl}
            aria-haspopup="menu"
            aria-expanded={submenu === "zellij"}
            onmouseenter={() => openSubmenu("zellij")}
            onclick={() => toggleSubmenu("zellij")}
          >
            <span>{t("tabs.zellij")}</span>
            <svg viewBox="0 0 6 10" width="6" height="10" aria-hidden="true">
              <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
            </svg>
          </button>
        {/if}
        {#if newTabOptions.tailscale}
          <button
            class="shell-picker-item"
            role="menuitem"
            onmouseenter={() => (submenu = null)}
            onclick={() => {
              closeShellPicker();
              uiState.openRemoteConnect(tile.id);
            }}
          >
            <span class="shell-picker-label">{t("remote.connectMenu")}</span>
          </button>
        {/if}
        {#if backgroundConsoles.length > 0}
          <button
            class="shell-picker-item shell-picker-submenu-row"
            class:open={submenu === "background"}
            role="menuitem"
            title={t("tabs.backgroundHint")}
            bind:this={backgroundRowEl}
            aria-haspopup="menu"
            aria-expanded={submenu === "background"}
            onmouseenter={() => openSubmenu("background")}
            onclick={() => toggleSubmenu("background")}
          >
            <span>{t("tabs.background")} ({backgroundConsoles.length})</span>
            <svg viewBox="0 0 6 10" width="6" height="10" aria-hidden="true">
              <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
            </svg>
          </button>
        {/if}
        {#if favoriteSessions.length > 0}
          <button
            class="shell-picker-item shell-picker-submenu-row"
            class:open={submenu === "favorites"}
            role="menuitem"
            bind:this={favoritesRowEl}
            aria-haspopup="menu"
            aria-expanded={submenu === "favorites"}
            onmouseenter={() => openSubmenu("favorites")}
            onclick={() => toggleSubmenu("favorites")}
          >
            <span><span class="favorite-star" aria-hidden="true">★</span> {t("favorites.title")}</span>
            <svg viewBox="0 0 6 10" width="6" height="10" aria-hidden="true">
              <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
            </svg>
          </button>
        {/if}
        {#if recentSessions.length > 0}
          <button
            class="shell-picker-item shell-picker-submenu-row"
            class:open={recentOpen}
            role="menuitem"
            bind:this={recentRowEl}
            aria-haspopup="menu"
            aria-expanded={recentOpen}
            onmouseenter={() => openSubmenu("recent")}
            onclick={() => toggleSubmenu("recent")}
          >
            <span>{t("aiMode.recent")}</span>
            <svg viewBox="0 0 6 10" width="6" height="10" aria-hidden="true">
              <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
            </svg>
          </button>
        {/if}
      </div>
      {#if submenu === "zellij" && newTabOptions.zellij}
        <div
          class="recent-submenu"
          style="top:{recentPos.top}px; left:{recentPos.left}px"
          role="menu"
          use:menuKeys={{ close: closeShellPicker, back: () => (submenu = null) }}
          in:fly={{ x: -4, duration: 120, easing: cubicOut }}
        >
          <button class="shell-picker-item" role="menuitem" onclick={() => pickZellij()}>
            <span class="shell-picker-label">{t("tabs.zellijNew")}</span>
          </button>
          {#each newTabOptions.zellij.sessions as session (session.name)}
            <button class="shell-picker-item" role="menuitem" onclick={() => pickZellij(session.name)}>
              <span class="shell-picker-label">{session.name}</span>
              {#if session.exited}
                <span class="shell-picker-note">{t("tabs.zellijExited")}</span>
              {/if}
            </button>
          {/each}
        </div>
      {/if}
      {#if submenu === "background"}
        <div
          class="recent-submenu"
          style="top:{recentPos.top}px; left:{recentPos.left}px"
          role="menu"
          use:menuKeys={{ close: closeShellPicker, back: () => (submenu = null) }}
          in:fly={{ x: -4, duration: 120, easing: cubicOut }}
        >
          {#each backgroundConsoles as kept (kept.id)}
            {@const label = backgroundLabel(kept)}
            <div class="shell-picker-row">
              <button
                class="shell-picker-item"
                role="menuitem"
                title={kept.cwd ?? kept.path}
                onclick={() => pickBackground(kept)}
              >
                <ConsoleIcon kind={programName(kept.path)} />
                <span class="shell-picker-label">{label}</span>
              </button>
              <button
                class="shell-picker-elsewhere"
                tabindex="-1"
                data-menu-skip
                title={t("tabs.closeBackground", { name: label })}
                aria-label={t("tabs.closeBackground", { name: label })}
                onclick={() => void closeBackground(kept)}
              >
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M4.5 4.5l7 7M11.5 4.5l-7 7" fill="none" stroke="currentColor" stroke-width="1.2" />
                </svg>
              </button>
            </div>
          {/each}
        </div>
      {/if}
      {#if submenu === "favorites"}
        <div
          class="recent-submenu"
          style="top:{recentPos.top}px; left:{recentPos.left}px"
          role="menu"
          use:menuKeys={{ close: closeShellPicker, back: () => (submenu = null) }}
          in:fly={{ x: -4, duration: 120, easing: cubicOut }}
        >
          {#each favoriteSessions as favorite (favorite.agentSessionId)}
            <button
              class="shell-picker-item"
              role="menuitem"
              title={favorite.cwd ?? favorite.projectDir ?? ""}
              onclick={() => pickFavorite(favorite)}
            >
              <ConsoleIcon kind={favorite.tool} />
              <span class="shell-picker-label">{favoriteLabel(favorite)}</span>
            </button>
          {/each}
        </div>
      {/if}
      {#if recentOpen}
        <div
          class="recent-submenu"
          style="top:{recentPos.top}px; left:{recentPos.left}px"
          role="menu"
          use:menuKeys={{ close: closeShellPicker, back: () => (submenu = null) }}
          in:fly={{ x: -4, duration: 120, easing: cubicOut }}
        >
          {#each recentSessions as session (session.agentSessionId)}
            <button
              class="shell-picker-item"
              role="menuitem"
              title={session.cwd ?? session.projectDir ?? ""}
              onclick={() => pickRecent(session)}
            >
              <ConsoleIcon kind={session.tool} />
              <span class="shell-picker-label">{sessionLabel(session)}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  {#if tabMenu}
    <div use:portal>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="shell-picker-backdrop"
        onpointerdown={closeTabMenu}
        oncontextmenu={(e) => {
          e.preventDefault();
          closeTabMenu();
        }}
      ></div>
      <div
        class="tab-menu"
        style="top:{tabMenu.y}px; left:{tabMenu.x}px"
        role="menu"
        use:keepInWindow
        use:menuKeys={{ close: closeTabMenu }}
      >
        {#if tabMenuFavorite}
          {@const saved = favoritesStore.byId.has(tabMenuFavorite.agentSessionId)}
          <button
            class="shell-picker-item"
            role="menuitem"
            onmouseenter={() => (groupSubmenu = null)}
            onclick={favoriteFromTabMenu}
          >
            <span class="tab-menu-star" class:on={saved} aria-hidden="true">{saved ? "★" : "☆"}</span>
            <span class="shell-picker-label">{t(saved ? "favorites.edit" : "favorites.add")}</span>
            <span class="tab-menu-hint">
              {formatKeybindingLabel(appConfigStore.getEffectiveShortcut("favoriteTab"), isMac)}
            </span>
          </button>
          <div class="shell-picker-divider"></div>
        {/if}
        <button class="shell-picker-item" role="menuitem" onmouseenter={() => (groupSubmenu = null)} onclick={addToNewGroup}>
          <span class="tab-menu-star" aria-hidden="true">
            <svg viewBox="0 0 12 12" width="11" height="11">
              <rect x="1" y="3" width="7" height="7" rx="1.5" fill="none" stroke="currentColor" />
              <path d="M9 1v4M7 3h4" stroke="currentColor" />
            </svg>
          </span>
          <span class="shell-picker-label">{t("tabGroups.addToNew")}</span>
        </button>
        {#if otherGroups.length > 0}
          <button
            class="shell-picker-item shell-picker-submenu-row"
            class:open={!!groupSubmenu}
            role="menuitem"
            bind:this={moveRowEl}
            aria-haspopup="menu"
            aria-expanded={!!groupSubmenu}
            onmouseenter={openGroupSubmenu}
            onclick={() => (groupSubmenu ? (groupSubmenu = null) : openGroupSubmenu())}
          >
            <span class="tab-menu-row">
              <span class="tab-menu-star" aria-hidden="true"></span>
              <span class="shell-picker-label">{t("tabGroups.moveTo")}</span>
            </span>
            <svg viewBox="0 0 6 10" width="6" height="10" aria-hidden="true">
              <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
            </svg>
          </button>
        {/if}
        {#if tabMenuSession?.groupId}
          <button class="shell-picker-item" role="menuitem" onmouseenter={() => (groupSubmenu = null)} onclick={removeFromGroup}>
            <span class="tab-menu-star" aria-hidden="true"></span>
            <span class="shell-picker-label">{t("tabGroups.removeFrom")}</span>
          </button>
        {/if}
        <div class="shell-picker-divider"></div>
        {#each [["right", "tabs.splitRight"], ["left", "tabs.splitLeft"], ["bottom", "tabs.splitDown"]] as const as [side, label] (side)}
          <button
            class="shell-picker-item"
            role="menuitem"
            onmouseenter={() => (groupSubmenu = null)}
            onclick={() => void splitFromTabMenu(side)}
          >
            <span class="tab-menu-star" aria-hidden="true">
              <svg viewBox="0 0 12 12" width="11" height="11">
                <rect x="0.5" y="0.5" width="11" height="11" fill="none" stroke="currentColor" />
                {#if side === "bottom"}
                  <rect x="0.5" y="6" width="11" height="5.5" fill="currentColor" opacity="0.45" />
                {:else}
                  <rect x={side === "right" ? 6 : 0.5} y="0.5" width="5.5" height="11" fill="currentColor" opacity="0.45" />
                {/if}
              </svg>
            </span>
            <span class="shell-picker-label">{t(label)}</span>
          </button>
        {/each}
        <div class="shell-picker-divider"></div>
        <button class="shell-picker-item" role="menuitem" onmouseenter={() => (groupSubmenu = null)} onclick={closeFromTabMenu}>
          <span class="tab-menu-star" aria-hidden="true"></span>
          <span class="shell-picker-label">{t("tabs.close")}</span>
        </button>
      </div>
      {#if groupSubmenu}
        <div
          class="recent-submenu"
          style="top:{groupSubmenu.top}px; left:{groupSubmenu.left}px"
          role="menu"
          use:keepInWindow
          use:menuKeys={{ close: closeTabMenu, back: () => (groupSubmenu = null) }}
          in:fly={{ x: -4, duration: 120, easing: cubicOut }}
        >
          {#each otherGroups as group (group.id)}
            <button class="shell-picker-item" role="menuitem" onclick={() => moveToGroup(group.id)}>
              <span class="tab-menu-dot" style="background:{GROUP_COLORS[group.color]}" aria-hidden="true"></span>
              <span class="shell-picker-label">{group.name || t("tabGroups.unnamed")}</span>
            </button>
          {/each}
          <div class="shell-picker-divider"></div>
          <button class="shell-picker-item" role="menuitem" onclick={addToNewGroup}>
            <span class="tab-menu-dot empty" aria-hidden="true"></span>
            <span class="shell-picker-label">{t("tabGroups.newGroupEllipsis")}</span>
          </button>
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
  :global(body.termo-dragging-tab),
  :global(body.termo-dragging-tab *) {
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
  :global(body.termo-dragging-divider) .tile-chrome {
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

  /* Where a dragged tab would land (see showPreview): the whole tile, or the
     half a new group would take. Built imperatively, so global. It glides
     between shapes as the pointer crosses zones, and fades in and out. */
  :global(.tab-drop-preview) {
    position: fixed;
    z-index: 999;
    box-sizing: border-box;
    display: flex;
    /* The label sits up top, clear of the dragged tab under the pointer. */
    align-items: flex-start;
    justify-content: center;
    padding: 14px 4px;
    border: 2px dashed var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 8%, transparent);
    color: var(--tabbar-active-fg, #f0f0f0);
    font-size: 12px;
    font-weight: 600;
    pointer-events: none;
    opacity: 0;
    transform: scale(0.98);
    transition:
      left 160ms cubic-bezier(0.22, 1, 0.36, 1),
      top 160ms cubic-bezier(0.22, 1, 0.36, 1),
      width 160ms cubic-bezier(0.22, 1, 0.36, 1),
      height 160ms cubic-bezier(0.22, 1, 0.36, 1),
      background 160ms ease,
      opacity 120ms ease,
      transform 120ms ease;
  }

  :global(.tab-drop-preview.visible) {
    opacity: 1;
    transform: scale(1);
  }

  :global(.tab-drop-label:empty) {
    display: none;
  }

  :global(.tab-drop-label) {
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
  }

  /* A new group reads as solid ground, not just a highlighted tile. */
  :global(.tab-drop-preview.split) {
    border-style: solid;
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 22%, transparent);
    backdrop-filter: blur(1px);
  }

  /* The tab being dragged, left behind as a placeholder. */
  :global(.tile-tab.drag-source) {
    opacity: 0.35;
  }

  /* The copy of the tab that follows the pointer (see createGhost). Moved
     with transform, so its left/top stay at the origin. */
  :global(.tile-tab.tab-drag-ghost) {
    /* Over .in-group's relative, which is more specific. */
    position: fixed !important;
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
  }

  /* Appearance → Tabs: the tabs share the whole strip in equal parts, down
     to a width that still shows a title; past that the strip scrolls. */
  .tile-chrome.fill-width .tile-tab {
    flex: 1 1 0;
    max-width: none;
  }

  .tile-tab-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }

  .tile-tab-file {
    width: 11px;
    height: 12px;
    flex-shrink: 0;
    opacity: 0.7;
  }

  /* Unsaved changes in a file tab. */
  .tile-tab-dirty {
    flex-shrink: 0;
    font-size: 10px;
    color: var(--pane-active-border, #3a6df0);
  }

  /* The console's type, as an icon in place of its name. */
  .tile-tab-kind {
    display: flex;
    flex-shrink: 0;
  }

  /* Right-click on a tab: above the backdrop it shares with the picker. */
  .tab-menu {
    position: fixed;
    pointer-events: auto;
    z-index: 200;
    width: 220px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .tab-menu-star {
    width: 14px;
    flex-shrink: 0;
    text-align: center;
    opacity: 0.6;
  }

  .tab-menu-star svg {
    display: block;
    margin: 0 auto;
  }

  /* "Move to group" holds its icon slot and label together, apart from the
     arrow at the far end. */
  .tab-menu-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  /* A group's color, in the "Move to group" flyout. */
  .tab-menu-dot {
    width: 10px;
    height: 10px;
    margin: 0 2px;
    flex-shrink: 0;
    border-radius: 50%;
  }

  .tab-menu-dot.empty {
    border: 1px dashed currentColor;
    box-sizing: border-box;
    opacity: 0.6;
  }

  /* A tab group's chip, ahead of its tabs (see tab-groups.ts): its color,
     its name, and while folded how many tabs it hides. Clicked it folds and
     unfolds; dragged it takes the whole group along. */
  .tile-group-chip {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
    max-width: 140px;
    padding: 0 10px;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--group-color);
    color: #101114;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    user-select: none;
    touch-action: none;
    transition: filter 100ms ease;
  }

  .tile-group-chip:hover {
    filter: brightness(1.1);
  }

  .tile-group-chip:focus-visible {
    outline: 2px solid var(--tabbar-active-fg, #f0f0f0);
    outline-offset: -2px;
  }

  /* No name: just a narrow band of its color. */
  .tile-group-chip.unnamed:not(.collapsed) {
    width: 10px;
    padding: 0;
  }

  .tile-group-chip-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tile-group-chip-name:empty {
    display: none;
  }

  .tile-group-chip-count {
    padding: 0 5px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.2);
    font-size: 10px;
  }

  /* The chip that follows the pointer while a group is dragged. */
  :global(.tile-group-chip.tab-drag-ghost) {
    position: fixed;
    left: 0;
    top: 0;
    margin: 0;
    z-index: 1000;
    pointer-events: none;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.45);
  }

  :global(.tile-group-chip.drag-source) {
    opacity: 0.35;
  }

  :global(.tile-group-chip.drag-insert-before) {
    box-shadow: -3px 0 0 var(--pane-active-border, #3a6df0);
  }

  /* A dragged tab is over this group: letting go adds it (see showJoin).
     The chip lights up and the group's tabs take a tint of its color. */
  :global(.tile-group-chip.join-target) {
    filter: brightness(1.15);
    box-shadow: inset 0 0 0 2px rgba(255, 255, 255, 0.55);
  }

  :global(.tile-tab.join-target) {
    background: color-mix(in srgb, var(--group-color) 18%, transparent);
  }

  /* The dragged tab, taking the color of the group it's about to join. */
  :global(.tile-tab.tab-drag-ghost.in-group) {
    background: color-mix(in srgb, var(--group-color) 22%, var(--tabbar-active-bg, #1e1e1e));
  }

  /* A grouped tab carries its group's color along its bottom edge. */
  .tile-tab.in-group {
    position: relative;
  }

  .tile-tab.in-group::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    background: var(--group-color);
    pointer-events: none;
  }

  .tab-menu-star.on {
    color: #e5c07b;
    opacity: 1;
  }

  .tab-menu-hint {
    flex-shrink: 0;
    font-size: 10px;
    opacity: 0.5;
  }

  /* A favorite conversation is open in this tab (see favorites-store). */
  .tile-tab-favorite {
    flex-shrink: 0;
    font-size: 11px;
    line-height: 1;
    color: #e5c07b;
  }

  .favorite-star {
    color: #e5c07b;
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

  /* The two halves share one outline on hover, split by a hairline, and each
     lights up on its own so it's clear which one the click will hit. */
  .tile-tab-add-group {
    display: flex;
    align-items: stretch;
    flex-shrink: 0;
    margin: 4px 4px 4px 2px;
    border: 1px solid transparent;
    border-radius: 4px;
    transition: border-color 100ms ease;
  }

  .tile-tab-add-group:hover,
  .tile-tab-add-group.open {
    border-color: var(--tabbar-border, #3a3a3a);
  }

  .tile-tab-add,
  .tile-tab-more {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--tabbar-fg, #9a9a9a);
    cursor: pointer;
    transition:
      background 100ms ease,
      color 100ms ease;
  }

  .tile-tab-add {
    width: 22px;
    border-radius: 3px 0 0 3px;
  }

  .tile-tab-more {
    width: 15px;
    border-left: 1px solid transparent;
    border-radius: 0 3px 3px 0;
  }

  .tile-tab-add-group:hover .tile-tab-more,
  .tile-tab-add-group.open .tile-tab-more {
    border-left-color: var(--tabbar-border, #3a3a3a);
  }

  .tile-tab-add:hover,
  .tile-tab-more:hover,
  .tile-tab-add-group.open .tile-tab-more {
    color: var(--tabbar-active-fg, #f0f0f0);
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
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

  .shell-picker-item:hover:not(:disabled),
  .shell-picker-item:focus-visible {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  /* A shell's row: the shell itself, and at its end the folder icon that
     asks where to open it. The icon only shows while the row is under the
     pointer, so the menu stays as quiet as before. */
  .shell-picker-row {
    position: relative;
    display: flex;
    flex-shrink: 0;
  }

  .shell-picker-row .shell-picker-item {
    flex: 1;
    min-width: 0;
    padding-right: 34px;
  }

  .shell-picker-elsewhere {
    position: absolute;
    top: 50%;
    right: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 22px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--tabbar-active-fg, #f0f0f0);
    opacity: 0;
    transform: translateY(-50%);
    cursor: pointer;
    transition:
      opacity 100ms ease,
      background 100ms ease;
  }

  .shell-picker-elsewhere svg {
    width: 15px;
    height: 15px;
  }

  .shell-picker-row:hover .shell-picker-elsewhere,
  .shell-picker-elsewhere:focus-visible {
    opacity: 0.75;
  }

  .shell-picker-item:focus-visible {
    outline: none;
  }

  .shell-picker-elsewhere:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.12));
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
