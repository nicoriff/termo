<script lang="ts">
  import { openPreferencesWindow } from "$lib/core/window-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { remoteWatch } from "$lib/remote/remote-watch.svelte";
</script>

<!-- Who's connected from the tailnet. -->
{#if remoteWatch.viewers.length > 0}
  <button
    class="status-item"
    title={t("remote.viewersHint", {
      who: remoteWatch.viewers.map((viewer) => `${viewer.login} (${viewer.node})`).join(", "),
    })}
    onclick={() => void openPreferencesWindow("remote")}
  >
    <span class="status-dot" aria-hidden="true"></span>{t("remote.viewers", { count: String(remoteWatch.viewers.length) })}
  </button>
{/if}
