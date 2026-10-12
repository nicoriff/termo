<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { SIDEBAR_WIDTH, agentsStore } from "$lib/agents/agents-store.svelte";

  let resizing = $state(false);

  /** Dragging the right edge: the width follows the pointer, and is
   * remembered once it's let go. */
  function startResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startWidth = agentsStore.width;
    resizing = true;
    const move = (e: PointerEvent) => agentsStore.setWidth(startWidth + e.clientX - startX);
    const end = () => {
      resizing = false;
      agentsStore.setWidth(agentsStore.width, true);
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
    if (event.key === "ArrowLeft") agentsStore.setWidth(agentsStore.width - step, true);
    else if (event.key === "ArrowRight") agentsStore.setWidth(agentsStore.width + step, true);
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
  aria-valuemin={SIDEBAR_WIDTH.min}
  aria-valuemax={SIDEBAR_WIDTH.max}
  aria-valuenow={agentsStore.width}
  title={t("agents.resize")}
  tabindex="0"
  onpointerdown={startResize}
  ondblclick={() => agentsStore.setWidth(SIDEBAR_WIDTH.initial, true)}
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
