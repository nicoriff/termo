<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { Appearance, Theme } from "$lib/theme/theme-types";

  const appearances: Appearance[] = ["dark", "light"];

  /** The swatch under the pointer or the keyboard focus: previewed before
   * it's picked. */
  let hovered = $state<Theme | null>(null);

  const named = (name: string) => themeStore.themes.find((theme) => theme.name === name) ?? themeStore.current;

  /** Following the system there are two themes in play, so both are shown;
   * the one the system is using right now goes in front. */
  const previews = $derived.by<Theme[]>(() => {
    if (!themeStore.syncWithOs) return [hovered ?? themeStore.current];
    const light = hovered?.appearance === "light" ? hovered : named(themeStore.lightTheme);
    const dark = hovered?.appearance === "dark" ? hovered : named(themeStore.darkTheme);
    const front = hovered?.appearance ?? themeStore.systemAppearance;
    return front === "light" ? [dark, light] : [light, dark];
  });

  // Same rules as Preferences → Appearance: the stores apply, this persists.
  function choose(theme: Theme) {
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

<div class="onb-stage">
  <div class="previews" class:pair={previews.length > 1}>
    {#each previews as theme, index (theme.appearance + index)}
      {@const c = theme.colors}
      <div
        class="preview"
        role="img"
        aria-label={t("onboarding.appearance.previewLabel", { theme: theme.name })}
        style="--p-bg:{c.background}; --p-fg:{c.foreground}; --p-ui:{theme.ui.background}; --p-ui-fg:{theme.ui
          .foreground}; --p-border:{theme.ui.border}; --p-accent:{theme.ui.accent}; font-family:{themeStore.effectiveFontFamily}"
      >
        <div class="preview-tabs">
          <span class="preview-tab active">pwsh</span>
          <span class="preview-tab">Claude Code</span>
          <span class="preview-name">{theme.name}</span>
        </div>
        <div class="preview-body">
          <div><span style="color:{c.blue}">~/termo</span> <span style="color:{c.magenta}">main</span></div>
          <div><span style="color:{c.green}">❯</span> npm run build</div>
          <div><span style="color:{c.green}">✓</span> 214 modules <span style="color:{c.brightBlack}">1.8s</span></div>
          <div><span style="color:{c.yellow}">warning</span> 1 unused import</div>
          <div><span style="color:{c.green}">❯</span> git status</div>
          <div><span style="color:{c.red}">modified:</span> <span style="color:{c.cyan}">src/app.ts</span></div>
          <div><span style="color:{c.green}">❯</span> <i class="preview-cursor" style="background:{c.cursor}"></i></div>
        </div>
      </div>
    {/each}
  </div>
</div>

<div class="onb-panel">
  <h2>{t("onboarding.appearance.title")}</h2>
  <p class="onb-intro">{t("onboarding.appearance.intro")}</p>

  <label class="sync-row">
    <span>
      {t("preferences.appearance.syncWithOs")}
      <small>{t("preferences.appearance.syncWithOsHint")}</small>
    </span>
    <input class="onb-switch" type="checkbox" role="switch" checked={themeStore.syncWithOs} onchange={onSyncChange} />
  </label>

  {#each appearances as appearance}
    {@const label = t(appearance === "light" ? "preferences.appearance.lightThemes" : "preferences.appearance.darkThemes")}
    <div class="theme-group" role="group" aria-label={label}>
      <h3>{label}</h3>
      <div class="theme-grid">
        {#each themeStore.themes.filter((theme) => theme.appearance === appearance) as theme (theme.name)}
          {@const selected = isSelected(theme)}
          <button
            class="swatch"
            class:selected
            aria-pressed={selected}
            style="background:{theme.colors.background}; color:{theme.colors.foreground}; --swatch-accent:{theme.ui
              .accent}"
            onclick={() => choose(theme)}
            onpointerenter={() => (hovered = theme)}
            onpointerleave={() => (hovered = null)}
            onfocus={() => (hovered = theme)}
            onblur={() => (hovered = null)}
          >
            <span class="swatch-dots">
              <i style="background:{theme.colors.red}"></i>
              <i style="background:{theme.colors.green}"></i>
              <i style="background:{theme.colors.blue}"></i>
            </span>
            <span class="swatch-name">{theme.name}</span>
          </button>
        {/each}
      </div>
    </div>
  {/each}
  <p class="onb-hint">{t("onboarding.appearance.hint")}</p>
</div>

<style>
  .previews {
    position: relative;
    width: min(100%, 380px);
    aspect-ratio: 16 / 11;
  }

  .preview {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--p-border);
    border-radius: 8px;
    background: var(--p-bg);
    color: var(--p-fg);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.3);
    overflow: hidden;
    transition:
      background 180ms ease,
      color 180ms ease,
      border-color 180ms ease;
  }

  /* Two themes: the one behind peeks out from the top left. */
  .previews.pair .preview:first-child {
    inset: 0 14% 16% 0;
  }

  .previews.pair .preview:last-child {
    inset: 16% 0 0 14%;
  }

  .preview-tabs {
    flex-shrink: 0;
    display: flex;
    align-items: stretch;
    background: var(--p-ui);
    color: var(--p-ui-fg);
    border-bottom: 1px solid var(--p-border);
    font-family: system-ui, sans-serif;
    font-size: 10px;
  }

  .preview-tab {
    padding: 6px 10px;
    opacity: 0.6;
    border-right: 1px solid var(--p-border);
  }

  .preview-tab.active {
    opacity: 1;
    box-shadow: inset 0 -2px 0 var(--p-accent);
  }

  .preview-name {
    margin-left: auto;
    padding: 6px 10px;
    opacity: 0.6;
  }

  .preview-body {
    flex: 1;
    min-height: 0;
    padding: 10px 12px;
    font-size: 11px;
    line-height: 1.55;
    white-space: pre;
    overflow: hidden;
  }

  .preview-cursor {
    display: inline-block;
    width: 0.6em;
    height: 1.1em;
    vertical-align: text-bottom;
  }

  .sync-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 14px;
    font-size: 12px;
    cursor: pointer;
  }

  .sync-row small {
    display: block;
    margin-top: 2px;
    font-size: 10px;
    opacity: 0.6;
  }

  .theme-group {
    margin-top: 12px;
  }

  h3 {
    margin: 0 0 6px;
    font-size: 11px;
    font-weight: 500;
    opacity: 0.7;
  }

  .theme-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .swatch {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 6px 9px;
    border: 2px solid transparent;
    border-radius: 6px;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .swatch.selected {
    border-color: var(--swatch-accent);
  }

  .swatch:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 2px;
  }

  .swatch-dots {
    display: flex;
    flex-shrink: 0;
    gap: 3px;
  }

  .swatch-dots i {
    display: block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .swatch-name {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
