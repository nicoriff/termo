<script lang="ts">
  import {
    collectSessions,
    collectTiles,
    computeDividers,
    computeLayout,
    paneInsets,
    resolvePaneRect,
    type SplitNode,
  } from "./split-tree";
  import { workspaceStore } from "./workspace-store.svelte";
  import TileChrome from "./TileChrome.svelte";
  import Terminal from "$lib/terminal/Terminal.svelte";
  import FileEditor from "$lib/editor/FileEditor.svelte";
  import DiffView from "$lib/editor/DiffView.svelte";
  import GitGraphView from "$lib/git/GitGraphView.svelte";
  import AgentRunView from "$lib/agents/AgentRunView.svelte";
  import ProjectView from "$lib/agents/ProjectView.svelte";
  import OrchestrationView from "$lib/agents/OrchestrationView.svelte";
  import OrchestrationConsole from "$lib/agents/OrchestrationConsole.svelte";
  import QuickAskTab from "$lib/companion/QuickAskTab.svelte";
  import { fade } from "svelte/transition";

  let { node }: { node: SplitNode } = $props();

  const layout = $derived(computeLayout(node));
  const tiles = $derived(collectTiles(node));
  const tileById = $derived(new Map(tiles.map((t) => [t.id, t])));
  // Flat + keyed by session.id at THIS level, independent of which tile currently
  // owns it — this is what lets drag-and-drop move a session between tiles
  // without ever destroying/recreating its <Terminal> (and killing its PTY).
  const sessions = $derived(collectSessions(node));
  const dividers = $derived(computeDividers(node));

  const TABSTRIP_HEIGHT = 28;

  let containerEl: HTMLDivElement | undefined = $state();

  function startDrag(divider: (typeof dividers)[number]) {
    return (event: PointerEvent) => {
      event.preventDefault();
      const rect = containerEl?.getBoundingClientRect();
      if (!rect) return;
      // Suppresses the zoom transition (see .session-slot/.tile-chrome CSS)
      // for the duration of the drag — otherwise every pointermove tick
      // would ease toward its target instead of tracking the pointer.
      document.body.classList.add("termo-dragging-divider");

      function onMove(e: PointerEvent) {
        const ratio =
          divider.direction === "row"
            ? ((e.clientX - rect!.left) / rect!.width - divider.regionLeft) / divider.regionWidth
            : ((e.clientY - rect!.top) / rect!.height - divider.regionTop) / divider.regionHeight;
        workspaceStore.updateSplitRatio(divider.id, ratio);
      }
      function onUp() {
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
        document.body.classList.remove("termo-dragging-divider");
      }
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    };
  }
</script>

