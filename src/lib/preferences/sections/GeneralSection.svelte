<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { layoutsStore } from "$lib/layout/layouts-store.svelte";
  import { t, type LanguagePreference } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "../SettingDropdown.svelte";

  // "Español"/"English" stay in their own language so they're recognizable
  // whichever UI language is active.
  const languageOptions = $derived([
    { value: "system", label: t("preferences.general.system") },
    { value: "es", label: "Español" },
    { value: "en", label: "English" },
  ]);

  /** A layout name is never empty (the save dialog trims and requires one), so
   * the empty string is free to mean "no layout". */
  const NO_LAYOUT = "";

  const layoutOptions = $derived([
    { value: NO_LAYOUT, label: t("preferences.general.noLayout") },
    ...layoutsStore.layouts.map((layout) => ({ value: layout.name, label: layout.name })),
  ]);

  // This window is preloaded once and reused, so its copy of the list can be
  // older than the main window's — reloaded here and again on every open.
  onMount(() => void layoutsStore.load());
</script>

<section>
  <h3>{t("preferences.general.language")}</h3>
  <div class="row">
    <span>{t("preferences.general.languageLabel")}</span>
    <SettingDropdown
      options={languageOptions}
      value={appConfigStore.language}
      onpick={(value) => appConfigStore.setLanguage(value as LanguagePreference)}
    />
  </div>
  <p class="setting-hint">{t("preferences.general.languageHint")}</p>
</section>

<section>
  <h3>{t("preferences.general.startup")}</h3>
  <div class="row">
    <span>{t("preferences.general.defaultLayout")}</span>
    <SettingDropdown
      options={layoutOptions}
      value={appConfigStore.defaultLayout ?? NO_LAYOUT}
      onopen={() => void layoutsStore.load()}
      onpick={(value) => appConfigStore.setDefaultLayout(value === NO_LAYOUT ? null : value)}
    />
  </div>
  <p class="setting-hint">{t("preferences.general.defaultLayoutHint")}</p>
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

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }
</style>
