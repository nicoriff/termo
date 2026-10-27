<script lang="ts">
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { Theme } from "$lib/theme/theme-types";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isLinux } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";

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

  /** The grid starts folded: with twenty-odd themes, open it pushed the rest
   * of the section a screen down. Picking doesn't fold it back, so several
   * can be tried in a row. */
  let open = $state(false);

  /** What the folded picker shows: the theme in use, or the light one and
   * the dark one when following the system. */
  const chosen = $derived(
    (themeStore.syncWithOs ? [themeStore.lightTheme, themeStore.darkTheme] : [themeStore.manualTheme])
      .map((name) => themeStore.themes.find((theme) => theme.name === name))
      .filter((theme): theme is Theme => theme !== undefined),
  );
</script>

<section>
  <h3>{t("preferences.appearance.theme")}</h3>
  <label class="toggle-row sync-row">
    <span>{t("preferences.appearance.syncWithOs")}</span>
    <input
      type="checkbox"
      checked={themeStore.syncWithOs}
      onchange={onSyncChange}
    />
  </label>
  <p class="setting-hint">{t("preferences.appearance.syncWithOsHint")}</p>
  {#if themeStore.syncWithOs}
    <p class="setting-hint">{t("preferences.appearance.activeTheme", { theme: themeStore.current.name })}</p>
  {/if}
  <div class="theme-picker" class:open>
  <button
    class="theme-trigger"
    class:open
    type="button"
    aria-expanded={open}
    aria-controls="appearance-theme-grid"
    onclick={() => (open = !open)}
  >
    <span class="trigger-themes">
      {#each chosen as theme (theme.name)}
        <span class="trigger-theme">
          <i
            class="trigger-chip"
            style="background:{theme.colors.background}; --swatch-fg:{theme.colors.foreground}; --swatch-accent:{theme
              .ui.accent}"
          ></i>
          <span class="trigger-name">{theme.name}</span>
        </span>
      {/each}
    </span>
    <svg viewBox="0 0 10 6" width="9" height="6" aria-hidden="true">
      <path d="M0 0l5 5 5-5" fill="none" stroke="currentColor" stroke-width="1" />
    </svg>
  </button>
  {#if open}
  <div id="appearance-theme-grid" class="theme-grid" role="group" aria-label={t("preferences.appearance.theme")}>
    {#each themeStore.themes as theme (theme.name)}
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
  {/if}
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

  .toggle-row.sync-row {
    margin-top: 0;
  }

  /* The picker is one box: folded, it looks like the font picker's trigger;
     open, the same box stretches down to hold the grid. */
  .theme-picker {
    margin-top: 14px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
  }

  .theme-picker:not(.open):hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  /* What's chosen, and a chevron to open the grid. */
  .theme-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: 0;
    border-radius: 3px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    text-align: left;
  }

  .theme-trigger.open {
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 3px 3px 0 0;
  }

  .theme-trigger svg {
    flex-shrink: 0;
    opacity: 0.7;
  }

  .theme-trigger.open svg {
    transform: rotate(180deg);
  }

  .trigger-themes {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .trigger-theme {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }

  /* The theme's background with a stripe of its accent. */
  .trigger-chip {
    display: block;
    flex-shrink: 0;
    width: 18px;
    height: 13px;
    border: 1px solid color-mix(in srgb, var(--swatch-fg) 25%, transparent);
    border-radius: 3px;
    box-shadow: inset 0 -3px 0 var(--swatch-accent);
  }

  .trigger-name {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* One grid, light and dark together: each row already shows which it is.
     Two columns at least, more when the window has room. With Sync with OS
     on, two are marked: the light one and the dark one. */
  .theme-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 8px;
    padding: 8px;
  }

  /* The faint border, mixed from the theme's text color, keeps a row apart
     from a window that has the same background. */
  .theme-swatch {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid color-mix(in srgb, currentColor 18%, transparent);
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
    box-shadow: 0 0 0 1px var(--swatch-accent);
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
