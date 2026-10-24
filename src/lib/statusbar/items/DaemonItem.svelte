<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { sessionsDaemon } from "$lib/core/sessions-daemon.svelte";
  import { openPreferencesWindow } from "$lib/core/window-client";
  import { t } from "$lib/i18n/i18n.svelte";
</script>

{#if !sessionsDaemon.answering && appConfigStore.persistentSessions}
  <button class="status-item status-daemon" title={t("statusBar.daemonDownHint")} onclick={() => void openPreferencesWindow("console")}>
    <span class="status-dot" style="background:#e3b341" aria-hidden="true"></span>{t("statusBar.daemonDown")}
  </button>
{/if}

<style>
  .status-daemon {
    color: #e3b341;
  }
</style>
