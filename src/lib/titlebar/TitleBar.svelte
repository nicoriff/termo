<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    closeWindow,
    isWindowMaximized,
    minimizeWindow,
    onWindowResized,
    toggleMaximizeWindow,
  } from "$lib/core/window-client";
  import { uiState } from "$lib/core/ui-state.svelte";

  let maximized = $state(false);
  let unlisten: (() => void) | null = null;

  async function refreshMaximized() {
    maximized = await isWindowMaximized();
    uiState.setMaximized(maximized);
  }

  async function toggleMaximize() {
    await toggleMaximizeWindow();
    await refreshMaximized();
  }

  onMount(() => {
    void refreshMaximized();
    // Best-effort sync for external changes (e.g. Aero-snap to top of screen).
    // The toggle button/double-click above refresh state directly since this
    // event doesn't reliably fire for every maximize/restore transition.
    void onWindowResized(() => void refreshMaximized()).then((fn) => (unlisten = fn));
  });

  onDestroy(() => unlisten?.());

  function handleDoubleClick() {
    void toggleMaximize();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="titlebar" data-tauri-drag-region ondblclick={handleDoubleClick}>
  <div class="titlebar-brand" data-tauri-drag-region>
    <img class="titlebar-icon" src="/app-icon.png" alt="" />
    <span class="titlebar-title">Termini</span>
  </div>

  <div class="titlebar-spacer" data-tauri-drag-region></div>

  <div class="titlebar-controls">
    <button
      class="titlebar-button"
      aria-label="Configuración"
      onclick={() => uiState.toggleSettings()}
    >
      <svg viewBox="0 0 14 14" width="13" height="13">
        <line x1="1" y1="3" x2="13" y2="3" stroke="currentColor" stroke-width="1" />
        <circle cx="9" cy="3" r="1.5" fill="currentColor" />
        <line x1="1" y1="7" x2="13" y2="7" stroke="currentColor" stroke-width="1" />
        <circle cx="5" cy="7" r="1.5" fill="currentColor" />
        <line x1="1" y1="11" x2="13" y2="11" stroke="currentColor" stroke-width="1" />
        <circle cx="10" cy="11" r="1.5" fill="currentColor" />
      </svg>
    </button>
    <button
      class="titlebar-button"
      aria-label="Minimizar"
      onclick={() => void minimizeWindow()}
    >
      <svg viewBox="0 0 10 10" width="10" height="10">
        <path d="M0 5h10" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
    <button
      class="titlebar-button"
      aria-label={maximized ? "Restaurar" : "Maximizar"}
      onclick={() => void toggleMaximize()}
    >
      {#if maximized}
        <svg viewBox="0 0 10 10" width="10" height="10">
          <path
            d="M2 0h8v8h-2M0 2h8v8H0z"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
          />
        </svg>
      {:else}
        <svg viewBox="0 0 10 10" width="10" height="10">
          <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      {/if}
    </button>
    <button
      class="titlebar-button titlebar-close"
      aria-label="Cerrar"
      onclick={() => void closeWindow()}
    >
      <svg viewBox="0 0 10 10" width="10" height="10">
        <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
  </div>
</div>

<style>
  .titlebar {
    display: flex;
    align-items: stretch;
    height: var(--titlebar-height, 34px);
    background: var(--titlebar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    user-select: none;
    flex-shrink: 0;
  }

  .titlebar-brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    opacity: 0.85;
  }

  .titlebar-icon {
    width: 16px;
    height: 16px;
  }

  .titlebar-title {
    font-family: "Ubuntu", sans-serif;
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .titlebar-spacer {
    flex: 1;
  }

  .titlebar-controls {
    display: flex;
    align-items: stretch;
    padding-right: 4px;
  }

  .titlebar-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .titlebar-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .titlebar-close:hover {
    background: var(--titlebar-close-hover-bg, #e81123);
    color: #fff;
  }
</style>
