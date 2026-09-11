<script lang="ts">
  import "@xterm/xterm/css/xterm.css";
  import { onDestroy, onMount } from "svelte";
  import { TerminalInstance } from "./TerminalInstance";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  let {
    shellPath = undefined,
    shellArgs = undefined,
    active = false,
  }: { shellPath?: string; shellArgs?: string[]; active?: boolean } = $props();

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

  // Becoming the active tab of the focused tile never clicks inside the
  // terminal itself (switching via a tab click, Ctrl+Tab/Ctrl+Shift+Tab, or
  // zooming), so without this the keyboard focus wouldn't follow — input
  // would go nowhere until you clicked the pane. Isolated from the theme
  // effect above so it only reruns when `active` itself changes.
  $effect(() => {
    if (active) instance?.focus();
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
