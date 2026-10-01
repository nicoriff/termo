<script lang="ts">
  import { onMount } from "svelte";
  import ResizeBorders from "$lib/titlebar/ResizeBorders.svelte";
  import TitleBar from "$lib/titlebar/TitleBar.svelte";
  import SplitView from "$lib/layout/SplitView.svelte";
  import TileOverviewMap from "$lib/layout/TileOverviewMap.svelte";
  import CommandPalette from "$lib/commands/CommandPalette.svelte";
  import SaveLayoutDialog from "$lib/layout/SaveLayoutDialog.svelte";
  import RestoreLayoutDialog from "$lib/layout/RestoreLayoutDialog.svelte";
  import DeleteLayoutDialog from "$lib/layout/DeleteLayoutDialog.svelte";
  import LayoutManagerDialog from "$lib/layout/LayoutManagerDialog.svelte";
  import QuitDialog from "./QuitDialog.svelte";
  import AboutDialog from "./AboutDialog.svelte";
  import UpdateDialog from "./UpdateDialog.svelte";
  import WhatsNewDialog from "./WhatsNewDialog.svelte";
  import { showWhatsNewOnce } from "./whats-new";
  import AgentLaunchDialog from "$lib/agent-launch/AgentLaunchDialog.svelte";
  import CloseFileDialog from "$lib/editor/CloseFileDialog.svelte";
  import { explorerStore } from "$lib/explorer/explorer-store.svelte";
  import AgentsSidebar from "$lib/agents/AgentsSidebar.svelte";
  import ActivityBar from "./ActivityBar.svelte";
  import AgentWizard from "$lib/agents/AgentWizard.svelte";
  import ProjectDialog from "$lib/agents/ProjectDialog.svelte";
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import { startAgentRunAttention } from "$lib/agents/agent-attention";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import { requestAgentLaunch } from "$lib/agent-launch/request-launch";
  import TerminalContextMenu from "$lib/terminal/TerminalContextMenu.svelte";
  import MarketplacePanel from "$lib/marketplace/MarketplacePanel.svelte";
  import { layoutsStore } from "$lib/layout/layouts-store.svelte";
  import AiModePanel from "$lib/ai-mode/AiModePanel.svelte";
  import StatusBar from "$lib/statusbar/StatusBar.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import {
    isFirstWindow,
    onWindowResized,
    openNewWindow,
    reportPending,
    setBackgroundBlur,
  } from "$lib/core/window-client";
  import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
  import { startAgentAttention } from "$lib/ai-mode/attention";
  import { resolveDefaultShell } from "$lib/core/default-shell";
  import { eventMatchesKeybinding } from "$lib/core/shortcuts";

  onMount(() => {
    let resizeDebounce: ReturnType<typeof setTimeout>;
    let unlisten: (() => void) | null = null;
    const stopAgentAttention = startAgentAttention();
    const stopAgentRunAttention = startAgentRunAttention();
    // So the first click on a tile's "+" finds its menu already filled.
    newTabOptions.refresh();
    void agentsStore.init();

    (async () => {
      await appConfigStore.init();

      await layoutsStore.load();
      // Launching straight into the chosen layout, rather than opening a
      // console and replacing it a moment later. A name that no longer exists
      // just falls through to the normal single console. Only the window Termo
      // opens with: one opened later starts with a single console.
      const preset =
        isFirstWindow() && appConfigStore.defaultLayout
          ? layoutsStore.layouts.find((layout) => layout.name === appConfigStore.defaultLayout)
          : undefined;
      if (preset) {
        layoutsStore.apply(preset);
      } else {
        const shell = await resolveDefaultShell();
        workspaceStore.initRoot(shell.path, shell.args, shell.title);
      }

      unlisten = await onWindowResized(() => {
        clearTimeout(resizeDebounce);
        resizeDebounce = setTimeout(() => void appConfigStore.recordWindowSize(), 400);
      });

      // After the consoles are up, so the dialog lands on a ready window. Once,
      // in the window Termo opens with.
      if (isFirstWindow()) void showWhatsNewOnce();
    })();

    return () => {
      clearTimeout(resizeDebounce);
      unlisten?.();
      stopAgentAttention();
      stopAgentRunAttention();
    };
  });

  // Re-runs when Preferences changes the setting (synced via config-updated).
  $effect(() => {
    if (!appConfigStore.loaded) return;
    void setBackgroundBlur(appConfigStore.backgroundBlur).catch(() => {});
  });

  // Modo IA is the switch for every AI feature: turned off (here or from
  // Preferences), whatever of it is open closes.
  $effect(() => {
    if (!appConfigStore.loaded || appConfigStore.aiModeEnabled) return;
    agentsStore.setOpen(false);
    uiState.closeAiSurfaces();
  });

  // App icon badge: Modo IA sessions waiting on the user.
  $effect(() => {
    const pending =
      appConfigStore.aiModeEnabled && appConfigStore.aiDockBadge
        ? aiStatusStore.counts.awaiting + aiStatusStore.counts.error
        : 0;
    void reportPending(pending).catch(() => {});
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
    // Overlays (the palette, the layout dialogs) own the keyboard while
    // they're open, and the palette can't be opened on top of the tile picker.
    if (uiState.overlayOpen) return;
    if (!workspaceStore.overviewOpen && eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("commandPalette"))) {
      event.preventDefault();
      event.stopPropagation();
      uiState.openCommandPalette();
      return;
    }
    if (!workspaceStore.overviewOpen && eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("newWindow"))) {
      event.preventDefault();
      event.stopPropagation();
      void openNewWindow();
      return;
    }
    if (
      !workspaceStore.overviewOpen &&
      eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("marketplace"))
    ) {
      event.preventDefault();
      event.stopPropagation();
      uiState.openMarketplace();
      return;
    }
    if (
      !workspaceStore.overviewOpen &&
      workspaceStore.focusedTileId &&
      eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("toggleExplorer"))
    ) {
      event.preventDefault();
      event.stopPropagation();
      explorerStore.toggle(workspaceStore.focusedTileId);
      return;
    }
    if (!workspaceStore.overviewOpen && eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("toggleAgents"))) {
      event.preventDefault();
      event.stopPropagation();
      agentsStore.toggle();
      return;
    }
    if (!workspaceStore.overviewOpen) {
      for (const [action, tool] of [
        ["newClaude", "claude"],
        ["newCodex", "codex"],
        ["newGemini", "gemini"],
        ["newCopilot", "copilot"],
      ] as const) {
        if (!eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut(action))) continue;
        event.preventDefault();
        event.stopPropagation();
        // Not installed: typing its command would only print an error.
        if (newTabOptions.agentClis?.[tool] !== false) requestAgentLaunch(tool);
        return;
      }
    }

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
    <!-- Only while it has something to show: Agents is its one button. -->
    {#if appConfigStore.aiModeEnabled && appConfigStore.agentsInActivityBar}
      <ActivityBar />
    {/if}
    {#if agentsStore.open}
      <AgentsSidebar />
    {/if}
    {#if workspaceStore.root}
      <SplitView node={workspaceStore.root} />
    {/if}
    {#if workspaceStore.overviewOpen}
      <TileOverviewMap />
    {/if}
    {#if uiState.commandPaletteOpen}
      <CommandPalette />
    {/if}
    {#if uiState.saveLayoutPromptOpen}
      <SaveLayoutDialog />
    {/if}
    {#if uiState.layoutManagerOpen}
      <LayoutManagerDialog />
    {/if}
    {#if uiState.aboutOpen}
      <AboutDialog />
    {/if}
    {#if uiState.updatesOpen}
      <UpdateDialog />
    {/if}
    {#if uiState.whatsNew}
      <WhatsNewDialog />
    {/if}
    {#if uiState.agentLaunch}
      <AgentLaunchDialog />
    {/if}
    {#if uiState.closeFileTarget}
      <CloseFileDialog />
    {/if}
    {#if uiState.agentWizard}
      <AgentWizard />
    {/if}
    {#if uiState.projectDialog}
      <ProjectDialog />
    {/if}
    {#if uiState.restoreLayoutTarget}
      <RestoreLayoutDialog />
    {/if}
    {#if uiState.deleteLayoutTarget}
      <DeleteLayoutDialog />
    {/if}
    {#if uiState.quitPrompt}
      <QuitDialog />
    {/if}
    {#if uiState.marketplaceOpen}
      <MarketplacePanel />
    {/if}
    {#if uiState.terminalContextMenu}
      <TerminalContextMenu />
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
