<script lang="ts">
  import { untrack } from "svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isLinux } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";

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
  <h3>{t("preferences.appearance.theme")}</h3>
  <div class="theme-grid">
    {#each themeStore.themes as theme (theme.name)}
      {@const selected = themeStore.current.name === theme.name}
      <button
        class="theme-swatch"
        class:selected
        aria-pressed={selected}
        style="background:{theme.colors.background}; color:{theme.colors.foreground}; --swatch-accent:{theme.ui
          .accent}"
        onclick={() => themeStore.setTheme(theme.name)}
      >
        <span class="swatch-dots">
          <i style="background:{theme.colors.red}"></i>
          <i style="background:{theme.colors.green}"></i>
          <i style="background:{theme.colors.blue}"></i>
        </span>
        <span class="swatch-name">{theme.name}</span>
        {#if selected}
          <!-- Decorative: aria-pressed already tells a screen reader which
               theme is on. -->
          <svg class="swatch-check" viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="8" cy="8" r="8" />
            <path d="M4.5 8.2 7 10.6l4.6-5" />
          </svg>
        {/if}
      </button>
    {/each}
  </div>
</section>

<section>
  <h3>{t("preferences.appearance.transparency")}</h3>
  <div class="setting-row">
    <span>{t("preferences.appearance.opacity")}</span>
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
    <span>{t("preferences.appearance.blur")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.backgroundBlur}
      disabled={isLinux}
      onchange={(e) => appConfigStore.setBackgroundBlur(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">
    {t(isLinux ? "preferences.appearance.blurUnavailableLinux" : "preferences.appearance.blurHint")}
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

  /* Two columns: with ten presets, one column pushed the rest of the
     section a full screen down. */
  .theme-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .theme-swatch {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    font-size: 12px;
    cursor: pointer;
    text-align: left;
  }

  .swatch-name {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* A fixed green rather than the theme's own: it has to read as "this one"
     on every swatch, light or dark, whatever the theme calls green. Pushed
     to the far end of the row, clear of the name. */
  .swatch-check {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    margin-left: auto;
  }

  .swatch-check circle {
    fill: #22a55b;
  }

  .swatch-check path {
    fill: none;
    stroke: #ffffff;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .theme-swatch.selected {
    border-color: var(--swatch-accent);
  }

  .swatch-dots {
    display: flex;
    flex-shrink: 0;
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
