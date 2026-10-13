<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { Theme } from "$lib/theme/theme-types";

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
          .foreground}; --p-border:{theme.ui.border}; --p-accent:{theme.ui.accent}"
        style:font-family={themeStore.effectiveFontFamily}
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
    <input class="onb-check" type="checkbox" checked={themeStore.syncWithOs} onchange={onSyncChange} />
  </label>

  <div class="theme-grid" role="group" aria-label={t("preferences.appearance.theme")}>
    {#each themeStore.themes as theme (theme.name)}
      {@const selected = isSelected(theme)}
      <button
        class="swatch"
        class:selected
        aria-pressed={selected}
        title={theme.name}
        onclick={() => choose(theme)}
        onpointerenter={() => (hovered = theme)}
        onpointerleave={() => (hovered = null)}
        onfocus={() => (hovered = theme)}
        onblur={() => (hovered = null)}
      >
        <span
          class="swatch-preview"
          style="background:{theme.colors.background}; --swatch-fg:{theme.colors.foreground}; --swatch-accent:{theme.ui
            .accent}"
        >
          <span class="swatch-line">
            <i style="background:{theme.colors.green}; width:6px"></i>
            <i style="background:{theme.colors.foreground}; width:30px"></i>
          </span>
          <span class="swatch-line">
            <i style="background:{theme.colors.blue}; width:18px"></i>
            <i style="background:{theme.colors.magenta}; width:12px"></i>
          </span>
          <span class="swatch-line">
            <i style="background:{theme.colors.yellow}; width:12px"></i>
            <i style="background:{theme.colors.red}; width:22px"></i>
          </span>
        </span>
        <span class="swatch-name">{theme.name}</span>
      </button>
    {/each}
  </div>
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

  /* Same previews as Preferences → Appearance: light and dark in one grid,
     every one the same size, with its name underneath. */
  .theme-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: 10px 6px;
    margin-top: 12px;
  }

  .swatch {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 11px;
    text-align: left;
    cursor: pointer;
  }

  .swatch-preview {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 5px;
    height: 46px;
    padding: 0 9px;
    border: 2px solid color-mix(in srgb, var(--swatch-fg) 18%, transparent);
    border-radius: 6px;
  }

  .swatch-line {
    display: flex;
    gap: 4px;
  }

  .swatch-line i {
    display: block;
    height: 4px;
    border-radius: 2px;
  }

  .swatch.selected .swatch-preview {
    border-color: var(--swatch-accent);
  }

  .swatch:focus-visible {
    outline: none;
  }

  .swatch:focus-visible .swatch-preview {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 2px;
  }

  .swatch-name {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    opacity: 0.75;
  }

  .swatch.selected .swatch-name {
    opacity: 1;
    font-weight: 600;
  }
</style>
