import type { PaneRect } from "./split-tree";

export type NavDirection = "left" | "right" | "up" | "down";

/**
 * Picks the tile whose rect center is the best match for moving `direction`
 * away from `currentTileId`'s center: among tiles strictly on the correct
 * side (dx/dy signed appropriately), the one minimizing primary-axis
 * distance plus a penalty for drifting off-axis. Returns null if there's no
 * candidate in that direction (caller should treat that as a no-op).
 */
export function findTileInDirection(
  layout: Map<string, PaneRect>,
  currentTileId: string,
  direction: NavDirection,
): string | null {
  const current = layout.get(currentTileId);
  if (!current) return null;
  const cx = current.left + current.width / 2;
  const cy = current.top + current.height / 2;

  const OFF_AXIS_PENALTY = 2;
  const EPS = 1e-6;

  let bestId: string | null = null;
  let bestScore = Infinity;

  for (const [tileId, rect] of layout) {
    if (tileId === currentTileId) continue;
    const tx = rect.left + rect.width / 2;
    const ty = rect.top + rect.height / 2;
    const dx = tx - cx;
    const dy = ty - cy;

    let primary: number;
    let offAxis: number;
    switch (direction) {
      case "left":
        if (dx >= -EPS) continue;
        primary = -dx;
        offAxis = Math.abs(dy);
        break;
      case "right":
        if (dx <= EPS) continue;
        primary = dx;
        offAxis = Math.abs(dy);
        break;
      case "up":
        if (dy >= -EPS) continue;
        primary = -dy;
        offAxis = Math.abs(dx);
        break;
      case "down":
        if (dy <= EPS) continue;
        primary = dy;
        offAxis = Math.abs(dx);
        break;
    }

    const score = primary + offAxis * OFF_AXIS_PENALTY;
    if (score < bestScore) {
      bestScore = score;
      bestId = tileId;
    }
  }

  return bestId;
}
