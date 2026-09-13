<script lang="ts">
  import { untrack } from "svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import FontPicker from "../FontPicker.svelte";

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

  // The only place in the app that calls theme setters as a direct result of
  // user action (Preferences replaced the old in-window settings panel), so
  // this is the only place that needs to persist theme changes — everywhere
  // else, themeStore changes come from applyFields() reacting to a synced
  // config-updated event, which must never re-trigger a save.
  $effect(() => {
    void themeStore.current.name;
    void themeStore.fontSizeOverride;
    void themeStore.fontFamilyOverride;
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
</style>
