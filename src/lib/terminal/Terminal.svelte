<script lang="ts">
  import "@xterm/xterm/css/xterm.css";
  import { onDestroy, onMount } from "svelte";
  import { TerminalInstance } from "./TerminalInstance";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import type { AgentLaunch } from "$lib/layout/split-tree";

  let {
    sessionId,
    shellPath = undefined,
    shellArgs = undefined,
    cwd = undefined,
    agent = undefined,
    active = false,
  }: {
    sessionId: string;
    shellPath?: string;
    shellArgs?: string[];
    cwd?: string;
    agent?: AgentLaunch;
    active?: boolean;
  } = $props();

  let container: HTMLDivElement;
  let instance: TerminalInstance | null = null;
  let resizeObserver: ResizeObserver | null = null;

  onMount(() => {
    // This is the SessionTab id (the tab's identity in workspace-store),
    // not the PTY id spawnPty() hands back later — Modo IA's status store
    // is keyed by the former since that's what TileChrome looks up by.
    instance = new TerminalInstance(sessionId);
    void instance.mount(container, shellPath, shellArgs, cwd, agent);

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
    // Also re-runs when something that stole focus (the command palette) asks
    // for it back, since `active` itself doesn't change in that case.
    void uiState.terminalFocusNonce;
    if (active) instance?.focus();
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    instance?.dispose();
  });
</script>

<!-- Right-click opens the Edit menu instead of the webview's own, which in a
     terminal offers nothing useful. The pane's pointerdown handler has
     already focused this console by the time the menu opens. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="terminal-container"
  bind:this={container}
  oncontextmenu={(event) => {
    event.preventDefault();
    uiState.openTerminalContextMenu(event.clientX, event.clientY);
  }}
></div>

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
