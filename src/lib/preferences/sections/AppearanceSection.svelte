<script lang="ts">
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { Appearance, Theme } from "$lib/theme/theme-types";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isLinux } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";

  const appearances: Appearance[] = ["light", "dark"];

  function onOpacityInput(event: Event) {
    themeStore.setBackgroundOpacity(Number((event.currentTarget as HTMLInputElement).value));
    void appConfigStore.persist();
  }

  // Save only user edits. An OS change or a config broadcast must not save again.
  function chooseTheme(theme: Theme) {
    if (themeStore.syncWithOs) themeStore.setAppearanceTheme(theme.appearance, theme.name);
    else themeStore.selectTheme(theme.name);
    void appConfigStore.persist();
  }

  function onSyncChange(event: Event) {
    themeStore.setSyncWithOs((event.currentTarget as HTMLInputElement).checked);
    void appConfigStore.persist();
  }

  function isSelected(theme: Theme): boolean {
    if (!themeStore.syncWithOs) return themeStore.manualTheme === theme.name;
    return (theme.appearance === "light" ? themeStore.lightTheme : themeStore.darkTheme) === theme.name;
  }
</script>

<section>
  <h3>{t("preferences.appearance.theme")}</h3>
  <label class="toggle-row sync-row">
    <span>{t("preferences.appearance.syncWithOs")}</span>
    <input
      class="appearance-switch"
      type="checkbox"
      role="switch"
      checked={themeStore.syncWithOs}
      onchange={onSyncChange}
    />
  </label>
  <p class="setting-hint">{t("preferences.appearance.syncWithOsHint")}</p>
  {#if themeStore.syncWithOs}
    <p class="setting-hint">{t("preferences.appearance.activeTheme", { theme: themeStore.current.name })}</p>
  {/if}
  {#each appearances as appearance}
    <div
      class="theme-category"
      role="group"
      aria-label={t(appearance === "light" ? "preferences.appearance.lightThemes" : "preferences.appearance.darkThemes")}
    >
      <h4>{t(appearance === "light" ? "preferences.appearance.lightThemes" : "preferences.appearance.darkThemes")}</h4>
      <div class="theme-grid">
        {#each themeStore.themes.filter((theme) => theme.appearance === appearance) as theme (theme.name)}
          {@const selected = isSelected(theme)}
          <button
            class="theme-swatch"
            class:selected
            aria-pressed={selected}
            style="background:{theme.colors.background}; color:{theme.colors.foreground}; --swatch-accent:{theme.ui
              .accent}"
            onclick={() => chooseTheme(theme)}
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
    </div>
  {/each}
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

<section>
  <h3>{t("preferences.appearance.tabs")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.appearance.tabsFillWidth")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.tabsFillWidth}
      onchange={(e) => appConfigStore.setTabsFillWidth(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.appearance.tabsFillWidthHint")}</p>
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

  .theme-category {
    margin-top: 16px;
  }

  h4 {
    font-size: 11px;
    font-weight: 500;
    margin: 0 0 8px;
  }

  .toggle-row.sync-row {
    margin-top: 0;
  }

  .appearance-switch {
    appearance: none;
    position: relative;
    width: 30px;
    height: 18px;
    margin: 0;
    border: 1px solid var(--tabbar-fg, #d4d4d4);
    border-radius: 9px;
    background: var(--tabbar-border, #2a2a2a);
  }

  .appearance-switch::before {
    content: "";
    position: absolute;
    width: 12px;
    height: 12px;
    top: 2px;
    left: 2px;
    border-radius: 50%;
    background: var(--tabbar-fg, #d4d4d4);
  }

  .appearance-switch:checked {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
  }

  .appearance-switch:checked::before {
    transform: translateX(12px);
    background: #fff;
  }

  .appearance-switch:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 3px;
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
