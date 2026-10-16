<script lang="ts">
  import { onMount } from "svelte";
  import ResizeBorders from "$lib/titlebar/ResizeBorders.svelte";
  import TitleBar from "$lib/titlebar/TitleBar.svelte";
  import SplitView from "$lib/layout/SplitView.svelte";
  import TileOverviewMap from "$lib/layout/TileOverviewMap.svelte";
  import CommandPalette from "$lib/commands/CommandPalette.svelte";
  import SaveLayoutDialog from "$lib/layout/SaveLayoutDialog.svelte";
  import FavoriteDialog from "$lib/ai-mode/FavoriteDialog.svelte";
  import { favoriteDraftForFocusedTab } from "$lib/ai-mode/favorites-store.svelte";
  import FavoritesManagerDialog from "$lib/ai-mode/FavoritesManagerDialog.svelte";
  import RestoreLayoutDialog from "$lib/layout/RestoreLayoutDialog.svelte";
  import DeleteLayoutDialog from "$lib/layout/DeleteLayoutDialog.svelte";
  import LayoutManagerDialog from "$lib/layout/LayoutManagerDialog.svelte";
  import QuitDialog from "./QuitDialog.svelte";
  import AboutDialog from "./AboutDialog.svelte";
  import UpdateDialog from "./UpdateDialog.svelte";
  import WhatsNewDialog from "./WhatsNewDialog.svelte";
  import { showWhatsNewOnce } from "./whats-new";
  import MissingAgentsDialog from "$lib/agent-install/MissingAgentsDialog.svelte";
  import { missingAgents } from "$lib/agent-install/missing-agents-store.svelte";
  import OnboardingWizard from "$lib/onboarding/OnboardingWizard.svelte";
  import AgentLaunchDialog from "$lib/agent-launch/AgentLaunchDialog.svelte";
  import CloseFileDialog from "$lib/editor/CloseFileDialog.svelte";
  import { explorerStore } from "$lib/explorer/explorer-store.svelte";
  import ExplorerPanel from "$lib/explorer/ExplorerPanel.svelte";
  import { gitStore } from "$lib/git/git-store.svelte";
  import { gitReviewStore } from "$lib/git/git-review-store.svelte";
  import GitPanel from "$lib/git/GitPanel.svelte";
  import { harnessStore } from "$lib/harness/harness-store.svelte";
  import HarnessPanel from "$lib/harness/HarnessPanel.svelte";
  import AgentsSidebar from "$lib/agents/AgentsSidebar.svelte";
  import ActivityBar from "./ActivityBar.svelte";
  import AgentWizard from "$lib/agents/AgentWizard.svelte";
  import ProjectDialog from "$lib/agents/ProjectDialog.svelte";
  import OrchestrationDialog from "$lib/agents/OrchestrationDialog.svelte";
  import { orchestrationStore } from "$lib/agents/orchestration-store.svelte";
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import { startAgentRunAttention } from "$lib/agents/agent-attention";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import { requestAgentLaunch } from "$lib/agent-launch/request-launch";
  import TerminalContextMenu from "$lib/terminal/TerminalContextMenu.svelte";
  import MarketplacePanel from "$lib/marketplace/MarketplacePanel.svelte";
  import DiagnosticsPanel from "$lib/diagnostics/DiagnosticsPanel.svelte";
  import CompanionPanel from "$lib/companion/CompanionPanel.svelte";
  import { companionNudges } from "$lib/companion/companion-nudges.svelte";
  import QuickAsk from "$lib/companion/QuickAsk.svelte";
  import { quickAskStore } from "$lib/companion/quick-ask-store.svelte";
  import { layoutsStore } from "$lib/layout/layouts-store.svelte";
  import { sessionKeeper } from "$lib/layout/session-keeper.svelte";
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
  import { startNotificationClicks } from "./notification-clicks";
  import { resolveDefaultShell } from "$lib/core/default-shell";
  import { eventMatchesKeybinding } from "$lib/core/shortcuts";

  /** The quick question is there only with every switch it hangs from on. */
  const quickAskAvailable = $derived(appConfigStore.aiModeEnabled && appConfigStore.companionEnabled && appConfigStore.quickAskEnabled);

  /** Asked from a console with an agent in it, and from nowhere else. */
  function focusedOnAgent(): boolean {
    const focused = workspaceStore.focusedSessionId;
    return focused !== null && aiStatusStore.byId[focused]?.tool != null;
  }

  onMount(() => {
    let resizeDebounce: ReturnType<typeof setTimeout>;
    let unlisten: (() => void) | null = null;
    let stopSessionKeeper: (() => void) | null = null;
    const stopAgentAttention = startAgentAttention();
    const stopAgentRunAttention = startAgentRunAttention();
    const stopNotificationClicks = startNotificationClicks();
    const stopCompanionNudges = companionNudges.start();
    const stopQuickAsk = quickAskStore.start();
    const stopGit = gitStore.start();
    const stopHarness = harnessStore.start();
    const stopGitReviews = gitReviewStore.start();
    // So the first click on a tile's "+" finds its menu already filled.
    newTabOptions.refresh();
    void agentsStore.init();
    void orchestrationStore.init();

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
      // Before either: consoles this window had open that are still running
      // (see session-keeper) come back where they were.
      const restored = await sessionKeeper.restore();
      if (!restored && preset) {
        layoutsStore.apply(preset);
      } else if (!restored) {
        const shell = await resolveDefaultShell();
        workspaceStore.initRoot(shell.path, shell.args, shell.title);
      }

      stopSessionKeeper = sessionKeeper.start();

      unlisten = await onWindowResized(() => {
        clearTimeout(resizeDebounce);
        resizeDebounce = setTimeout(() => void appConfigStore.recordWindowSize(), 400);
      });

      // After the consoles are up, so the dialog lands on a ready window. Once,
      // in the window Termo opens with. No version remembered means Termo
      // never ran here: the setup opens instead, and remembers the version
      // when it closes, so quitting halfway brings it back next time.
      if (isFirstWindow()) {
        if (appConfigStore.lastSeenVersion === null) uiState.openOnboarding();
        else {
          void showWhatsNewOnce();
          // Not on the first run: the setup has its own word on what's installed.
          void missingAgents.check();
        }
      }
    })();

    return () => {
      appConfigStore.stopSystemAppearanceSync();
      clearTimeout(resizeDebounce);
      unlisten?.();
      stopSessionKeeper?.();
      stopAgentAttention();
      stopAgentRunAttention();
      stopNotificationClicks();
      stopCompanionNudges();
      stopQuickAsk();
      stopGit();
      stopHarness();
      stopGitReviews();
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
      eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("diagnostics"))
    ) {
      event.preventDefault();
      event.stopPropagation();
      uiState.toggleDiagnostics();
      return;
    }
    if (
      appConfigStore.aiModeEnabled &&
      appConfigStore.companionEnabled &&
      !workspaceStore.overviewOpen &&
      eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("companion"))
    ) {
      event.preventDefault();
      event.stopPropagation();
      uiState.toggleCompanion();
      return;
    }
    if (
      quickAskAvailable &&
      focusedOnAgent() &&
      !workspaceStore.overviewOpen &&
      eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("quickAsk"))
    ) {
      event.preventDefault();
      event.stopPropagation();
      uiState.openQuickAsk();
      return;
    }
    if (!workspaceStore.overviewOpen && eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("toggleExplorer"))) {
      event.preventDefault();
      event.stopPropagation();
      explorerStore.toggle();
      // Closed from the keyboard, which the panel had: back to the console.
      if (!explorerStore.open) uiState.refocusTerminals();
      return;
    }
    if (
      appConfigStore.gitEnabled &&
      !workspaceStore.overviewOpen &&
      eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("toggleGit"))
    ) {
      event.preventDefault();
      event.stopPropagation();
      gitStore.toggle();
      if (!gitStore.open) uiState.refocusTerminals();
      return;
    }
    if (!workspaceStore.overviewOpen && eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("tabMenu"))) {
      event.preventDefault();
      event.stopPropagation();
      uiState.openTabMenu();
      return;
    }
    if (!workspaceStore.overviewOpen && eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("toggleHarness"))) {
      event.preventDefault();
      event.stopPropagation();
      harnessStore.toggle();
      if (!harnessStore.open) uiState.refocusTerminals();
      return;
    }
    if (!workspaceStore.overviewOpen) {
      for (const [action, section] of [
        ["toggleAgents", "local"],
        ["toggleOrchestration", "orchestration"],
      ] as const) {
        if (!eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut(action))) continue;
        event.preventDefault();
        event.stopPropagation();
        agentsStore.toggleSection(section);
        return;
      }
    }
    // The focused tab's conversation, as a favorite to add or edit (Modo IA).
    if (
      !workspaceStore.overviewOpen &&
      appConfigStore.aiModeEnabled &&
      eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("favoriteTab"))
    ) {
      event.preventDefault();
      event.stopPropagation();
      const draft = favoriteDraftForFocusedTab();
      if (draft) uiState.openFavoriteDialog(draft);
      return;
    }
    if (!workspaceStore.overviewOpen) {
      for (const [action, tool] of [
        ["newClaude", "claude"],
        ["newCodex", "codex"],
        ["newGemini", "gemini"],
        ["newCopilot", "copilot"],
        ["newGrok", "grok"],
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
    <ActivityBar />
    <!-- One side panel at a time (see explorerStore). -->
    {#if agentsStore.open}
      <AgentsSidebar />
    {:else if explorerStore.open}
      <ExplorerPanel />
    {:else if gitStore.open && appConfigStore.gitEnabled}
      <GitPanel />
    {:else if harnessStore.open}
      <HarnessPanel />
    {/if}
    {#if workspaceStore.root}
      <SplitView node={workspaceStore.root} />
    {/if}
    {#if uiState.diagnosticsOpen}
      <DiagnosticsPanel />
    {/if}
    {#if uiState.companionOpen && appConfigStore.aiModeEnabled && appConfigStore.companionEnabled}
      <CompanionPanel />
    {/if}
    {#if uiState.quickAskOpen && quickAskAvailable}
      <QuickAsk />
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
    {#if uiState.favoritesManagerOpen}
      <FavoritesManagerDialog />
    {/if}
    <!-- After the manager: renaming from it opens this on top. -->
    {#if uiState.favoriteDialog}
      <FavoriteDialog />
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
    {#if uiState.onboardingOpen}
      <OnboardingWizard />
    {/if}
    {#if uiState.agentLaunch && uiState.agentLaunch.target.kind !== "folder"}
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
    {#if uiState.orchestrationDialog}
      <OrchestrationDialog />
    {/if}
    <!-- Picking a folder for one of the forms above: on top of it. -->
    {#if uiState.agentLaunch?.target.kind === "folder"}
      <AgentLaunchDialog />
    {/if}
    {#if uiState.restoreLayoutTarget}
      <RestoreLayoutDialog />
    {/if}
    {#if uiState.deleteLayoutTarget}
      <DeleteLayoutDialog />
    {/if}
    <!-- After what's new, which says what this version brought. -->
    {#if uiState.missingAgents && !uiState.whatsNew && !uiState.onboardingOpen}
      <MissingAgentsDialog />
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
