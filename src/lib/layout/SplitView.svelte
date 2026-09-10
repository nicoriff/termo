<script lang="ts">
  import {
    collectSessions,
    collectTiles,
    computeDividers,
    computeLayout,
    paneInsets,
    type SplitNode,
  } from "./split-tree";
  import { workspaceStore } from "./workspace-store.svelte";
  import TileChrome from "./TileChrome.svelte";
  import Terminal from "$lib/terminal/Terminal.svelte";

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
      }
      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    };
  }
</script>

<div class="split-container" bind:this={containerEl}>
  {#each sessions as loc (loc.session.id)}
    {@const rect = layout.get(loc.tileId)}
    {#if rect}
      {@const insets = paneInsets(rect)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="session-slot"
        class:hidden={tileById.get(loc.tileId)?.activeSessionId !== loc.session.id}
        class:pane-focused={workspaceStore.focusedTileId === loc.tileId}
        style="left:calc({rect.left * 100}% + {insets.left}px); top:calc({rect.top * 100}% + {TABSTRIP_HEIGHT +
          insets.top}px); width:calc({rect.width * 100}% - {insets.left + insets.right}px); height:calc({rect.height *
          100}% - {TABSTRIP_HEIGHT + insets.top + insets.bottom}px)"
        onpointerdown={() => workspaceStore.focusTile(loc.tileId)}
      >
        <Terminal shellPath={loc.session.shellPath} shellArgs={loc.session.shellArgs} />
      </div>
    {/if}
  {/each}

  {#each tiles as tile (tile.id)}
    {@const rect = layout.get(tile.id)}
    {#if rect}
      <TileChrome {tile} {rect} />
    {/if}
  {/each}

  {#each dividers as divider (divider.id)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="divider {divider.direction}"
      style="left:{divider.left * 100}%; top:{divider.top * 100}%"
      onpointerdown={startDrag(divider)}
    ></div>
  {/each}
</div>

<style>
  .split-container {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  .session-slot {
    position: absolute;
    display: flex;
    box-sizing: border-box;
  }

  .session-slot.hidden {
    display: none;
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

  .divider.row {
    top: 0;
    height: 100%;
    width: 4px;
    margin-left: -2px;
    cursor: col-resize;
  }

  .divider.col {
    left: 0;
    width: 100%;
    height: 4px;
    margin-top: -2px;
    cursor: row-resize;
  }

  .divider:hover {
    background: var(--divider-hover-bg, #3a6df0);
  }
</style>
