import { aiStatusStore } from "../ai-mode/ai-status-store.svelte";
import type { ShellLaunch } from "../core/default-shell";
import {
  collectTiles,
  computeLayout,
  countSessions,
  makeSession,
  makeTile,
  splitLeaf,
  updateLeaf,
  updateRatio,
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
  addTab(tileId: string, shellPath?: string, shellArgs?: string[], title = "Shell", cwd?: string): string | null {
    if (!this.root) return null;
    const session = makeSession(title, shellPath, shellArgs, cwd);
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

  /** Ctrl+T: opens a new tab in the focused tile, cloning the shell
   * (path/args/title) of that tile's currently active tab — "same type of
   * console" the user is looking at, not whatever the app default is. */
  newTabLikeFocused(): void {
    if (!this.root || !this.focusedTileId) return;
    const tile = collectTiles(this.root).find((t) => t.id === this.focusedTileId);
    const active = tile?.sessions.find((s) => s.id === tile.activeSessionId);
    if (!tile || !active) return;
    this.addTab(tile.id, active.shellPath, active.shellArgs, active.title);
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
    if (!moved) return;
    if (tileVanishes && this.zoomedTileId === fromTileId) this.zoomedTileId = null;

    const base = afterRemoval ?? this.root;
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
}

export const workspaceStore = new WorkspaceStore();
export type { TileNode };
