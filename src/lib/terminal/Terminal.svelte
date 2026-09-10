<script lang="ts">
  import "@xterm/xterm/css/xterm.css";
  import { onDestroy, onMount } from "svelte";
  import { TerminalInstance } from "./TerminalInstance";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  let { shellPath = undefined, shellArgs = undefined }: { shellPath?: string; shellArgs?: string[] } =
    $props();

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
