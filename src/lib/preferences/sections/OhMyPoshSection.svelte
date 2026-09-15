<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { injectFontFace } from "$lib/theme/google-fonts";
  import {
    getOhMyPoshStatus,
    installNerdFont,
    installOhMyPosh,
    installOhMyPoshTheme,
    NERD_FONT_FAMILY,
    OH_MY_POSH_THEMES,
    type OhMyPoshStatus,
  } from "$lib/core/oh-my-posh-client";
  import { t } from "$lib/i18n/i18n.svelte";

  let status = $state<OhMyPoshStatus | null>(null);
  let busy = $state<"binary" | "theme" | "font" | null>(null);
  let error = $state<string | null>(null);

  async function refresh() {
    status = await getOhMyPoshStatus();
  }

  onMount(() => {
    void refresh();
  });

  async function run(kind: "binary" | "theme" | "font", task: () => Promise<void>) {
    busy = kind;
    error = null;
    try {
      await task();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = null;
    }
  }

  function install() {
    void run("binary", async () => {
      await installOhMyPosh();
      await installOhMyPoshTheme(appConfigStore.ohMyPoshTheme);
      await refresh();
    });
  }

  function pickTheme(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const theme = select.value;
    void run("theme", async () => {
      try {
        await installOhMyPoshTheme(theme);
        appConfigStore.setOhMyPoshTheme(theme);
      } catch (err) {
        select.value = appConfigStore.ohMyPoshTheme;
        throw err;
      }
    });
  }

  function useNerdFont() {
    void run("font", async () => {
      const path = await installNerdFont();
      injectFontFace(NERD_FONT_FAMILY, path);
      themeStore.setFontFamily(NERD_FONT_FAMILY);
      appConfigStore.setInstalledFont(path);
    });
  }
</script>

<svelte:window onfocus={() => void refresh()} />

<section>
  <h3>{t("preferences.ohMyPosh.installation")}</h3>
  {#if status === null}
    <p class="setting-hint">{t("preferences.ohMyPosh.searching")}</p>
  {:else if status.path}
    <p class="status-line">
      {t(status.managed ? "preferences.ohMyPosh.installedByTermini" : "preferences.ohMyPosh.installed")}
      <code>{status.path}</code>
    </p>
  {:else}
    <div class="row">
      <span>{t("preferences.ohMyPosh.notInstalled")}</span>
      <button class="action-btn" onclick={install} disabled={busy !== null}>
        {t(busy === "binary" ? "preferences.ohMyPosh.downloading" : "preferences.ohMyPosh.install")}
      </button>
    </div>
    <p class="setting-hint">{t("preferences.ohMyPosh.installHint")}</p>
  {/if}
</section>

<section>
  <h3>{t("preferences.ohMyPosh.prompt")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.ohMyPosh.enable")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.ohMyPoshEnabled}
      disabled={!status?.path}
      onchange={(e) => appConfigStore.setOhMyPoshEnabled(e.currentTarget.checked)}
    />
  </label>
  <div class="row">
    <span>{t("preferences.ohMyPosh.theme")}</span>
    <select value={appConfigStore.ohMyPoshTheme} onchange={pickTheme} disabled={busy !== null || !status?.path}>
      {#each OH_MY_POSH_THEMES as theme (theme)}
        <option value={theme}>{theme}</option>
      {/each}
    </select>
  </div>
  <p class="setting-hint">
    {[
      t("preferences.ohMyPosh.promptHint"),
      isMac ? t("preferences.ohMyPosh.promptHintMac") : "",
      t("preferences.ohMyPosh.onlyNewSessions"),
    ]
      .filter(Boolean)
      .join(" ")}
  </p>
</section>

<section>
  <h3>{t("preferences.ohMyPosh.nerdFont")}</h3>
  <div class="row">
    <span>{NERD_FONT_FAMILY}</span>
    {#if themeStore.fontFamilyOverride === NERD_FONT_FAMILY}
      <span class="setting-hint">{t("preferences.ohMyPosh.inUse")}</span>
    {:else}
      <button class="action-btn" onclick={useNerdFont} disabled={busy !== null}>
        {t(busy === "font" ? "preferences.ohMyPosh.downloading" : "preferences.ohMyPosh.installAndUse")}
      </button>
    {/if}
  </div>
  <p class="setting-hint">{t("preferences.ohMyPosh.nerdFontHint")}</p>
</section>

{#if error}
  <p class="error">{error}</p>
{/if}

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

  .row,
  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 8px;
  }

  .toggle-row {
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .status-line {
    margin: 0;
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  code {
    font-size: 10px;
    opacity: 0.7;
    word-break: break-all;
  }

  .action-btn,
  select {
    font-size: 12px;
    padding: 5px 10px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .action-btn:hover:not(:disabled),
  select:hover:not(:disabled) {
    border-color: var(--pane-active-border, #3a6df0);
  }

  button:disabled,
  select:disabled {
    opacity: 0.5;
    cursor: default;
  }

  option {
    background: var(--tabbar-active-bg, #1e1e1e);
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .error {
    font-size: 11px;
    color: #f14c4c;
  }
</style>
