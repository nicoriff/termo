<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "../SettingDropdown.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { SOUND_IDS, playSound, type SoundChoice, type SoundEvent } from "$lib/ai-mode/sounds";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { TOOL_IDS, type ToolId } from "$lib/ai-mode/pattern-table";

  const TOOLS = TOOL_IDS;
  const EVENTS: { event: SoundEvent; label: "preferences.aiMode.soundAwaiting" | "preferences.aiMode.soundFinished" }[] =
    [
      { event: "awaiting", label: "preferences.aiMode.soundAwaiting" },
      { event: "finished", label: "preferences.aiMode.soundFinished" },
    ];

  const soundOptions = $derived(
    (["none", ...SOUND_IDS] as SoundChoice[]).map((id) => ({
      value: id,
      label: t(`preferences.aiMode.soundNames.${id}`),
    })),
  );

  /** Picking a sound plays it, so choosing doesn't need a second click. */
  function pickSound(tool: ToolId, event: SoundEvent, choice: SoundChoice) {
    appConfigStore.setAiSound(tool, event, choice);
    playSound(choice, appConfigStore.aiSoundVolume);
  }

  /** What the volume slider plays once let go: the first sound set anywhere. */
  function sampleSound(): SoundChoice {
    for (const tool of TOOLS) {
      if (!appConfigStore.aiSoundsEnabled[tool]) continue;
      for (const { event } of EVENTS) {
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
  {#each TOOLS as tool (tool)}
    {@const enabled = appConfigStore.aiSoundsEnabled[tool]}
    <!-- One group per agent, headed by its icon, so the two sets of sounds
         are told apart at a glance. -->
    <div class="tool-group">
      <div class="tool-header">
        <ConsoleIcon kind={tool} size={14} />
        <span>{TOOL_DISPLAY_NAMES[tool]}</span>
        <label class="tool-toggle">
          <span>{t("preferences.aiMode.soundsOn")}</span>
          <input
            type="checkbox"
            checked={enabled}
            aria-label={t("preferences.aiMode.soundsOnFor", { tool: TOOL_DISPLAY_NAMES[tool] })}
            onchange={(e) => appConfigStore.setAiSoundsEnabled(tool, e.currentTarget.checked)}
          />
        </label>
      </div>
      <!-- Off keeps the picks, just dimmed and out of reach until it's back on. -->
      <div class="tool-sounds" class:off={!enabled} inert={!enabled}>
      {#each EVENTS as { event, label } (event)}
        {@const choice = appConfigStore.aiSounds[tool][event]}
        <div class="sound-row">
          <span>{t(label)}</span>
          <div class="sound-controls">
            <SettingDropdown
              options={soundOptions}
              value={choice}
              onpick={(value) => pickSound(tool, event, value as SoundChoice)}
            />
            <button
              class="preview-btn"
              title={t("preferences.aiMode.soundPreview")}
              aria-label={t("preferences.aiMode.soundPreview")}
              disabled={choice === "none" || appConfigStore.aiSoundVolume === 0}
              onclick={() => playSound(choice, appConfigStore.aiSoundVolume)}
            >
              <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
                <path d="M3 1.5v9l7.5-4.5z" fill="currentColor" />
              </svg>
            </button>
          </div>
        </div>
      {/each}
      </div>
    </div>
  {/each}
  <div class="sound-row volume-row">
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

  /* Each agent's sounds sit on a soft card of their own, which is what
     sets the two sets apart — no rule needed between them. */
  .tool-group {
    margin-bottom: 10px;
    padding: 10px 12px 2px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.025);
  }

  .tool-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 10px;
    font-size: 11px;
    font-weight: 600;
  }

  .tool-toggle {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: normal;
    opacity: 0.85;
    cursor: pointer;
  }

  .tool-toggle input {
    margin: 0;
    accent-color: var(--pane-active-border, #3a6df0);
    cursor: pointer;
  }

  .tool-sounds {
    transition: opacity 120ms ease;
  }

  .tool-sounds.off {
    opacity: 0.4;
  }

  .sound-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 8px;
    padding-left: 20px;
  }

  .volume-row {
    padding-left: 0;
    margin-top: 4px;
  }

  .sound-controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .preview-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    padding: 0;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .preview-btn:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .preview-btn:disabled {
    opacity: 0.35;
    cursor: default;
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
