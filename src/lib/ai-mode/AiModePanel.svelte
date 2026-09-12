<script lang="ts">
  import { aiStatusStore } from "./ai-status-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  // No CSS custom properties exist for the ANSI palette (theme-loader.ts
  // only exposes chrome colors like --titlebar-bg) — read straight from the
  // active theme instead, same convention SettingsPanel's swatch dots use.
  const colors = $derived(themeStore.current.colors);
</script>

<div class="ai-mode-panel">
  <span class="ai-mode-badge">
    <i style="background:{colors.blue}"></i>
    Corriendo {aiStatusStore.counts.running}
  </span>
  <span class="ai-mode-badge">
    <i style="background:{colors.yellow}"></i>
    Esperando {aiStatusStore.counts.awaiting}
  </span>
  <span class="ai-mode-badge">
    <i style="background:{colors.red}"></i>
    Error {aiStatusStore.counts.error}
  </span>
  <span class="ai-mode-badge">
    <i style="background:{colors.green}"></i>
    Listo {aiStatusStore.counts.finished}
  </span>
</div>

<style>
  .ai-mode-panel {
    flex-shrink: 0;
    display: flex;
    align-items: stretch;
    font-size: 12px;
    background: var(--tabbar-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    color: var(--tabbar-fg, #d4d4d4);
  }

  .ai-mode-badge {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 4px 12px;
  }

  .ai-mode-badge:not(:last-child) {
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .ai-mode-badge i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    display: block;
  }
</style>
