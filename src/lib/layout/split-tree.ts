export type SplitDirection = "row" | "col";

export interface SessionTab {
  id: string;
  title: string;
  shellPath?: string;
  shellArgs?: string[];
  /** Directory the console starts in. Only set when something asked for it
   * (restoring an agent session); otherwise the shell's own default wins. */
  cwd?: string;
}

export interface TileNode {
  kind: "leaf";
  id: string;
  sessions: SessionTab[];
  activeSessionId: string;
}

interface SplitBranch {
  kind: "split";
  id: string;
  direction: SplitDirection;
  ratio: number;
  children: [SplitNode, SplitNode];
}

export type SplitNode = TileNode | SplitBranch;

export function makeSession(title = "Shell", shellPath?: string, shellArgs?: string[], cwd?: string): SessionTab {
  return { id: crypto.randomUUID(), title, shellPath, shellArgs, cwd };
}

export function makeTile(session: SessionTab): TileNode {
  return { kind: "leaf", id: crypto.randomUUID(), sessions: [session], activeSessionId: session.id };
}

/**
 * Replaces the leaf matching `tileId` with `updater(tile)`. If `updater` returns
 * null the leaf is removed and the tree collapses that leaf's parent split into
 * the surviving sibling — the same "close last tab in this tile" rule used
 * throughout the store. Returns null only if `root` itself was the target leaf
 * and got removed (i.e. the whole tree collapsed away).
 */
export function updateLeaf(
  root: SplitNode,
  tileId: string,
  updater: (tile: TileNode) => TileNode | null,
): SplitNode | null {
  if (root.kind === "leaf") {
    return root.id === tileId ? updater(root) : root;
  }
  const a = updateLeaf(root.children[0], tileId, updater);
  const b = updateLeaf(root.children[1], tileId, updater);
  if (a === null && b === null) return null;
  if (a === null) return b;
  if (b === null) return a;
  return { ...root, children: [a, b] };
}

export function splitLeaf(
  root: SplitNode,
  targetTileId: string,
  direction: SplitDirection,
  newTile: TileNode,
  newSplitId: string,
): SplitNode {
  if (root.kind === "leaf") {
    if (root.id !== targetTileId) return root;
    return { kind: "split", id: newSplitId, direction, ratio: 0.5, children: [root, newTile] };
  }
  return {
    ...root,
    children: [
      splitLeaf(root.children[0], targetTileId, direction, newTile, newSplitId),
      splitLeaf(root.children[1], targetTileId, direction, newTile, newSplitId),
    ],
  };
}

export function updateRatio(root: SplitNode, splitId: string, ratio: number): SplitNode {
  if (root.kind === "leaf") return root;
  const clamped = Math.min(0.85, Math.max(0.15, ratio));
  if (root.id === splitId) return { ...root, ratio: clamped };
  return {
    ...root,
    children: [updateRatio(root.children[0], splitId, ratio), updateRatio(root.children[1], splitId, ratio)],
  };
}

export function countSessions(root: SplitNode): number {
  return root.kind === "leaf"
    ? root.sessions.length
    : countSessions(root.children[0]) + countSessions(root.children[1]);
}

export function collectTiles(root: SplitNode): TileNode[] {
  return root.kind === "leaf" ? [root] : [...collectTiles(root.children[0]), ...collectTiles(root.children[1])];
}

export interface SessionLocation {
  session: SessionTab;
  tileId: string;
}

/** Flat, order-stable list of every session across every tile — keyed by
 * session.id at the render layer so a <Terminal> never remounts (and never
 * kills its PTY) when its owning tile changes, e.g. via drag-and-drop. */
export function collectSessions(root: SplitNode): SessionLocation[] {
  if (root.kind === "leaf") return root.sessions.map((session) => ({ session, tileId: root.id }));
  return [...collectSessions(root.children[0]), ...collectSessions(root.children[1])];
}

export interface PaneRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Half the divider's visual width (it's 4px total, centered on the split
 * boundary) — panes are inset by this on any edge that sits on an internal
 * boundary so the divider fills that gap exactly instead of overlapping
 * pane content that's rendered flush to the boundary line. */
export const DIVIDER_GAP = 2;

