<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { customizeStatusBar } from "$lib/core/window-client";
  import { t } from "$lib/i18n/i18n.svelte";

  /** The bar is arranged on the bar itself, in the console window (see
   * StatusBarEditor); this page sends the user there, and back to the
   * default. */
  const customized = $derived(appConfigStore.statusBarLayout !== null);
</script>

<section>
  <h3>{t("preferences.statusBar.title")}</h3>
  <p class="setting-hint first">{t("preferences.statusBar.editHint")}</p>
  <div class="actions">
    <button class="text-button primary" onclick={() => void customizeStatusBar()}>{t("preferences.statusBar.edit")}</button>
    <button class="text-button" disabled={!customized} onclick={() => appConfigStore.setStatusBarLayout(null)}>
      {t("preferences.statusBar.restore")}
    </button>
  </div>
  <p class="setting-hint">{t("preferences.statusBar.menuHint")}</p>
</section>

<style>
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    margin: 0 0 8px;
  }

  .setting-hint {
    margin: 10px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .setting-hint.first {
    margin: 0 0 12px;
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .text-button {
    padding: 5px 12px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .text-button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .text-button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .text-button.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .text-button.primary:hover {
    filter: brightness(1.1);
  }
</style>
