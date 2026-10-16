import { aiStatusStore } from "../ai-mode/ai-status-store.svelte";
import type { ShellLaunch } from "../core/default-shell";
import { tabCwd } from "../terminal/tab-cwd.svelte";
import {
  collectSessions,
  collectTiles,
  fileFolder,
  isConsoleTab,
  isFileTab,
  makeAgentRunSession,
  makeOrchestrationSession,
  makeProjectSession,
  makeFileSession,
  makeDiffSession,
  makeGitGraphSession,
  computeLayout,
  countSessions,
  makeSession,
  makeTile,
  splitLeaf,
  updateLeaf,
  updateRatio,
  type AgentLaunch,
  type DiffTarget,
  type DropZone,
  type SessionTab,
  type SplitDirection,
  type SplitNode,
  type TileNode,
} from "./split-tree";
import { findTileInDirection, type NavDirection } from "./spatial-nav";

class WorkspaceStore {
  // Starts empty and is populated once by initRoot() after the persisted
  // config (default shell, etc.) has loaded — see +page.svelte's onMount.
  // Rendering waits on `root !== null` so no Terminal mounts prematurely
  // with the wrong shell.
  root = $state<SplitNode | null>(null);
  focusedTileId = $state<string | null>(null);
  zoomedTileId = $state<string | null>(null);
  overviewOpen = $state(false);
  overviewSelectedTileId = $state<string | null>(null);

  initRoot(shellPath?: string, shellArgs?: string[], title = "Shell"): void {
    if (this.root) return;
    const tile = makeTile(makeSession(title, shellPath, shellArgs));
    this.root = tile;
    this.focusedTileId = tile.id;
  }

  /** Swaps the whole tree for another one — restoring a saved layout. Every
   * console currently open goes away with its tile (each <Terminal> unmounts
   * and kills its PTY), and the incoming tiles spawn their own. */
  replaceRoot(root: SplitNode): void {
    this.root = root;
    this.focusedTileId = collectTiles(root)[0]?.id ?? null;
    this.zoomedTileId = null;
    this.overviewOpen = false;
    this.overviewSelectedTileId = null;
  }

  /** Returns the new tab's id, so callers that need to talk to its console
   * (restoring an agent session types a command into it) can find it. */
  addTab(
    tileId: string,
    shellPath?: string,
    shellArgs?: string[],
    title = "Shell",
    cwd?: string,
    agent?: AgentLaunch,
  ): string | null {
    if (!this.root) return null;
    const session = { ...makeSession(title, shellPath, shellArgs, cwd), ...(agent ? { agent } : {}) };
    const updated = updateLeaf(this.root, tileId, (tile) => ({
      ...tile,
      sessions: [...tile.sessions, session],
      activeSessionId: session.id,
    }));
    if (!updated) return null;
    this.root = updated;
    this.focusedTileId = tileId;
    return session.id;
  }

  /** Shows, in a new tab of `tileId`, a console the sessions daemon kept
   * running (see SessionTab.attach). */
  attachTab(tileId: string, consoleId: string, title: string, cwd?: string): void {
    if (!this.root) return;
    const session: SessionTab = { ...makeSession(title, undefined, undefined, cwd), attach: consoleId };
    const updated = updateLeaf(this.root, tileId, (tile) => ({
      ...tile,
      sessions: [...tile.sessions, session],
      activeSessionId: session.id,
    }));
    if (!updated) return;
    this.root = updated;
    this.focusedTileId = tileId;
  }

  /** The agent a console started with has exited and, being the window's
   * only tab, the console now runs its shell (see TerminalInstance). */
  dropAgent(sessionId: string): void {
    if (!this.root) return;
    const loc = collectSessions(this.root).find((candidate) => candidate.session.id === sessionId);
    if (!loc?.session.agent) return;
    const updated = updateLeaf(this.root, loc.tileId, (tile) => ({
      ...tile,
      sessions: tile.sessions.map((session) => {
        if (session.id !== sessionId) return session;
        const { agent: _agent, ...rest } = session;
        return rest;
      }),
    }));
    if (updated) this.root = updated;
  }

  /** Ctrl+T: opens a new tab in the focused tile, cloning the shell
   * (path/args/title) of that tile's currently active tab — "same type of
   * console" the user is looking at, not whatever the app default is. */
  newTabLikeFocused(): void {
    if (this.focusedTileId) this.newTabLikeActive(this.focusedTileId);
  }

