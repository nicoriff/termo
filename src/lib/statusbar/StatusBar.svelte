<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
</script>

<div class="status-bar">
  <span class="status-item">Modo IA: {appConfigStore.aiModeEnabled ? "activado" : "desactivado"}</span>
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
    opacity: 0.85;
  }

  .status-item {
    white-space: nowrap;
  }
</style>
