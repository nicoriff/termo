<script lang="ts">
  import "@xterm/xterm/css/xterm.css";
  import { onDestroy, onMount } from "svelte";
  import { TerminalInstance } from "./TerminalInstance";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import type { AgentLaunch } from "$lib/layout/split-tree";
  import { fade } from "svelte/transition";
  import { t } from "$lib/i18n/i18n.svelte";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";

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
  /** An agent's CLI is starting and hasn't drawn anything yet: the console
   * stays out of sight behind a notice until it has (see TerminalInstance). */
  let starting = $state(false);

  onMount(() => {
    // This is the SessionTab id (the tab's identity in workspace-store),
    // not the PTY id spawnPty() hands back later — Modo IA's status store
    // is keyed by the former since that's what TileChrome looks up by.
    instance = new TerminalInstance(sessionId);
    instance.onStarting = (value) => (starting = value);
    // The shell exited (`exit`, Ctrl+D): its tab goes with it. The window's
    // only tab can't close, so that one gets a fresh shell instead.
    instance.onExit = () => {
      if (!workspaceStore.closeSession(sessionId)) void instance?.restartShell(shellPath, shellArgs, cwd);
    };
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
<div class="terminal-frame">
  <div
    class="terminal-container"
    class:concealed={starting}
    bind:this={container}
    oncontextmenu={(event) => {
      event.preventDefault();
      uiState.openTerminalContextMenu(event.clientX, event.clientY);
    }}
  ></div>
  {#if starting && agent}
    <div class="starting" role="status" out:fade={{ duration: 160 }}>
      <div class="starting-notice">
        <ConsoleIcon kind={agent.tool} size={16} />
        <span>{t("agentLaunch.starting", { tool: TOOL_DISPLAY_NAMES[agent.tool] })}</span>
        <span class="starting-spinner" aria-hidden="true"></span>
      </div>
    </div>
  {/if}
</div>

<style>
  .terminal-frame {
    position: relative;
    width: 100%;
    height: 100%;
    background: var(--terminal-bg, transparent);
  }

  .terminal-container {
    width: 100%;
    height: 100%;
    overflow: hidden;
    transition: opacity 160ms ease;
  }

  /* Still laid out and running, so it's measured and fed as usual. */
  .terminal-container.concealed {
    opacity: 0;
    transition: none;
  }

  /* Lets clicks through: the console underneath still takes the focus. */
  .starting {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  /* Shown a moment late: a CLI that draws at once never flashes it. */
  .starting-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12.5px;
    opacity: 0;
    animation: starting-in 200ms ease 250ms forwards;
  }

  .starting-spinner {
    width: 11px;
    height: 11px;
    margin-left: 2px;
    border: 1.5px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    opacity: 0.6;
    animation: starting-spin 800ms linear infinite;
  }

  @keyframes starting-in {
    to {
      opacity: 0.7;
    }
  }

  @keyframes starting-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* xterm.css hardcodes an opaque black background on its scrollable viewport
     (for macOS scrollbar rendering) — override it so background opacity/theme
     transparency actually shows through the terminal, not just the chrome. */
  .terminal-container :global(.xterm-viewport) {
    background-color: transparent !important;
  }
</style>
