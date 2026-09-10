<script lang="ts">
  import { startResizeDragging, type ResizeDirection } from "$lib/core/window-client";

  const EDGE = 6;
  const CORNER = 10;

  function drag(direction: ResizeDirection) {
    return (event: MouseEvent) => {
      if (event.button !== 0) return;
      void startResizeDragging(direction);
    };
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="edge n" style="height:{EDGE}px" onmousedown={drag("North")}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="edge s" style="height:{EDGE}px" onmousedown={drag("South")}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="edge e" style="width:{EDGE}px" onmousedown={drag("East")}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="edge w" style="width:{EDGE}px" onmousedown={drag("West")}></div>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="corner nw" style="width:{CORNER}px;height:{CORNER}px" onmousedown={drag("NorthWest")}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="corner ne" style="width:{CORNER}px;height:{CORNER}px" onmousedown={drag("NorthEast")}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="corner sw" style="width:{CORNER}px;height:{CORNER}px" onmousedown={drag("SouthWest")}></div>
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="corner se" style="width:{CORNER}px;height:{CORNER}px" onmousedown={drag("SouthEast")}></div>

<style>
  .edge,
  .corner {
    position: fixed;
    z-index: 1000;
  }

  .n {
    top: 0;
    left: 0;
    right: 0;
    cursor: n-resize;
  }
  .s {
    bottom: 0;
    left: 0;
    right: 0;
    cursor: s-resize;
  }
  .e {
    top: 0;
    right: 0;
    bottom: 0;
    cursor: e-resize;
  }
  .w {
    top: 0;
    left: 0;
    bottom: 0;
    cursor: w-resize;
  }

  .nw {
    top: 0;
    left: 0;
    cursor: nw-resize;
  }
  .ne {
    top: 0;
    right: 0;
    cursor: ne-resize;
  }
  .sw {
    bottom: 0;
    left: 0;
    cursor: sw-resize;
  }
  .se {
    bottom: 0;
    right: 0;
    cursor: se-resize;
  }
</style>
