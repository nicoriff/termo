/**
 * Where a dropdown's panel goes, in window coordinates (it's `position:
 * fixed`, so a scrolling container can't clip it): below the trigger, or
 * above it when there isn't room below and there's more above — what a
 * native <select> does. Its height shrinks to what fits either way.
 */
export interface PanelPlacement {
  left: number;
  width: number;
  /** Set when it opens downwards: distance from the window's top. */
  top?: number;
  /** Set when it opens upwards: distance from the window's bottom. */
  bottom?: number;
  maxHeight: number;
}

/** Between the trigger and the panel. */
const GAP = 4;
/** Kept clear between the panel and the window's edge. */
const EDGE = 8;
/** Never shorter than this, even squeezed: a couple of rows stay usable. */
const MIN_HEIGHT = 64;

/**
 * `trigger`: the trigger's bounding rect. `contentHeight`: how tall the
 * panel wants to be (its rows, capped by the component's own maximum).
 */
export function placePanel(
  trigger: { left: number; top: number; bottom: number; width: number },
  viewportHeight: number,
  contentHeight: number,
): PanelPlacement {
  const below = viewportHeight - trigger.bottom - GAP - EDGE;
  const above = trigger.top - GAP - EDGE;
  const upwards = below < contentHeight && above > below;
  const room = upwards ? above : below;
  const maxHeight = Math.max(MIN_HEIGHT, Math.min(contentHeight, room));
  return upwards
    ? { left: trigger.left, width: trigger.width, bottom: viewportHeight - trigger.top + GAP, maxHeight }
    : { left: trigger.left, width: trigger.width, top: trigger.bottom + GAP, maxHeight };
}
