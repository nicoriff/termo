<script lang="ts">
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import FontPicker from "./FontPicker.svelte";

  function decreaseFontSize() {
    themeStore.setFontSize(Math.max(9, themeStore.effectiveFontSize - 1));
  }
  function increaseFontSize() {
    themeStore.setFontSize(Math.min(28, themeStore.effectiveFontSize + 1));
  }
  function resetFontSize() {
    themeStore.setFontSize(null);
  }

  function onOpacityInput(event: Event) {
    themeStore.setBackgroundOpacity(Number((event.currentTarget as HTMLInputElement).value));
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="settings-overlay" onpointerdown={() => uiState.toggleSettings()}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="settings-panel" onpointerdown={(e) => e.stopPropagation()}>
    <div class="settings-header">
      <h2>Apariencia</h2>
      <button class="close-btn" aria-label="Cerrar configuración" onclick={() => uiState.toggleSettings()}>
        <svg viewBox="0 0 10 10" width="11" height="11">
          <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
    </div>

    <section>
      <h3>Tema</h3>
      <div class="theme-grid">
        {#each themeStore.themes as theme (theme.name)}
          <button
            class="theme-swatch"
            class:selected={themeStore.current.name === theme.name}
            style="background:{theme.colors.background}; color:{theme.colors.foreground}; --swatch-accent:{theme
              .ui.accent}"
            onclick={() => themeStore.setTheme(theme.name)}
          >
            <span class="swatch-dots">
              <i style="background:{theme.colors.red}"></i>
              <i style="background:{theme.colors.green}"></i>
              <i style="background:{theme.colors.blue}"></i>
            </span>
            {theme.name}
          </button>
        {/each}
      </div>
    </section>

    <section>
      <h3>Fuente</h3>
      <div class="font-row">
        <span>Tamaño</span>
        <div class="font-controls">
          <button onclick={decreaseFontSize} aria-label="Reducir fuente">-</button>
          <span class="font-size-value">{themeStore.effectiveFontSize}px</span>
          <button onclick={increaseFontSize} aria-label="Agrandar fuente">+</button>
          {#if themeStore.fontSizeOverride !== null}
            <button class="reset-btn" onclick={resetFontSize}>restablecer</button>
          {/if}
        </div>
      </div>
      <div class="font-family-row">
        <FontPicker />
      </div>
    </section>

    <section>
      <h3>Transparencia</h3>
      <div class="font-row">
        <span>Opacidad del fondo</span>
        <span class="font-size-value">{themeStore.backgroundOpacity}%</span>
      </div>
      <input
        type="range"
        min="0"
        max="100"
        step="1"
        value={themeStore.backgroundOpacity}
        oninput={onOpacityInput}
        class="opacity-slider"
      />
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

    <section>
      <h3>PowerShell</h3>
      <label class="toggle-row">
        <span>Menú de autocompletado (Tab, Tab)</span>
        <input
          type="checkbox"
          checked={appConfigStore.psreadlineMenuComplete}
          onchange={(e) => appConfigStore.setPsReadlineMenuComplete(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">Solo afecta sesiones nuevas de PowerShell/pwsh abiertas desde Termini.</p>
    </section>
  </div>
</div>

<style>
  .settings-overlay {
    position: absolute;
    inset: 0;
    z-index: 100;
    display: flex;
    justify-content: flex-end;
  }

  .settings-panel {
    width: 280px;
    height: 100%;
    background: var(--tabbar-active-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    box-shadow: -4px 0 12px rgba(0, 0, 0, 0.3);
    padding: 16px;
    box-sizing: border-box;
    overflow-y: auto;
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }

  .settings-header h2 {
    font-size: 14px;
    margin: 0;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    padding: 4px;
    display: flex;
  }

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

  .theme-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .theme-swatch {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    font-size: 12px;
    cursor: pointer;
    text-align: left;
  }

  .theme-swatch.selected {
    border-color: var(--swatch-accent);
  }

  .swatch-dots {
    display: flex;
    gap: 3px;
  }

  .swatch-dots i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    display: block;
  }

  .font-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
  }

  .font-controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .font-controls button {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .font-size-value {
    min-width: 36px;
    text-align: center;
  }

  .reset-btn {
    width: auto !important;
    padding: 0 6px;
    font-size: 10px;
    opacity: 0.75;
  }

  .font-family-row {
    margin-top: 10px;
  }

  .opacity-slider {
    width: 100%;
    margin-top: 8px;
    accent-color: var(--pane-active-border, #3a6df0);
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
