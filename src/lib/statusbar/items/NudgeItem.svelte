<script lang="ts">
  import { nudgeTitle } from "$lib/companion/companion";
  import { companionNudges } from "$lib/companion/companion-nudges.svelte";
  import { companionStore } from "$lib/companion/companion-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { activeConsole } from "../active.svelte";

  /** What the Companion has to say about the active console's session. */
  const nudge = $derived(companionNudges.for(activeConsole.tabId));
  const nudgeText = $derived(
    !nudge
      ? ""
      : nudge.kind === "cacheExpiring"
        ? t("companion.nudge.cacheExpiring", { minutes: String(nudge.minutes ?? 1) })
        : t(nudgeTitle(nudge.kind)),
  );

  /** Looked at: the panel says the rest, and the notice doesn't come back. */
  function openNudge() {
    if (!nudge) return;
    companionNudges.markSeen(nudge);
    companionStore.picked = null;
    uiState.openCompanion();
  }
</script>

{#if nudge}
  <button class="status-item" title={t("companion.nudge.hint")} onclick={openNudge}>
    <span class="status-dot" style="background:#f5a623" aria-hidden="true"></span>{nudgeText}
  </button>
{/if}
