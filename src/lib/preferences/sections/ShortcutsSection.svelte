<script lang="ts">
  import ShortcutRecorder from "../ShortcutRecorder.svelte";
  import { SHORTCUT_LABEL_KEYS } from "$lib/core/shortcuts";
  import { t } from "$lib/i18n/i18n.svelte";
</script>

<section>
  <h3>{t("preferences.shortcuts.title")}</h3>
  <div class="shortcut-row">
    <span>{t(SHORTCUT_LABEL_KEYS.newTab)}</span>
    <ShortcutRecorder action="newTab" />
  </div>
  <div class="shortcut-row">
    <span>{t(SHORTCUT_LABEL_KEYS.cycleTab)}</span>
    <ShortcutRecorder action="cycleTab" />
  </div>
  <div class="shortcut-row">
    <span>{t(SHORTCUT_LABEL_KEYS.zoomTile)}</span>
    <ShortcutRecorder action="zoomTile" mode="zoom" />
  </div>
  {#each ["aiRunning", "aiAwaiting", "aiError", "aiFinished", "aiNextPending"] as const as action (action)}
    <div class="shortcut-row">
      <span>{t(SHORTCUT_LABEL_KEYS[action])}</span>
      <ShortcutRecorder {action} />
    </div>
  {/each}
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

  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    padding: 8px 0;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .shortcut-row:last-child {
    border-bottom: none;
  }
</style>
