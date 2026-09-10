<script lang="ts">
  import "@xterm/xterm/css/xterm.css";
  import { onDestroy, onMount } from "svelte";
  import { TerminalInstance } from "./TerminalInstance";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  let {
    shellPath = undefined,
    shellArgs = undefined,
    zoomed = false,
  }: { shellPath?: string; shellArgs?: string[]; zoomed?: boolean } = $props();

  let container: HTMLDivElement;
  let instance: TerminalInstance | null = null;
  let resizeObserver: ResizeObserver | null = null;

  onMount(() => {
    instance = new TerminalInstance();
    void instance.mount(container, shellPath, shellArgs);

    resizeObserver = new ResizeObserver(() => instance?.resize());
    resizeObserver.observe(container);
  });

  $effect(() => {
    instance?.applyTheme(themeStore.current, {
      fontSizeOverride: themeStore.fontSizeOverride,
      fontFamilyOverride: themeStore.fontFamilyOverride,
      backgroundOpacity: themeStore.backgroundOpacity,
    });
  });

  // Zooming via the button or Alt+Up doesn't click inside the terminal, so
  // without this the keyboard focus wouldn't follow — you'd have to click
  // the enlarged pane before typing. Isolated from the theme effect above
  // so it only reruns when `zoomed` itself changes.
  $effect(() => {
    if (zoomed) instance?.focus();
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    instance?.dispose();
  });
</script>

<div class="terminal-container" bind:this={container}></div>

<style>
  .terminal-container {
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--terminal-bg, transparent);
  }

  /* xterm.css hardcodes an opaque black background on its scrollable viewport
     (for macOS scrollbar rendering) — override it so background opacity/theme
     transparency actually shows through the terminal, not just the chrome. */
  .terminal-container :global(.xterm-viewport) {
    background-color: transparent !important;
  }
</style>
