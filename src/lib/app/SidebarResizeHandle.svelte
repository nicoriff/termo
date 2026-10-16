<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { SIDEBAR_WIDTH, agentsStore } from "$lib/agents/agents-store.svelte";

  interface Sizer {
    width: number;
    setWidth(width: number, persist?: boolean): void;
  }

  /** The panels on the left edge share the agents panel's width; one that
   * needs more room (the Harness panel) brings its own, with its limits. */
  let { sizer = agentsStore, limits = SIDEBAR_WIDTH }: { sizer?: Sizer; limits?: { initial: number; min: number; max: number } } =
    $props();

  let resizing = $state(false);

  /** Dragging the right edge: the width follows the pointer, and is
   * remembered once it's let go. */
  function startResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startWidth = sizer.width;
    resizing = true;
    const move = (e: PointerEvent) => sizer.setWidth(startWidth + e.clientX - startX);
    const end = () => {
      resizing = false;
      sizer.setWidth(sizer.width, true);
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  }

  function onResizeKey(event: KeyboardEvent) {
    const step = event.shiftKey ? 40 : 10;
    if (event.key === "ArrowLeft") sizer.setWidth(sizer.width - step, true);
    else if (event.key === "ArrowRight") sizer.setWidth(sizer.width + step, true);
    else return;
    event.preventDefault();
  }
</script>

<!-- The right edge of a docked side panel (agents, explorer): they share one
     width, so switching between them doesn't move the consoles. The panel it's
     in has to be positioned. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="sidebar-resize-handle"
  class:resizing
  role="separator"
  aria-orientation="vertical"
  aria-label={t("agents.resize")}
  aria-valuemin={limits.min}
  aria-valuemax={limits.max}
  aria-valuenow={sizer.width}
  title={t("agents.resize")}
  tabindex="0"
  onpointerdown={startResize}
  ondblclick={() => sizer.setWidth(limits.initial, true)}
  onkeydown={onResizeKey}
></div>

<style>
  /* A thin strip over the right border, wider than it looks so it's easy to
     grab. */
  .sidebar-resize-handle {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 25;
    width: 6px;
    cursor: ew-resize;
    outline: none;
  }

  .sidebar-resize-handle:hover,
  .sidebar-resize-handle:focus-visible,
  .sidebar-resize-handle.resizing {
    background: var(--divider-hover-bg, #3a6df0);
    opacity: 0.6;
  }

  /* While dragging, the pointer shouldn't select text or flicker back to an
     arrow over the consoles. */
  :global(body:has(.sidebar-resize-handle.resizing)) {
    cursor: ew-resize;
    user-select: none;
  }
</style>
