<script lang="ts">
  import { collectTiles, computeLayout } from "./split-tree";
  import { workspaceStore } from "./workspace-store.svelte";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";

  const root = $derived(workspaceStore.root);
  // Always the real split layout, never the floating zoom rect — the map
  // should mirror the underlying tile arrangement even if one tile happens
  // to be zoomed right now.
  const layout = $derived(root ? computeLayout(root) : new Map());
  const tiles = $derived(root ? collectTiles(root) : []);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overview-backdrop" onpointerdown={() => workspaceStore.cancelOverview()}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="overview-map" onpointerdown={(e) => e.stopPropagation()}>
    {#each tiles as tile (tile.id)}
      {@const rect = layout.get(tile.id)}
      {#if rect}
        {@const activeSession = tile.sessions.find((s) => s.id === tile.activeSessionId)}
        <div
          class="overview-tile"
          class:selected={workspaceStore.overviewSelectedTileId === tile.id}
          class:was-focused={workspaceStore.focusedTileId === tile.id}
          style="left:{rect.left * 100}%; top:{rect.top * 100}%; width:{rect.width * 100}%; height:{rect.height *
            100}%"
          role="button"
          tabindex="0"
          onpointerenter={() => workspaceStore.setOverviewSelection(tile.id)}
          onclick={() => workspaceStore.confirmOverview()}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") workspaceStore.confirmOverview();
          }}
        >
          <span class="overview-tile-title">
            {activeSession ? tabTitleStore.titleFor(activeSession.id, activeSession.title) : "Shell"}
          </span>
          {#if tile.sessions.length > 1}
            <span class="overview-tile-badge">{tile.sessions.length}</span>
          {/if}
        </div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .overview-backdrop {
    position: absolute;
    inset: 0;
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.55);
  }

  .overview-map {
    position: relative;
    width: 80%;
    height: 80%;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }

  .overview-tile {
    position: absolute;
    box-sizing: border-box;
    margin: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    cursor: pointer;
    overflow: hidden;
    transition:
      border-color 100ms ease,
      background 100ms ease;
  }

  .overview-tile.was-focused {
    border-style: dashed;
  }

  .overview-tile.selected {
    background: rgba(58, 109, 240, 0.16);
    border: 2px solid var(--pane-active-border, #3a6df0);
    margin: 2px;
  }

  .overview-tile-title {
    max-width: calc(100% - 24px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .overview-tile-badge {
    flex-shrink: 0;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 10px;
    line-height: 16px;
    text-align: center;
  }
</style>