  /** The same for a given tile: what the "+" of each tile's tab strip does. */
  newTabLikeActive(tileId: string): void {
    if (!this.root) return;
    const tile = collectTiles(this.root).find((t) => t.id === tileId);
    const active = tile?.sessions.find((s) => s.id === tile.activeSessionId);
    if (!tile || !active) return;
    if (!isConsoleTab(active)) {
      // A file or an agent's runs can't be "cloned": the tile's last console
      // is what's copied, opened next to the file — or the default shell, if
      // it has none.
      const console = [...tile.sessions].reverse().find(isConsoleTab);
      this.addTab(tile.id, console?.shellPath, console?.shellArgs, console?.title, sessionFolder(active));
      return;
    }
    // Where that console is now (as it reported via OSC 7), not just where it
    // was opened — a new tab should land in the directory you were working in.
    this.addTab(tile.id, active.shellPath, active.shellArgs, active.title, sessionFolder(active));
  }

  /** Shows one of an agent's runs in a tab. Each agent has at most one such
   * tab: when it's already open anywhere it switches to `runId` and comes to
   * the front; otherwise a new one opens in the focused tile. */
  openAgentRun(agentId: string, runId: string, title: string): void {
    if (!this.root) return;
    const open = collectSessions(this.root).find((loc) => loc.session.agentRun?.agentId === agentId);
    if (open) {
      const updated = updateLeaf(this.root, open.tileId, (tile) => ({
        ...tile,
        sessions: tile.sessions.map((session) =>
          session.id === open.session.id ? { ...session, title, agentRun: { agentId, runId } } : session,
        ),
        activeSessionId: open.session.id,
      }));
      if (updated) this.root = updated;
      this.focusedTileId = open.tileId;
      return;
    }
    const tileId = this.focusedTileId ?? collectTiles(this.root)[0]?.id;
    if (!tileId) return;
    const session = makeAgentRunSession(title, agentId, runId);
    const updated = updateLeaf(this.root, tileId, (tile) => ({
      ...tile,
      sessions: [...tile.sessions, session],
      activeSessionId: session.id,
    }));
    if (!updated) return;
    this.root = updated;
    this.focusedTileId = tileId;
  }

  /** Shows a project in a tab: the one it already has, brought forward, or
   * a new one in the focused tile. */
  openProject(projectId: string, title: string): void {
    if (!this.root) return;
    const open = collectSessions(this.root).find((loc) => loc.session.projectView?.projectId === projectId);
    if (open) {
      const updated = updateLeaf(this.root, open.tileId, (tile) => ({
        ...tile,
        sessions: tile.sessions.map((session) => (session.id === open.session.id ? { ...session, title } : session)),
        activeSessionId: open.session.id,
      }));
      if (updated) this.root = updated;
      this.focusedTileId = open.tileId;
      return;
    }
    const tileId = this.focusedTileId ?? collectTiles(this.root)[0]?.id;
    if (!tileId) return;
    const session = makeProjectSession(title, projectId);
    const updated = updateLeaf(this.root, tileId, (tile) => ({
      ...tile,
      sessions: [...tile.sessions, session],
      activeSessionId: session.id,
    }));
    if (!updated) return;
    this.root = updated;
    this.focusedTileId = tileId;
  }

  /** Shows an orchestration in a tab: the one it already has, brought
   * forward, or a new one in the focused tile. */
  openOrchestration(orchestrationId: string, title: string, console = false): void {
    if (!this.root) return;
    const open = collectSessions(this.root).find(
      (loc) =>
        loc.session.orchestrationView?.orchestrationId === orchestrationId &&
        (loc.session.orchestrationView.console === true) === console,
    );
    if (open) {
      const updated = updateLeaf(this.root, open.tileId, (tile) => ({
        ...tile,
        sessions: tile.sessions.map((session) => (session.id === open.session.id ? { ...session, title } : session)),
        activeSessionId: open.session.id,
      }));
      if (updated) this.root = updated;
      this.focusedTileId = open.tileId;
      return;
    }
    const tileId = this.focusedTileId ?? collectTiles(this.root)[0]?.id;
    if (!tileId) return;
    const session = makeOrchestrationSession(title, orchestrationId, console);
    const updated = updateLeaf(this.root, tileId, (tile) => ({
      ...tile,
      sessions: [...tile.sessions, session],
      activeSessionId: session.id,
    }));
    if (!updated) return;
    this.root = updated;
    this.focusedTileId = tileId;
  }

