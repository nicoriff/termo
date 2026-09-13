<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { listShells } from "$lib/core/pty-client";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { ShellInfo } from "$lib/core/types";

  let shells = $state<ShellInfo[]>([]);

  onMount(() => {
    void listShells().then((list) => (shells = list));
  });

  // themeStore setters never persist themselves (kept decoupled from
  // appConfigStore) — each Preferences section that mutates a themeStore
  // field owns persisting the fields it actually touches, same pattern as
  // AppearanceSection.
  $effect(() => {
    void themeStore.outputHighlightingEnabled;
    // untrack: persist() reads every config field synchronously, which would
    // otherwise make this effect re-run (and re-save) on any config change.
    if (appConfigStore.loaded) untrack(() => void appConfigStore.persist());
  });
</script>

<section>
  <h3>Consola por defecto</h3>
  <div class="shell-list">
    {#each shells as shell (shell.id)}
      <button
        class="shell-item"
        class:selected={(appConfigStore.defaultShellId ?? shells[0]?.id) === shell.id}
        onclick={() => appConfigStore.setDefaultShell(shell.id)}
      >
        {shell.display_name}
      </button>
    {/each}
    {#if shells.length === 0}
      <p class="setting-hint">Buscando consolas disponibles...</p>
    {/if}
  </div>
</section>

<section>
  <h3>Salida de la consola</h3>
  <label class="toggle-row">
    <span>Resaltar IPs, URLs, rutas, errores...</span>
    <input
      type="checkbox"
      checked={themeStore.outputHighlightingEnabled}
      onchange={(e) => themeStore.setOutputHighlighting(e.currentTarget.checked)}
    />
  </label>
</section>

<style>
  section {
    margin-bottom: 20px;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    margin: 0 0 8px;
  }

  .shell-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .shell-item {
    text-align: left;
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: transparent;
    color: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .shell-item:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .shell-item.selected {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }
</style>
