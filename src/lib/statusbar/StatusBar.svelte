<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { getPtyMemory } from "$lib/core/pty-client";
  import { collectTiles } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { getPtyId } from "$lib/terminal/pty-registry";

  const MEMORY_POLL_MS = 2000;

  const activeTabId = $derived.by(() => {
    if (!workspaceStore.root || !workspaceStore.focusedTileId) return null;
    const tile = collectTiles(workspaceStore.root).find((t) => t.id === workspaceStore.focusedTileId);
    return tile?.activeSessionId ?? null;
  });

  let memoryBytes = $state<number | null>(null);

  $effect(() => {
    const tabId = activeTabId;
    memoryBytes = null;
    if (!tabId) return;

    let cancelled = false;
    async function sample(tabId: string) {
      // No PTY id yet while the tab's shell is still spawning — the next
      // poll picks it up.
      const ptyId = getPtyId(tabId);
      if (!ptyId) return;
      try {
        const bytes = await getPtyMemory(ptyId);
        if (!cancelled) memoryBytes = bytes;
      } catch {
        if (!cancelled) memoryBytes = null;
      }
    }

    void sample(tabId);
    const timer = setInterval(() => void sample(tabId), MEMORY_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  });

  function formatBytes(bytes: number): string {
    const mb = bytes / (1024 * 1024);
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
  }
</script>

<div class="status-bar">
  <span class="status-item">Modo IA: {appConfigStore.aiModeEnabled ? "activado" : "desactivado"}</span>
  {#if memoryBytes !== null}
    <span class="status-item status-memory" title="Memoria de la consola activa y de los procesos que corren en ella">
      Memoria: {formatBytes(memoryBytes)}
    </span>
  {/if}
</div>

<style>
  .status-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    height: 22px;
    padding: 0 10px;
    font-size: 11px;
    background: var(--titlebar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  /* Dimmed text only — dimming the whole bar would also lighten its
     background below the opacity setting. */
  .status-item {
    white-space: nowrap;
    opacity: 0.85;
  }

  .status-memory {
    margin-left: auto;
  }
</style>