<div class="split-container" class:zoomed={workspaceStore.zoomedTileId !== null} bind:this={containerEl}>
  {#if workspaceStore.zoomedTileId}
    <div class="zoom-backdrop" transition:fade={{ duration: 140 }}></div>
  {/if}

  {#each sessions as loc (loc.session.id)}
    {@const baseRect = layout.get(loc.tileId)}
    {#if baseRect}
      {@const resolved = resolvePaneRect(loc.tileId, layout, workspaceStore.zoomedTileId) ?? {
        rect: baseRect,
        insets: paneInsets(baseRect),
      }}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="session-slot"
        class:hidden={tileById.get(loc.tileId)?.activeSessionId !== loc.session.id ||
          (workspaceStore.zoomedTileId !== null && workspaceStore.zoomedTileId !== loc.tileId)}
        class:pane-focused={workspaceStore.focusedTileId === loc.tileId}
        style="left:calc({resolved.rect.left * 100}% + {resolved.insets.left}px); top:calc({resolved.rect.top *
          100}% + {TABSTRIP_HEIGHT + resolved.insets.top}px); width:calc({resolved.rect.width * 100}% - {resolved
          .insets.left + resolved.insets.right}px); height:calc({resolved.rect.height * 100}% - {TABSTRIP_HEIGHT +
          resolved.insets.top + resolved.insets.bottom}px)"
        onpointerdown={() => workspaceStore.focusTile(loc.tileId)}
      >
        {#if loc.session.filePath}
          <FileEditor
            sessionId={loc.session.id}
            filePath={loc.session.filePath}
            active={workspaceStore.focusedTileId === loc.tileId &&
              tileById.get(loc.tileId)?.activeSessionId === loc.session.id}
          />
        {:else if loc.session.diff}
          <DiffView
            diff={loc.session.diff}
            active={workspaceStore.focusedTileId === loc.tileId &&
              tileById.get(loc.tileId)?.activeSessionId === loc.session.id}
          />
        {:else if loc.session.gitGraph}
          <GitGraphView root={loc.session.gitGraph.root} />
        {:else if loc.session.agentRun}
          <AgentRunView agentId={loc.session.agentRun.agentId} runId={loc.session.agentRun.runId} />
        {:else if loc.session.projectView}
          <ProjectView projectId={loc.session.projectView.projectId} />
        {:else if loc.session.orchestrationView?.console}
          <OrchestrationConsole orchestrationId={loc.session.orchestrationView.orchestrationId} />
        {:else if loc.session.orchestrationView}
          <OrchestrationView orchestrationId={loc.session.orchestrationView.orchestrationId} />
        {:else}
          <Terminal
            sessionId={loc.session.id}
            shellPath={loc.session.shellPath}
            shellArgs={loc.session.shellArgs}
            cwd={loc.session.cwd}
            agent={loc.session.agent}
            attach={loc.session.attach}
            attachHost={loc.session.attachHost}
            active={workspaceStore.focusedTileId === loc.tileId &&
              tileById.get(loc.tileId)?.activeSessionId === loc.session.id}
          />
          <QuickAskTab sessionId={loc.session.id} />
        {/if}
      </div>
    {/if}
  {/each}

  {#each tiles as tile (tile.id)}
    {@const resolved = resolvePaneRect(tile.id, layout, workspaceStore.zoomedTileId)}
    {#if resolved}
      <TileChrome {tile} rect={resolved.rect} insets={resolved.insets} />
    {/if}
  {/each}

  {#if !workspaceStore.zoomedTileId}
    {#each dividers as divider (divider.id)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="divider {divider.direction}"
        style={divider.direction === "row"
          ? `left:${divider.left * 100}%; top:${divider.regionTop * 100}%; height:${divider.regionHeight * 100}%`
          : `left:${divider.regionLeft * 100}%; top:${divider.top * 100}%; width:${divider.regionWidth * 100}%`}
        onpointerdown={startDrag(divider)}
      ></div>
    {/each}
  {/if}
</div>

<style>
  .split-container {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  /* While a tile is zoomed the other tiles aren't rendered, and the window
     itself has no background, so without this the area around the zoomed
     tile would show straight through to the desktop. */
  .split-container.zoomed {
    background: var(--terminal-bg, transparent);
  }

  .session-slot {
    position: absolute;
    display: flex;
    box-sizing: border-box;
    transition:
      left 140ms ease,
      top 140ms ease,
      width 140ms ease,
      height 140ms ease;
  }

  .session-slot.hidden {
    display: none;
  }

  /* Suppressed during a divider drag (startDrag toggles this body class) —
     without it, every pointermove tick would ease toward its target instead
     of tracking the pointer, making the drag feel laggy. */
  :global(body.termo-dragging-divider) .session-slot {
    transition: none;
  }

  /* No z-index here on purpose: .session-slot has none either (z-index:auto),
     so paint order between them falls back to DOM order — this element is
     placed before the sessions loop in the markup, so it always paints
     beneath the zoomed session (and beneath .tile-chrome, which does have
     an explicit z-index). Giving this an explicit z-index would instead put
     it in the "positive z-index" paint bucket, which paints AFTER (on top
     of) any z-index:auto element regardless of DOM order — covering the
     zoomed terminal. */
  .zoom-backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    pointer-events: none;
  }

  /* xterm.css reserves a permanent scrollbar track on every terminal
     viewport; with several split panes visible at once that reads as a
     wall of scrollbars. Make it invisible (transparent) on panes that
     don't have focus — wheel scrolling still works there, it just doesn't
     show. The gutter WIDTH must stay identical whether focused or not
     (fixed px below, never 0): changing it changes how many columns fit(),
     the xterm layout addon, computes, but nothing here re-runs fit()/
     resizePty() on a focus change — so a width change would silently
     desync xterm's column count from the PTY's, which full-screen TUIs
     that position output/cursor from their believed terminal size (e.g.
     claude) render as a misplaced cursor. See TerminalInstance.mount(). */
  :global(.session-slot .xterm-viewport::-webkit-scrollbar) {
    width: 10px;
  }

  :global(.session-slot .xterm-viewport::-webkit-scrollbar-track) {
    background: transparent;
  }

  :global(.session-slot:not(.pane-focused) .xterm-viewport::-webkit-scrollbar-thumb) {
    background: transparent;
  }

  .divider {
    position: absolute;
    z-index: 9;
    background: var(--divider-bg, #2a2a2a);
  }

  /* Length comes from the inline style: each divider spans only its own
     split region, not the whole container — a nested divider sized to 100%
     would cut straight through the neighboring tiles. */
  .divider.row {
    width: 4px;
    margin-left: -2px;
    cursor: col-resize;
  }

  .divider.col {
    height: 4px;
    margin-top: -2px;
    cursor: row-resize;
  }

  .divider:hover {
    background: var(--divider-hover-bg, #3a6df0);
  }
</style>