  /** Opens `filePath` in an editor tab in `tileId` — or, when it's already
   * open anywhere, brings that tab forward instead of opening it twice. */
  openFile(tileId: string, filePath: string): void {
    if (!this.root) return;
    const open = collectSessions(this.root).find((loc) => loc.session.filePath === filePath);
    if (open) {
      this.setActiveSession(open.tileId, open.session.id);
      return;
    }
    const session = makeFileSession(filePath);
    const updated = updateLeaf(this.root, tileId, (tile) => ({
      ...tile,
      sessions: [...tile.sessions, session],
      activeSessionId: session.id,
    }));
    if (!updated) return;
    this.root = updated;
    this.focusedTileId = tileId;
  }

  /** Shows what changed in a file in a tab: the one that already shows
   * that same pair of versions, brought forward, or a new one in the
   * focused tile. */
  openDiff(diff: DiffTarget): void {
    this.openOwnTab(
      (session) =>
        session.diff?.root === diff.root &&
        session.diff.path === diff.path &&
        session.diff.from === diff.from &&
        session.diff.to === diff.to,
      () => makeDiffSession(diff),
    );
  }

  /** Shows a repository's history in a tab; each repository has one. */
  openGitGraph(root: string, title: string): void {
    this.openOwnTab(
      (session) => session.gitGraph?.root === root,
      () => makeGitGraphSession(title, root),
    );
  }

  /** Brings forward the tab `matches` finds, wherever it is, or opens the
   * one `make` builds in the focused tile. */
  private openOwnTab(matches: (session: SessionTab) => boolean, make: () => SessionTab): void {
    if (!this.root) return;
    const open = collectSessions(this.root).find((loc) => matches(loc.session));
    if (open) {
      this.setActiveSession(open.tileId, open.session.id);
      return;
    }
    const tileId = this.focusedTileId ?? collectTiles(this.root)[0]?.id;
    if (!tileId) return;
    const session = make();
    const updated = updateLeaf(this.root, tileId, (tile) => ({
      ...tile,
      sessions: [...tile.sessions, session],
      activeSessionId: session.id,
    }));
    if (!updated) return;
    this.root = updated;
    this.focusedTileId = tileId;
  }

  /** The tab in focus: the active one of the focused tile. */
  get focusedSession(): SessionTab | null {
    if (!this.root || !this.focusedTileId) return null;
    const tile = collectTiles(this.root).find((candidate) => candidate.id === this.focusedTileId);
    return tile?.sessions.find((session) => session.id === tile.activeSessionId) ?? null;
  }

  closeTab(tileId: string, sessionId: string): void {
    if (!this.root || countSessions(this.root) <= 1) return;
    let tileVanishes = false;
    const updated = updateLeaf(this.root, tileId, (tile) => {
      const remaining = tile.sessions.filter((s) => s.id !== sessionId);
      if (remaining.length === 0) {
        tileVanishes = true;
        return null;
      }
      const activeSessionId =
        tile.activeSessionId === sessionId ? remaining[remaining.length - 1].id : tile.activeSessionId;
      return { ...tile, sessions: remaining, activeSessionId };
    });
    if (updated) {
      this.root = updated;
      if (tileVanishes && this.zoomedTileId === tileId) this.zoomedTileId = null;
      aiStatusStore.remove(sessionId);
    }
  }

  /** Closes a tab wherever it lives — its console's process has ended.
   * False when it's the only tab left, which stays. */
  closeSession(sessionId: string): boolean {
    if (!this.root || countSessions(this.root) <= 1) return false;
    const loc = collectSessions(this.root).find((candidate) => candidate.session.id === sessionId);
    if (!loc) return false;
    this.closeTab(loc.tileId, sessionId);
    return true;
  }

  setActiveSession(tileId: string, sessionId: string): void {
    if (!this.root) return;
    const updated = updateLeaf(this.root, tileId, (tile) => ({ ...tile, activeSessionId: sessionId }));
    if (updated) this.root = updated;
    this.focusedTileId = tileId;
  }

