<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { layoutsStore } from "$lib/layout/layouts-store.svelte";
  import { t, type LanguagePreference } from "$lib/i18n/i18n.svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import SettingDropdown from "../SettingDropdown.svelte";
  import { setKeepAwake } from "$lib/titlebar/keep-awake";
  import { listBrowsers, openLink, type Browser } from "$lib/core/links";
  import { isMac, isWindows } from "$lib/core/platform";

  /** Hiding the button would leave a session on with nowhere to turn it
   * off, so hiding it also ends one. */
  function setKeepAwakeButton(visible: boolean) {
    appConfigStore.setKeepAwakeButton(visible);
    if (!visible) void setKeepAwake(false, false, null).catch(() => {});
  }

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

  /** The system's default browser, and the choice to pick a program. Paths
   * are never empty, so neither value can be one. */
  const SYSTEM_BROWSER = "";
  const OTHER_BROWSER = "\u0000other";

  let browsers = $state<Browser[]>([]);

  function loadBrowsers() {
    void listBrowsers()
      .then((found) => (browsers = found))
      .catch(() => {});
  }

  /** The last part of a path, without `.exe`, `.app` or `.desktop`. */
  function programName(path: string): string {
    const last = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
    return last.replace(/\.(exe|app|desktop)$/i, "");
  }

  const browserOptions = $derived.by(() => {
    const chosen = appConfigStore.linkBrowser;
    const options = [
      { value: SYSTEM_BROWSER, label: t("preferences.general.systemBrowser") },
      ...browsers.map((browser) => ({ value: browser.path, label: browser.name })),
    ];
    // One picked with "Other…", or one no longer found: still shown as chosen.
    if (chosen && !browsers.some((browser) => browser.path === chosen)) {
      options.push({ value: chosen, label: programName(chosen) });
    }
    options.push({ value: OTHER_BROWSER, label: t("preferences.general.otherBrowser") });
    return options;
  });

  async function pickBrowser(value: string) {
    if (value !== OTHER_BROWSER) {
      appConfigStore.setLinkBrowser(value === SYSTEM_BROWSER ? null : value);
      return;
    }
    const picked = await openDialog({
      title: t("preferences.general.otherBrowserTitle"),
      multiple: false,
      directory: false,
      defaultPath: isWindows ? "C:\\Program Files" : isMac ? "/Applications" : "/usr/share/applications",
      filters: isWindows
        ? [{ name: t("preferences.general.programs"), extensions: ["exe"] }]
        : isMac
          ? [{ name: t("preferences.general.programs"), extensions: ["app"] }]
          : undefined,
    }).catch(() => null);
    if (typeof picked === "string" && picked) appConfigStore.setLinkBrowser(picked);
  }

  // This window is preloaded once and reused, so its copy of the list can be
  // older than the main window's — reloaded here and again on every open.
  onMount(() => {
    void layoutsStore.load();
    loadBrowsers();
  });
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

<section>
  <h3>{t("preferences.general.titleBar")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.general.keepAwakeButton")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.keepAwakeButton}
      onchange={(e) => setKeepAwakeButton(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.general.keepAwakeButtonHint")}</p>
  <label class="row toggle-row">
    <span>{t("preferences.general.remoteButton")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.remoteButton}
      onchange={(e) => appConfigStore.setRemoteButton(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.general.remoteButtonHint")}</p>
</section>

<section>
  <h3>{t("preferences.general.links")}</h3>
  <div class="row">
    <span>{t("preferences.general.linkBrowser")}</span>
    <div class="row-end">
      <button class="link-test" onclick={() => void openLink("https://github.com/nicoriff/termo-releases").catch(() => {})}>
        {t("preferences.general.linkBrowserTry")}
      </button>
      <SettingDropdown
        options={browserOptions}
        value={appConfigStore.linkBrowser ?? SYSTEM_BROWSER}
        onopen={loadBrowsers}
        onpick={(value) => void pickBrowser(value)}
      />
    </div>
  </div>
  <p class="setting-hint">{t("preferences.general.linkBrowserHint")}</p>
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

  .toggle-row {
    cursor: pointer;
  }

  .row-end {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .link-test {
    background: none;
    border: none;
    padding: 0;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    cursor: pointer;
  }

  .link-test:hover {
    text-decoration: underline;
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
