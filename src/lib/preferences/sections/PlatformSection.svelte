<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac, isWindows } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";

  let { os }: { os: "mac" | "windows" | "linux" } = $props();

  const currentOs = isMac ? "mac" : isWindows ? "windows" : "linux";
  const active = $derived(os === currentOs);
</script>

<div class="platform-section" class:disabled={!active}>
  {#if !active}
    <p class="setting-hint">
      {t("preferences.platform.onlyEditableOn", {
        os: os === "mac" ? "macOS" : os === "windows" ? "Windows" : "Linux",
      })}
    </p>
  {/if}

  {#if os === "windows"}
    <section>
      <h3>PowerShell</h3>
      <label class="toggle-row">
        <span>{t("preferences.platform.menuComplete")}</span>
        <input
          type="checkbox"
          checked={appConfigStore.psreadlineMenuComplete}
          disabled={!active}
          onchange={(e) => appConfigStore.setPsReadlineMenuComplete(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">{t("preferences.platform.psreadlineHint")}</p>
    </section>

    <section>
      <h3>{t("preferences.platform.encoding")}</h3>
      <label class="toggle-row">
        <span>{t("preferences.platform.utf8")}</span>
        <input
          type="checkbox"
          checked={appConfigStore.consoleUtf8}
          disabled={!active}
          onchange={(e) => appConfigStore.setConsoleUtf8(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">{t("preferences.platform.utf8Hint")}</p>
    </section>
  {:else}
    <section>
      <h3>zsh</h3>
      <label class="toggle-row">
        <span>{t("preferences.platform.menuComplete")}</span>
        <input
          type="checkbox"
          checked={appConfigStore.zshMenuComplete}
          disabled={!active}
          onchange={(e) => appConfigStore.setZshMenuComplete(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">{t("preferences.platform.zshHint")}</p>
    </section>

    <section>
      <h3>Bash</h3>
      <label class="toggle-row">
        <span>{t("preferences.platform.bashColors")}</span>
        <input
          type="checkbox"
          checked={appConfigStore.bashPromptColors}
          disabled={!active}
          onchange={(e) => appConfigStore.setBashPromptColors(e.currentTarget.checked)}
        />
      </label>
      <p class="setting-hint">
        {os === "mac"
          ? `${t("preferences.platform.bashMacExtra")} ${t("preferences.platform.bashHint")}`
          : t("preferences.platform.bashHint")}
      </p>
    </section>
  {/if}
</div>

<style>
  .platform-section.disabled {
    opacity: 0.5;
    pointer-events: none;
  }

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

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    cursor: pointer;
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
