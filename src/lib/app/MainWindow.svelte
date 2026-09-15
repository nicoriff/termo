<script lang="ts">
  import { onMount } from "svelte";
  import ResizeBorders from "$lib/titlebar/ResizeBorders.svelte";
  import TitleBar from "$lib/titlebar/TitleBar.svelte";
  import SplitView from "$lib/layout/SplitView.svelte";
  import TileOverviewMap from "$lib/layout/TileOverviewMap.svelte";
  import AiModePanel from "$lib/ai-mode/AiModePanel.svelte";
  import StatusBar from "$lib/statusbar/StatusBar.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { onWindowResized, setBackgroundBlur, setBadgeCount } from "$lib/core/window-client";
  import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
  import { startAgentAttention } from "$lib/ai-mode/attention";
  import { resolveDefaultShell } from "$lib/core/default-shell";
  import { eventMatchesKeybinding } from "$lib/core/shortcuts";

  onMount(() => {
    let resizeDebounce: ReturnType<typeof setTimeout>;
    let unlisten: (() => void) | null = null;
    const stopAgentAttention = startAgentAttention();

    (async () => {
      await appConfigStore.init();

      const shell = await resolveDefaultShell();
      workspaceStore.initRoot(shell.path, shell.args, shell.title);

      unlisten = await onWindowResized(() => {
        clearTimeout(resizeDebounce);
        resizeDebounce = setTimeout(() => void appConfigStore.recordWindowSize(), 400);
      });
    })();

    return () => {
      clearTimeout(resizeDebounce);
      unlisten?.();
      stopAgentAttention();
    };
  });

  // Re-runs when Preferences changes the setting (synced via config-updated).
  $effect(() => {
    if (!appConfigStore.loaded) return;
    void setBackgroundBlur(appConfigStore.backgroundBlur).catch(() => {});
  });

  // App icon badge: Modo IA sessions waiting on the user.
  $effect(() => {
    const pending =
      appConfigStore.aiModeEnabled && appConfigStore.aiDockBadge
        ? aiStatusStore.counts.awaiting + aiStatusStore.counts.error
        : 0;
    void setBadgeCount(pending > 0 ? pending : undefined).catch(() => {});
  });

  // The zoom shortcut's ↓ (Alt+↓, Cmd+↓ on macOS, or whatever it's remapped
  // to) restores a zoomed tile; with nothing zoomed it opens the tile
  // overview instead. The overview stays open on its own: arrows/mouse move
  // the selection, Enter or a click focuses that tile, and Escape, the zoom
  // shortcut's ↑ or a click outside cancels. Registered at window level in the capture phase so it
  // runs ahead of xterm's own keydown handling
  // (TerminalInstance.attachCustomKeyEventHandler), which handles the zoom
  // shortcut itself whenever this doesn't take it.
  function handleOverviewKeyDown(event: KeyboardEvent) {
    if (!workspaceStore.overviewOpen) {
      const opensOverview =
        event.key === "ArrowDown" &&
        workspaceStore.zoomedTileId === null &&
        eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("zoomTile"));
      if (!opensOverview) return;
      event.preventDefault();
      event.stopPropagation();
      workspaceStore.openOverview();
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    if (event.key === "ArrowUp" && eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("zoomTile"))) {
      workspaceStore.cancelOverview();
      return;
    }
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

</script>

<svelte:window onkeydowncapture={handleOverviewKeyDown} onblur={() => workspaceStore.cancelOverview()} />

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
    {#if workspaceStore.overviewOpen}
      <TileOverviewMap />
    {/if}
  </main>
  <StatusBar />
</div>

<style>
  .window-chrome {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    /* No background here: every region (title bar, panels, tab strips,
       consoles, status bar) paints its own translucent background exactly
       once. Stacking a second translucent layer under them made each region
       more opaque than the "Opacidad del fondo" setting says. */
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
