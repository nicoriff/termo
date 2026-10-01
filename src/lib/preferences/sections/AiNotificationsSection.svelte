<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { playSound, type SoundChoice, type SoundEvent } from "$lib/ai-mode/sounds";
  import { TOOL_IDS } from "$lib/ai-mode/pattern-table";

  const EVENTS: SoundEvent[] = ["awaiting", "finished"];

  /** What the volume slider plays once let go: the first sound set anywhere
   * (each agent's own are picked on its page). */
  function sampleSound(): SoundChoice {
    for (const tool of TOOL_IDS) {
      if (!appConfigStore.aiSoundsEnabled[tool]) continue;
      for (const event of EVENTS) {
        if (appConfigStore.aiSounds[tool][event] !== "none") return appConfigStore.aiSounds[tool][event];
      }
    }
    return "none";
  }
</script>

<section>
  <h3>{t("preferences.aiMode.notices")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.notifyAwaiting")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiNotifyAwaiting}
      onchange={(e) => appConfigStore.setAiNotifyAwaiting(e.currentTarget.checked)}
    />
  </label>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.notifyFinished")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiNotifyFinished}
      onchange={(e) => appConfigStore.setAiNotifyFinished(e.currentTarget.checked)}
    />
  </label>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.notifyError")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiNotifyError}
      onchange={(e) => appConfigStore.setAiNotifyError(e.currentTarget.checked)}
    />
  </label>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.dockBadge")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiDockBadge}
      onchange={(e) => appConfigStore.setAiDockBadge(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.aiMode.noticesHint")}</p>
</section>

<section>
  <h3>{t("preferences.aiMode.sounds")}</h3>
  <div class="sound-row">
    <span>{t("preferences.aiMode.soundVolume")}</span>
    <div class="sound-controls">
      <input
        class="volume"
        type="range"
        min="0"
        max="100"
        step="5"
        value={appConfigStore.aiSoundVolume}
        aria-label={t("preferences.aiMode.soundVolume")}
        oninput={(e) => appConfigStore.setAiSoundVolume(Number(e.currentTarget.value))}
        onchange={() => playSound(sampleSound(), appConfigStore.aiSoundVolume)}
      />
      <span class="volume-value">{appConfigStore.aiSoundVolume}%</span>
    </div>
  </div>
  <p class="setting-hint">{t("preferences.aiMode.soundsHint")}</p>
  <p class="setting-hint">{t("preferences.aiMode.soundsPerAgent")}</p>
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

  .sound-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 8px;
  }

  .sound-controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .volume {
    width: 140px;
    accent-color: var(--pane-active-border, #3a6df0);
    cursor: pointer;
  }

  .volume-value {
    width: 34px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }
</style>
