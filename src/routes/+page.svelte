<script lang="ts">
  import { onMount } from "svelte";
  import ResizeBorders from "$lib/titlebar/ResizeBorders.svelte";
  import TitleBar from "$lib/titlebar/TitleBar.svelte";
  import SplitView from "$lib/layout/SplitView.svelte";
  import SettingsPanel from "$lib/settings/SettingsPanel.svelte";
  import TileOverviewMap from "$lib/layout/TileOverviewMap.svelte";
  import AiModePanel from "$lib/ai-mode/AiModePanel.svelte";
  import StatusBar from "$lib/statusbar/StatusBar.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { onWindowResized } from "$lib/core/window-client";
  import { listShells } from "$lib/core/pty-client";
  import { resolveShellArgs } from "$lib/core/shell-args";

  onMount(() => {
    let resizeDebounce: ReturnType<typeof setTimeout>;
    let unlisten: (() => void) | null = null;

    (async () => {
      await appConfigStore.init();

      // Always resolve a concrete ShellInfo (even for "no explicit default",
      // which just means "first detected shell") rather than letting the
      // backend pick blind — resolveShellArgs needs to know the shell id to
      // decide whether to inject the PSReadLine menu-complete startup command.
      const shells = await listShells();
      const shell =
        (appConfigStore.defaultShellId && shells.find((s) => s.id === appConfigStore.defaultShellId)) || shells[0];
      const shellArgs = resolveShellArgs(shell?.id, shell?.args, appConfigStore.psreadlineMenuComplete);
      workspaceStore.initRoot(shell?.path, shellArgs, shell?.display_name);

      unlisten = await onWindowResized(() => {
        clearTimeout(resizeDebounce);
        resizeDebounce = setTimeout(() => void appConfigStore.recordWindowSize(), 400);
      });
    })();

    return () => {
      clearTimeout(resizeDebounce);
      unlisten?.();
    };
  });

  $effect(() => {
    // Track theme/font changes and persist them once the initial config load
    // has completed (avoids overwriting the saved config with defaults).
    void themeStore.current.name;
    void themeStore.fontSizeOverride;
    void themeStore.fontFamilyOverride;
    void themeStore.backgroundOpacity;
    void themeStore.outputHighlightingEnabled;
    if (appConfigStore.loaded) void appConfigStore.persist();
  });

  // Ctrl+Alt: hold to show the tile overview map (arrows/mouse to pick a
  // tile, release Ctrl or Alt to focus it — Alt-Tab style). Registered at
  // window level in the capture phase so it runs ahead of xterm's own
  // keydown handling (TerminalInstance.attachCustomKeyEventHandler), which
  // otherwise would forward these keystrokes to the shell. This is also the
  // only place in the app that reacts to keyup, since the overview needs to
  // know when either modifier is released.
  //
  // getModifierState("AltGraph") guards against AltGr (right Alt) on
  // Spanish/Latin keyboard layouts, which the OS reports as ctrlKey+altKey
  // both true — without this, typing "@", "#", etc. would pop the overview
  // open instead of inserting the character.
  function handleOverviewKeyDown(event: KeyboardEvent) {
    // Safety net: if the overview is open but Ctrl+Alt aren't BOTH still
    // held, the keyup that should have closed it (below) never arrived —
    // e.g. focus briefly left the window while a key was down, which drops
    // the keyup for whichever key was down at the time. Without this, the
    // overview would stay open forever and this handler would keep
    // swallowing every keystroke app-wide. Close it now and fall through
    // to treat this keystroke normally instead.
    if (workspaceStore.overviewOpen && !(event.ctrlKey && event.altKey)) {
      workspaceStore.confirmOverview();
    }

    const isRealCtrlAlt = event.ctrlKey && event.altKey && !event.getModifierState("AltGraph");
    if (isRealCtrlAlt && !workspaceStore.overviewOpen && !uiState.settingsOpen) {
      event.preventDefault();
      event.stopPropagation();
      workspaceStore.openOverview();
      return;
    }
    if (!workspaceStore.overviewOpen) return;

    event.preventDefault();
    event.stopPropagation();
    switch (event.key) {
      case "Escape":
        workspaceStore.cancelOverview();
        break;
      case "Enter":
        workspaceStore.confirmOverview();
        break;
      case "ArrowLeft":
        workspaceStore.moveOverviewSelection("left");
        break;
      case "ArrowRight":
        workspaceStore.moveOverviewSelection("right");
        break;
      case "ArrowUp":
        workspaceStore.moveOverviewSelection("up");
        break;
      case "ArrowDown":
        workspaceStore.moveOverviewSelection("down");
        break;
    }
  }

  function handleOverviewKeyUp(event: KeyboardEvent) {
    if (!workspaceStore.overviewOpen) return;
    if (event.key === "Control" || event.key === "Alt") {
      event.preventDefault();
      event.stopPropagation();
      workspaceStore.confirmOverview();
    }
  }
</script>

<svelte:window
  onkeydowncapture={handleOverviewKeyDown}
  onkeyupcapture={handleOverviewKeyUp}
  onblur={() => workspaceStore.cancelOverview()}
/>

<div class="window-chrome" class:maximized={uiState.maximized}>
  <ResizeBorders />
  <TitleBar />
  {#if appConfigStore.aiModeEnabled}
    <AiModePanel />
  {/if}
  <main class="content">
    {#if workspaceStore.root}
      <SplitView node={workspaceStore.root} />
    {/if}
    {#if uiState.settingsOpen}
      <SettingsPanel />
    {/if}
    {#if workspaceStore.overviewOpen}
      <TileOverviewMap />
    {/if}
  </main>
  <StatusBar />
</div>

<style>
  @font-face {
    font-family: "Ubuntu";
    font-style: normal;
    font-weight: 400;
    font-display: swap;
    src: url("/fonts/ubuntu-400.woff2") format("woff2");
  }
  @font-face {
    font-family: "Ubuntu";
    font-style: normal;
    font-weight: 500;
    font-display: swap;
    src: url("/fonts/ubuntu-500.woff2") format("woff2");
  }
  @font-face {
    font-family: "Ubuntu";
    font-style: normal;
    font-weight: 700;
    font-display: swap;
    src: url("/fonts/ubuntu-700.woff2") format("woff2");
  }

  :global(html, body) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
    font-family: "Ubuntu", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .window-chrome {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    background: var(--terminal-bg, transparent);
    border-radius: 10px;
    overflow: hidden;
  }

  .window-chrome.maximized {
    border-radius: 0;
  }

  .content {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
  }
</style>