  /** Activates a tab wherever it lives and brings its tile forward — zoomed
   * when it shares the window with other tiles (a lone tile is already in
   * front, and zooming it would only shrink it). */
  focusSession(sessionId: string): void {
    if (!this.root) return;
    const tiles = collectTiles(this.root);
    const tile = tiles.find((t) => t.sessions.some((s) => s.id === sessionId));
    if (!tile) return;
    this.setActiveSession(tile.id, sessionId);
    this.zoomedTileId = tiles.length > 1 ? tile.id : null;
  }

  /** The active tab of the focused tile. */
  get focusedSessionId(): string | null {
    if (!this.root || !this.focusedTileId) return null;
    return collectTiles(this.root).find((t) => t.id === this.focusedTileId)?.activeSessionId ?? null;
  }

  /** Whether a tab is currently on screen: its tile's active tab, and not
   * hidden behind another tile's zoom. */
  isSessionVisible(sessionId: string): boolean {
    if (!this.root) return false;
    const tile = collectTiles(this.root).find((t) => t.activeSessionId === sessionId);
    return !!tile && (this.zoomedTileId === null || this.zoomedTileId === tile.id);
  }

  /** Ctrl+Tab / Ctrl+Shift+Tab: cycle the active tab within the focused
   * tile, wrapping around at either end. No-op with 0-1 tabs or no focus. */
  cycleTab(direction: 1 | -1): void {
    if (!this.root || !this.focusedTileId) return;
    const tile = collectTiles(this.root).find((t) => t.id === this.focusedTileId);
    if (!tile || tile.sessions.length < 2) return;
    const currentIndex = tile.sessions.findIndex((s) => s.id === tile.activeSessionId);
    if (currentIndex === -1) return;
    const nextIndex = (currentIndex + direction + tile.sessions.length) % tile.sessions.length;
    this.setActiveSession(tile.id, tile.sessions[nextIndex].id);
  }

  splitTile(tileId: string, direction: SplitDirection, shell: ShellLaunch): void {
    if (!this.root) return;
    const newTile = makeTile(makeSession(shell.title, shell.path, shell.args));
    this.root = splitLeaf(this.root, tileId, direction, newTile, crypto.randomUUID());
    this.focusedTileId = newTile.id;
  }

  focusTile(tileId: string): void {
    this.focusedTileId = tileId;
  }

  toggleZoom(tileId: string): void {
    if (this.zoomedTileId === tileId) {
      this.zoomedTileId = null;
    } else {
      this.zoomedTileId = tileId;
      this.focusedTileId = tileId;
    }
  }

  /** Alt+Up: bring the focused tile to the foreground. No-op if nothing is focused. */
  zoomFocused(): void {
    if (this.focusedTileId) this.zoomedTileId = this.focusedTileId;
  }

  /** Alt+Down: always restores, regardless of what's currently zoomed. */
  unzoom(): void {
    this.zoomedTileId = null;
  }

  /** Zoom shortcut's ↓ with nothing zoomed: shows the tile overview map,
   * preselecting whatever tile currently has focus. No-op with no tiles or
   * if already open. */
  openOverview(): void {
    if (!this.root || this.overviewOpen) return;
    const tiles = collectTiles(this.root);
    this.overviewSelectedTileId = this.focusedTileId ?? tiles[0]?.id ?? null;
    this.overviewOpen = true;
  }

  /** Arrow keys / hover while the overview is open: moves the selection
   * toward the nearest tile in that direction, using the tree's real split
   * layout (never the floating zoom rect) so the map always matches the
   * on-screen tile arrangement. No-op if there's no tile that way. */
  moveOverviewSelection(direction: NavDirection): void {
    if (!this.root || !this.overviewSelectedTileId) return;
    const next = findTileInDirection(computeLayout(this.root), this.overviewSelectedTileId, direction);
    if (next) this.overviewSelectedTileId = next;
  }

  setOverviewSelection(tileId: string): void {
    this.overviewSelectedTileId = tileId;
  }

  /** Enter or click: commits the selection as focus and closes the overview. */
  confirmOverview(): void {
    if (this.overviewSelectedTileId) this.focusTile(this.overviewSelectedTileId);
    this.overviewOpen = false;
  }

  /** Escape, blur, or click outside a tile: closes without changing focus. */
  cancelOverview(): void {
    this.overviewOpen = false;
  }

  updateSplitRatio(splitId: string, ratio: number): void {
    if (!this.root) return;
    this.root = updateRatio(this.root, splitId, ratio);
  }

