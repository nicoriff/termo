<script lang="ts">
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isLinux } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";
  import type { TabColorsMode } from "$lib/theme/theme-paint";
  import SettingDropdown from "../SettingDropdown.svelte";

  function onOpacityInput(event: Event) {
    themeStore.setBackgroundOpacity(Number((event.currentTarget as HTMLInputElement).value));
    void appConfigStore.persist();
  }

</script>

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
  <div class="setting-row tab-colors-row">
    <span>{t("preferences.appearance.tabColors")}</span>
    <SettingDropdown
      options={[
        { value: "theme", label: t("preferences.appearance.tabColorsTheme") },
        { value: "always", label: t("preferences.appearance.tabColorsAlways") },
        { value: "never", label: t("preferences.appearance.tabColorsNever") },
      ]}
      value={appConfigStore.tabColors}
      onpick={(value) => appConfigStore.setTabColors(value as TabColorsMode)}
    />
  </div>
  <p class="setting-hint">{t("preferences.appearance.tabColorsHint")}</p>
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

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
  }

  .tab-colors-row {
    gap: 10px;
    margin-top: 12px;
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
