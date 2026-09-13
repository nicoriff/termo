<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
</script>

<section>
  <h3>Modo IA</h3>
  <label class="toggle-row">
    <span>Detectar estado de Claude Code / Codex en las pestañas</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiModeEnabled}
      onchange={(e) => appConfigStore.setAiModeEnabled(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">
    Muestra un panel con conteos, un punto de estado por pestaña y el modo actual en la barra de estado.
  </p>
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