export interface PaneInsets {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Per-edge inset for `rect`: DIVIDER_GAP on edges that sit on an internal
 * split boundary, 0 on edges that sit on the outer window edge. Any rect
 * produced by computeLayout has left/top of exactly 0 (or width/height
 * summing to exactly 1) only at the outer edge, since internal boundaries
 * always come from a strictly-between-0-and-1 ratio split. */
export function paneInsets(rect: PaneRect): PaneInsets {
  const EPS = 1e-6;
  return {
    left: rect.left > EPS ? DIVIDER_GAP : 0,
    top: rect.top > EPS ? DIVIDER_GAP : 0,
    right: rect.left + rect.width < 1 - EPS ? DIVIDER_GAP : 0,
    bottom: rect.top + rect.height < 1 - EPS ? DIVIDER_GAP : 0,
  };
}

/** Fraction of each edge left as margin around a zoomed tile's floating
 * overlay — the zoomed pane occupies the central 90% of the console area
 * (1 - 2*ZOOM_MARGIN on each axis). */
export const ZOOM_MARGIN = 0.05;

const ZOOM_RECT: PaneRect = {
  left: ZOOM_MARGIN,
  top: ZOOM_MARGIN,
  width: 1 - 2 * ZOOM_MARGIN,
  height: 1 - 2 * ZOOM_MARGIN,
};

const ZERO_INSETS: PaneInsets = { left: 0, top: 0, right: 0, bottom: 0 };

/**
 * Resolves the rect+insets a tile should render at, given zoom state:
 * - If another tile is zoomed, this tile must not render at all -> null.
 * - If THIS tile is the zoomed one, it floats at 90% of the console area
 *   with zero insets (a floating overlay touching no divider — paneInsets's
 *   edge-adjacency heuristic doesn't apply and would shrink it for no reason).
 * - Otherwise (no zoom active), the tile's normal layout rect, inset as usual.
 */
export function resolvePaneRect(
  tileId: string,
  layout: Map<string, PaneRect>,
  zoomedTileId: string | null,
): { rect: PaneRect; insets: PaneInsets } | null {
  if (zoomedTileId !== null) {
    if (zoomedTileId !== tileId) return null;
    return { rect: ZOOM_RECT, insets: ZERO_INSETS };
  }
  const rect = layout.get(tileId);
  return rect ? { rect, insets: paneInsets(rect) } : null;
}

/** Normalized (0..1) rectangle per tile, computed from the split tree's ratios. */
export function computeLayout(root: SplitNode): Map<string, PaneRect> {
  const result = new Map<string, PaneRect>();

  function walk(node: SplitNode, rect: PaneRect) {
    if (node.kind === "leaf") {
      result.set(node.id, rect);
      return;
    }
    if (node.direction === "row") {
      const w0 = rect.width * node.ratio;
      walk(node.children[0], { ...rect, width: w0 });
      walk(node.children[1], { ...rect, left: rect.left + w0, width: rect.width - w0 });
    } else {
      const h0 = rect.height * node.ratio;
      walk(node.children[0], { ...rect, height: h0 });
      walk(node.children[1], { ...rect, top: rect.top + h0, height: rect.height - h0 });
    }
  }

  walk(root, { left: 0, top: 0, width: 1, height: 1 });
  return result;
}

export interface DividerInfo {
  id: string;
  direction: SplitDirection;
  /** Divider line position, normalized (0..1) to the whole container. */
  left: number;
  top: number;
  /** The split region this divider adjusts — needed to convert a drag delta back into a ratio. */
  regionLeft: number;
  regionTop: number;
  regionWidth: number;
  regionHeight: number;
}

export function computeDividers(root: SplitNode): DividerInfo[] {
  const result: DividerInfo[] = [];

  function walk(node: SplitNode, rect: PaneRect) {
    if (node.kind === "leaf") return;
    if (node.direction === "row") {
      const w0 = rect.width * node.ratio;
      result.push({
        id: node.id,
        direction: "row",
        left: rect.left + w0,
        top: rect.top,
        regionLeft: rect.left,
        regionTop: rect.top,
        regionWidth: rect.width,
        regionHeight: rect.height,
      });
      walk(node.children[0], { ...rect, width: w0 });
      walk(node.children[1], { ...rect, left: rect.left + w0, width: rect.width - w0 });
    } else {
      const h0 = rect.height * node.ratio;
      result.push({
        id: node.id,
        direction: "col",
        left: rect.left,
        top: rect.top + h0,
        regionLeft: rect.left,
        regionTop: rect.top,
        regionWidth: rect.width,
        regionHeight: rect.height,
      });
      walk(node.children[0], { ...rect, height: h0 });
      walk(node.children[1], { ...rect, top: rect.top + h0, height: rect.height - h0 });
    }
  }

  walk(root, { left: 0, top: 0, width: 1, height: 1 });
  return result;
}
