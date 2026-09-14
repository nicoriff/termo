<script lang="ts">
  import { untrack } from "svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isLinux } from "$lib/core/platform";

  function onOpacityInput(event: Event) {
    themeStore.setBackgroundOpacity(Number((event.currentTarget as HTMLInputElement).value));
  }

  // themeStore setters never persist themselves; each Preferences section
  // persists the fields it changes (FontSection owns the font ones).
  $effect(() => {
    void themeStore.current.name;
    void themeStore.backgroundOpacity;
    // untrack: persist() reads every config field synchronously, which would
    // otherwise make this effect re-run (and re-save) on any config change.
    if (appConfigStore.loaded) untrack(() => void appConfigStore.persist());
  });
</script>

<section>
  <h3>Tema</h3>
  <div class="theme-grid">
    {#each themeStore.themes as theme (theme.name)}
      <button
        class="theme-swatch"
        class:selected={themeStore.current.name === theme.name}
        style="background:{theme.colors.background}; color:{theme.colors.foreground}; --swatch-accent:{theme.ui
          .accent}"
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
  <h3>Transparencia</h3>
  <div class="setting-row">
    <span>Opacidad del fondo</span>
    <span class="setting-value">{themeStore.backgroundOpacity}%</span>
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
  <label class="toggle-row" class:disabled={isLinux}>
    <span>Desenfocar lo que está detrás</span>
    <input
      type="checkbox"
      checked={appConfigStore.backgroundBlur}
      disabled={isLinux}
      onchange={(e) => appConfigStore.setBackgroundBlur(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">
    {isLinux ? "No disponible en Linux." : "Se nota más con la opacidad del fondo por debajo de 85%."}
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

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
  }

  .setting-value {
    min-width: 36px;
    text-align: center;
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
    margin-top: 12px;
    font-size: 12px;
    cursor: pointer;
  }

  .toggle-row.disabled {
    opacity: 0.5;
    cursor: default;
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
