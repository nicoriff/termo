<script lang="ts">
  import { getPtyMemory } from "$lib/core/pty-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { getPtyId } from "$lib/terminal/pty-registry";
  import { activeConsole } from "../active.svelte";
  import { formatBytes } from "../format";

  const MEMORY_POLL_MS = 2000;

  let memoryBytes = $state<number | null>(null);

  $effect(() => {
    const tabId = activeConsole.tabId;
    memoryBytes = null;
    if (!tabId) return;

    let cancelled = false;
    async function sample(tabId: string) {
      // No PTY id yet while the tab's shell is still spawning — the next
      // poll picks it up.
      const ptyId = getPtyId(tabId);
      if (!ptyId) return;
      try {
        const bytes = await getPtyMemory(ptyId);
        if (!cancelled) memoryBytes = bytes;
      } catch {
        if (!cancelled) memoryBytes = null;
      }
    }

    void sample(tabId);
    const timer = setInterval(() => void sample(tabId), MEMORY_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  });
</script>

{#if memoryBytes !== null}
  <span class="status-item" title={t("statusBar.memoryHint")}>
    {t("statusBar.memory", { value: formatBytes(memoryBytes) })}
  </span>
{/if}
