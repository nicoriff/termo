<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
</script>

<section>
  <h3>{t("preferences.aiMode.title")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.detect")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiModeEnabled}
      onchange={(e) => appConfigStore.setAiModeEnabled(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.aiMode.detectHint")}</p>
  <p class="setting-hint">{t("preferences.aiMode.perAgentHint")}</p>
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

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 6px;
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
