import { dropSideAt, type SidePanelId } from "./side-panel-layout";
import { sidePanelStore, type SidePanelDrag } from "./side-panel-store.svelte";

/** How far the pointer moves before a press becomes a drag. */
const THRESHOLD_PX = 4;

/** Where a section dropped at this point would go: the column marks its
 * groups (`data-side-group`), their sections (`data-side-section`), and the
 * strip it opens in when hidden (`data-side-column-drop`). */
function targetAt(x: number, y: number): SidePanelDrag["target"] {
  const hit = document.elementFromPoint(x, y);
  if (!hit) return null;
  if (hit.closest("[data-side-column-drop]")) return { kind: "column" };
  const groupEl = hit.closest<HTMLElement>("[data-side-group]");
  if (!groupEl) return null;
  const group = Number(groupEl.dataset.sideGroup);
  const box = groupEl.getBoundingClientRect();
  const side = dropSideAt((y - box.top) / box.height);
  if (side) return { kind: "group", group, side };
  // Between the sections of its accordion: before the one under the
  // pointer, or after it past its middle.
  const sections = [...groupEl.querySelectorAll<HTMLElement>("[data-side-section]")];
  let position = sections.length;
  for (const [index, section] of sections.entries()) {
    const rect = section.getBoundingClientRect();
    if (y < rect.bottom) {
      position = y < rect.top + rect.height / 2 ? index : index + 1;
      break;
    }
  }
  return { kind: "accordion", group, position };
}

/**
 * Drags a section toward the side panel from `source` (an activity bar
 * button, or a section's header), the way a tab is dragged: pointer
 * capture, a label following the pointer, and the column showing where it
 * would land. A press that doesn't move stays a click; the click a drag
 * ends in doesn't reach `source`.
 */
export function startSectionDrag(event: PointerEvent, id: SidePanelId, label: string): void {
  if (event.button !== 0) return;
  const source = event.currentTarget as HTMLElement;
  const startX = event.clientX;
  const startY = event.clientY;
  const pointerId = event.pointerId;
  let ghost: HTMLElement | null = null;

  const move = (e: PointerEvent) => {
    if (!ghost) {
      if (Math.hypot(e.clientX - startX, e.clientY - startY) <= THRESHOLD_PX) return;
      try {
        source.setPointerCapture(pointerId);
      } catch {
        // Already released: the drag still follows the window's events.
      }
      ghost = document.createElement("div");
      ghost.className = "side-panel-drag-ghost";
      ghost.textContent = label;
      document.body.appendChild(ghost);
      document.body.classList.add("termo-dragging-section");
      sidePanelStore.drag = { id, target: null };
    }
    ghost.style.transform = `translate(${e.clientX + 12}px, ${e.clientY + 10}px)`;
    sidePanelStore.drag = { id, target: targetAt(e.clientX, e.clientY) };
  };

  const end = (e: PointerEvent) => {
    window.removeEventListener("pointermove", move, true);
    window.removeEventListener("pointerup", end, true);
    window.removeEventListener("pointercancel", end, true);
    if (!ghost) return;
    ghost.remove();
    document.body.classList.remove("termo-dragging-section");
    if (e.type === "pointerup") sidePanelStore.drop();
    else sidePanelStore.drag = null;
    swallowNextClick();
  };

  window.addEventListener("pointermove", move, true);
  window.addEventListener("pointerup", end, true);
  window.addEventListener("pointercancel", end, true);
}

/** The click that follows letting go: it's the drag's end, not a press. */
function swallowNextClick(): void {
  const swallow = (e: MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
  };
  window.addEventListener("click", swallow, { capture: true, once: true });
  setTimeout(() => window.removeEventListener("click", swallow, true), 0);
}
