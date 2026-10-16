<script lang="ts">
  import type { Snippet } from "svelte";

  /** An icon button that shows more about a row of the Harness panel in a
   * tooltip: on hover, on keyboard focus, or pinned open by a click. Placed
   * against the window rather than the panel, which would clip it. */
  let { label, children }: { label: string; children: Snippet } = $props();

  const WIDTH = 280;
  const MARGIN = 8;

  let button: HTMLButtonElement | undefined = $state();
  let tip: HTMLDivElement | undefined = $state();
  let hovered = $state(false);
  let pinned = $state(false);
  let position = $state({ left: 0, top: 0 });
  const open = $derived(hovered || pinned);
  const id = `harness-info-${Math.random().toString(36).slice(2, 9)}`;

  /** Beside the button, on the right, or below it when there's no room;
   * kept inside the window. */
  function place() {
    if (!button) return;
    const rect = button.getBoundingClientRect();
    const height = tip?.offsetHeight ?? 0;
    let left = rect.right + MARGIN;
    let top = rect.top - 6;
    if (left + WIDTH > window.innerWidth - MARGIN) {
      left = Math.max(MARGIN, rect.right - WIDTH);
      top = rect.bottom + 6;
    }
    top = Math.max(MARGIN, Math.min(top, window.innerHeight - height - MARGIN));
    position = { left, top };
  }

  $effect(() => {
    if (!open) return;
    place();
    // Its height is known once it's on screen.
    requestAnimationFrame(place);
  });

  function onWindowPointer(event: PointerEvent) {
    if (!pinned) return;
    const target = event.target as Node;
    if (button?.contains(target) || tip?.contains(target)) return;
    pinned = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && open) {
      event.stopPropagation();
      pinned = false;
      hovered = false;
    }
  }
</script>

<!-- Pinned, Escape closes it wherever the focus went since. -->
<svelte:window
  onpointerdown={onWindowPointer}
  onkeydown={(event) => pinned && event.key === "Escape" && (pinned = false)}
  onscroll={() => open && place()}
  onresize={() => open && place()}
/>

<button
  bind:this={button}
  class="info-button"
  class:active={open}
  aria-label={label}
  aria-expanded={open}
  aria-describedby={open ? id : undefined}
  onclick={() => (pinned = !pinned)}
  onpointerenter={() => (hovered = true)}
  onpointerleave={() => (hovered = false)}
  onfocus={() => (hovered = true)}
  onblur={() => (hovered = false)}
  onkeydown={onKeydown}
>
  <svg viewBox="0 0 16 16" aria-hidden="true">
    <circle cx="8" cy="8" r="6.3" fill="none" stroke="currentColor" stroke-width="1.2" />
    <path d="M8 7.2v4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
    <circle cx="8" cy="4.9" r="0.85" fill="currentColor" />
  </svg>
</button>

{#if open}
  <div
    bind:this={tip}
    {id}
    class="info-tip"
    role="tooltip"
    style="left:{position.left}px;top:{position.top}px;width:{WIDTH}px"
    onpointerenter={() => (hovered = true)}
    onpointerleave={() => (hovered = false)}
  >
    {@render children()}
  </div>
{/if}

<style>
  .info-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    opacity: 0.45;
    cursor: pointer;
  }

  .info-button svg {
    width: 13px;
    height: 13px;
  }

  .info-button:hover,
  .info-button:focus-visible,
  .info-button.active {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .info-button:focus-visible {
    outline: 1px solid var(--pane-active-border, #3a6df0);
  }

  .info-tip {
    position: fixed;
    z-index: 1000;
    box-sizing: border-box;
    padding: 9px 11px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.35);
    font-size: 11.5px;
    line-height: 1.45;
  }
</style>
