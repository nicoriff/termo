<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import type { PromptEngine } from "$lib/core/types";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { injectFontFace } from "$lib/theme/google-fonts";
  import {
    getOhMyPoshStatus,
    installNerdFont,
    installOhMyPosh,
    installOhMyPoshTheme,
    NERD_FONT_FAMILY,
    OH_MY_POSH_THEMES,
  } from "$lib/core/oh-my-posh-client";
  import { getStarshipStatus, installStarship, installStarshipPreset, STARSHIP_PRESETS } from "$lib/core/starship-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "../SettingDropdown.svelte";
  import PromptToolSection from "./PromptToolSection.svelte";

  let fontBusy = $state(false);
  let fontError = $state<string | null>(null);

  function useNerdFont() {
    fontBusy = true;
    fontError = null;
    void (async () => {
      try {
        const path = await installNerdFont();
        injectFontFace(NERD_FONT_FAMILY, path);
        themeStore.setFontFamily(NERD_FONT_FAMILY);
        appConfigStore.setInstalledFont(path);
      } catch (err) {
        fontError = err instanceof Error ? err.message : String(err);
      } finally {
        fontBusy = false;
      }
    })();
  }
</script>

<section>
  <div class="row">
    <span>{t("preferences.prompt.engine")}</span>
    <SettingDropdown
      options={[
        { value: "none", label: t("preferences.prompt.none") },
        { value: "oh_my_posh", label: "Oh My Posh" },
        { value: "starship", label: "Starship" },
      ]}
      value={appConfigStore.promptEngine}
      label={t("preferences.prompt.engine")}
      onpick={(value) => appConfigStore.setPromptEngine(value as PromptEngine)}
    />
  </div>
  <p class="setting-hint">{t("preferences.prompt.engineHint")}</p>
</section>

{#if appConfigStore.promptEngine === "oh_my_posh"}
  <PromptToolSection
    name="Oh My Posh"
    getStatus={getOhMyPoshStatus}
    install={async () => {
      await installOhMyPosh();
      await installOhMyPoshTheme(appConfigStore.ohMyPoshTheme);
    }}
    themes={OH_MY_POSH_THEMES}
    theme={appConfigStore.ohMyPoshTheme}
    themeLabel={t("preferences.ohMyPosh.theme")}
    pickTheme={async (theme) => {
      await installOhMyPoshTheme(theme);
      appConfigStore.setOhMyPoshTheme(theme);
    }}
    searching={t("preferences.ohMyPosh.searching")}
    installHint={t("preferences.ohMyPosh.installHint")}
    hint={[t("preferences.ohMyPosh.promptHint"), isMac ? t("preferences.ohMyPosh.promptHintMac") : ""]
      .filter(Boolean)
      .join(" ")}
  />
{:else if appConfigStore.promptEngine === "starship"}
  <PromptToolSection
    name="Starship"
    getStatus={getStarshipStatus}
    install={async () => {
      await installStarship();
      await installStarshipPreset(appConfigStore.starshipPreset);
    }}
    themes={STARSHIP_PRESETS}
    theme={appConfigStore.starshipPreset}
    themeLabel={t("preferences.starship.preset")}
    pickTheme={async (preset) => {
      await installStarshipPreset(preset);
      appConfigStore.setStarshipPreset(preset);
    }}
    themeInactive={appConfigStore.starshipUseOwnConfig}
    searching={t("preferences.starship.searching")}
    installHint={t("preferences.starship.installHint")}
    hint={t("preferences.starship.promptHint")}
  >
    {#snippet children({ installed })}
      <label class="toggle-row">
        <span>{t("preferences.starship.useOwnConfig")}</span>
        <input
          type="checkbox"
          checked={appConfigStore.starshipUseOwnConfig}
          disabled={!installed}
          onchange={(e) => appConfigStore.setStarshipUseOwnConfig(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">{t("preferences.starship.useOwnConfigHint")}</p>
    {/snippet}
  </PromptToolSection>
{/if}

{#if appConfigStore.promptEngine !== "none"}
  <section>
    <h3>{t("preferences.prompt.nerdFont")}</h3>
    <div class="row">
      <span>{NERD_FONT_FAMILY}</span>
      {#if themeStore.fontFamilyOverride === NERD_FONT_FAMILY}
        <span class="setting-hint">{t("preferences.prompt.inUse")}</span>
      {:else}
        <button class="action-btn" onclick={useNerdFont} disabled={fontBusy}>
          {t(fontBusy ? "preferences.prompt.downloading" : "preferences.prompt.installAndUse")}
        </button>
      {/if}
    </div>
    <p class="setting-hint">{t("preferences.prompt.nerdFontHint")}</p>
  </section>
{/if}

{#if fontError}
  <p class="error">{fontError}</p>
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

  .action-btn {
    font-size: 12px;
    padding: 5px 10px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .action-btn:hover:not(:disabled) {
    border-color: var(--pane-active-border, #3a6df0);
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
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
