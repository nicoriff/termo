<script lang="ts">
  import { untrack } from "svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import FontPicker from "../FontPicker.svelte";
  import { t } from "$lib/i18n/i18n.svelte";

  function decreaseFontSize() {
    themeStore.setFontSize(Math.max(9, themeStore.effectiveFontSize - 1));
  }
  function increaseFontSize() {
    themeStore.setFontSize(Math.min(28, themeStore.effectiveFontSize + 1));
  }
  function resetFontSize() {
    themeStore.setFontSize(null);
  }

  // themeStore setters never persist themselves; this section persists the
  // font fields it changes (AppearanceSection owns theme and opacity).
  $effect(() => {
    void themeStore.fontSizeOverride;
    void themeStore.fontFamilyOverride;
    // untrack: persist() reads every config field synchronously, which would
    // otherwise make this effect re-run (and re-save) on any config change.
    if (appConfigStore.loaded) untrack(() => void appConfigStore.persist());
  });
</script>

<section>
  <h3>{t("preferences.font.size")}</h3>
  <div class="font-row">
    <span>{t("preferences.font.sizeLabel")}</span>
    <div class="font-controls">
      <button onclick={decreaseFontSize} aria-label={t("preferences.font.decrease")}>-</button>
      <span class="font-size-value">{themeStore.effectiveFontSize}px</span>
      <button onclick={increaseFontSize} aria-label={t("preferences.font.increase")}>+</button>
      {#if themeStore.fontSizeOverride !== null}
        <button class="reset-btn" onclick={resetFontSize}>{t("preferences.font.reset")}</button>
      {/if}
    </div>
  </div>
</section>

<section>
  <h3>{t("preferences.font.family")}</h3>
  <FontPicker />
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

  .font-controls button:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  /* Same look as the other Preferences buttons (e.g. Instalar/Quitar),
     just sized to the 22px row of the −/+ controls. */
  .font-controls .reset-btn {
    width: auto;
    margin-left: 4px;
    padding: 0 10px;
    font-size: 11px;
  }
</style>
