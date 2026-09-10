import {
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

class WorkspaceStore {
  // Starts empty and is populated once by initRoot() after the persisted
  // config (default shell, etc.) has loaded — see +page.svelte's onMount.
  // Rendering waits on `root !== null` so no Terminal mounts prematurely
  // with the wrong shell.
  root = $state<SplitNode | null>(null);
  focusedTileId = $state<string | null>(null);

  initRoot(shellPath?: string, shellArgs?: string[], title = "Shell"): void {
    if (this.root) return;
    const tile = makeTile(makeSession(title, shellPath, shellArgs));
    this.root = tile;
    this.focusedTileId = tile.id;
  }

  addTab(tileId: string, shellPath?: string, shellArgs?: string[], title = "Shell"): void {
    if (!this.root) return;
    const session = makeSession(title, shellPath, shellArgs);
    const updated = updateLeaf(this.root, tileId, (tile) => ({
      ...tile,
      sessions: [...tile.sessions, session],
      activeSessionId: session.id,
    }));
    if (updated) this.root = updated;
  }

  closeTab(tileId: string, sessionId: string): void {
    if (!this.root || countSessions(this.root) <= 1) return;
    const updated = updateLeaf(this.root, tileId, (tile) => {
      const remaining = tile.sessions.filter((s) => s.id !== sessionId);
      if (remaining.length === 0) return null;
      const activeSessionId =
        tile.activeSessionId === sessionId ? remaining[remaining.length - 1].id : tile.activeSessionId;
      return { ...tile, sessions: remaining, activeSessionId };
    });
    if (updated) this.root = updated;
  }

  setActiveSession(tileId: string, sessionId: string): void {
    if (!this.root) return;
    const updated = updateLeaf(this.root, tileId, (tile) => ({ ...tile, activeSessionId: sessionId }));
    if (updated) this.root = updated;
    this.focusedTileId = tileId;
  }

  splitTile(tileId: string, direction: SplitDirection): void {
    if (!this.root) return;
    const newTile = makeTile(makeSession());
    this.root = splitLeaf(this.root, tileId, direction, newTile, crypto.randomUUID());
    this.focusedTileId = newTile.id;
  }

  focusTile(tileId: string): void {
    this.focusedTileId = tileId;
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
    const afterRemoval = updateLeaf(this.root, fromTileId, (tile) => {
      moved = tile.sessions.find((s) => s.id === sessionId);
      if (!moved) return tile;
      const remaining = tile.sessions.filter((s) => s.id !== sessionId);
      if (remaining.length === 0) return null;
      const activeSessionId =
        tile.activeSessionId === sessionId ? remaining[remaining.length - 1].id : tile.activeSessionId;
      return { ...tile, sessions: remaining, activeSessionId };
    });
    if (!moved) return;

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