  /**
   * Moves a live session (and its PTY) from one tile to another — or
   * reorders it within the same tile — without killing it. `beforeSessionId`
   * inserts just ahead of that session in the destination tile; omitted (or
   * not found) it's appended at the end.
   */
  moveSession(fromTileId: string, toTileId: string, sessionId: string, beforeSessionId?: string): void {
    if (!this.root) return;

    if (fromTileId === toTileId) {
      const updated = updateLeaf(this.root, toTileId, (tile) => {
        const moved = tile.sessions.find((s) => s.id === sessionId);
        if (!moved) return tile;
        const without = tile.sessions.filter((s) => s.id !== sessionId);
        const insertAt = beforeSessionId ? without.findIndex((s) => s.id === beforeSessionId) : -1;
        const sessions =
          insertAt === -1
            ? [...without, moved]
            : [...without.slice(0, insertAt), moved, ...without.slice(insertAt)];
        return { ...tile, sessions, activeSessionId: sessionId };
      });
      if (updated) this.root = updated;
      return;
    }

    const detached = this.detachSession(fromTileId, sessionId);
    if (!detached) return;
    const { base, moved } = detached;
    const afterAdd = updateLeaf(base, toTileId, (tile) => {
      const insertAt = beforeSessionId ? tile.sessions.findIndex((s) => s.id === beforeSessionId) : -1;
      const sessions =
        insertAt === -1
          ? [...tile.sessions, moved!]
          : [...tile.sessions.slice(0, insertAt), moved!, ...tile.sessions.slice(insertAt)];
      return { ...tile, sessions, activeSessionId: moved!.id };
    });
    if (afterAdd) {
      this.root = afterAdd;
      this.focusedTileId = toTileId;
    }
  }

  /**
   * Drops a tab on the side of a tile: that tile splits in two and the tab
   * becomes the whole of the new half, left/right or above/below it. A tile
   * can't be split by its own only tab — there'd be nothing left beside it.
   */
  splitWithSession(fromTileId: string, targetTileId: string, sessionId: string, side: Exclude<DropZone, "center">): void {
    if (!this.root) return;
    if (fromTileId === targetTileId) {
      const tile = collectTiles(this.root).find((t) => t.id === fromTileId);
      if (!tile || tile.sessions.length < 2) return;
    }
    const detached = this.detachSession(fromTileId, sessionId);
    if (!detached) return;
    const newTile = makeTile(detached.moved);
    const direction: SplitDirection = side === "left" || side === "right" ? "row" : "col";
    this.root = splitLeaf(detached.base, targetTileId, direction, newTile, crypto.randomUUID(), side === "left" || side === "top");
    this.focusedTileId = newTile.id;
  }

  /** Takes a tab out of its tile, dropping the tile if that was its last
   * tab. Returns the tree without it, or null when there's no such tab. */
  private detachSession(fromTileId: string, sessionId: string): { base: SplitNode; moved: SessionTab } | null {
    if (!this.root) return null;
    let moved: SessionTab | undefined;
    let tileVanishes = false;
    const afterRemoval = updateLeaf(this.root, fromTileId, (tile) => {
      moved = tile.sessions.find((s) => s.id === sessionId);
      if (!moved) return tile;
      const remaining = tile.sessions.filter((s) => s.id !== sessionId);
      if (remaining.length === 0) {
        tileVanishes = true;
        return null;
      }
      const activeSessionId =
        tile.activeSessionId === sessionId ? remaining[remaining.length - 1].id : tile.activeSessionId;
      return { ...tile, sessions: remaining, activeSessionId };
    });
    if (!moved) return null;
    if (tileVanishes && this.zoomedTileId === fromTileId) this.zoomedTileId = null;
    return { base: afterRemoval ?? this.root, moved };
  }
}

/** The folder a tab is "in": where its console is now (as the shell last
 * reported, else where it opened), or the folder of the file it shows. */
export function sessionFolder(session: SessionTab): string | undefined {
  if (isFileTab(session)) return fileFolder(session.filePath);
  // A diff or a history is of a repository: whatever follows the tab in
  // focus (the explorer, the Git panel) stays on it.
  if (session.diff) return session.diff.root;
  if (session.gitGraph) return session.gitGraph.root;
  if (session.agentRun || session.projectView || session.orchestrationView) return undefined;
  return tabCwd.byTab[session.id] ?? session.cwd;
}

export const workspaceStore = new WorkspaceStore();
export type { TileNode };
