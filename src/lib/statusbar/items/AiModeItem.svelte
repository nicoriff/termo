<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
</script>

<span class="status-item">
  {t("statusBar.aiMode", { state: t(appConfigStore.aiModeEnabled ? "statusBar.aiOn" : "statusBar.aiOff") })}
</span>
