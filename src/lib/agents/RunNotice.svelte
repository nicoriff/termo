<script lang="ts">
  import type { RunEvent } from "$lib/core/agents-client";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { brokenServers, sessionParts, usageText } from "./run-diagnosis";

  /** A line about the run rather than its work: what ran it (the CLI's own
   * report when it starts) or a usage-limit warning. */
  let { event }: { event: RunEvent } = $props();

  const broken = $derived(event.session ? brokenServers(event.session) : []);
</script>

{#if event.kind === "session" && event.session}
  <div class="notice session">
    <span class="parts">{sessionParts(event.session, t).join(" · ")}</span>
    {#if broken.length > 0}
      <span class="warn">{t("agents.diagnosis.mcpFailed", { servers: broken.join(", ") })}</span>
    {/if}
  </div>
{:else if event.kind === "usage" && event.usage}
  <div class="notice usage" class:rejected={event.usage.status === "rejected"}>
    ⚠ {usageText(event.usage, t, i18n.locale)}
  </div>
{/if}

<style>
  .notice {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    font-size: 11px;
    line-height: 1.45;
  }

  .session .parts {
    opacity: 0.55;
  }

  .warn {
    color: var(--warning-fg, #d7ba7d);
  }

  .usage {
    padding: 3px 8px;
    border-left: 3px solid var(--warning-fg, #d7ba7d);
    border-radius: 3px;
    background: rgba(215, 186, 125, 0.1);
  }

  .usage.rejected {
    border-color: var(--error-fg, #f48771);
    background: rgba(244, 135, 113, 0.1);
  }
</style>
