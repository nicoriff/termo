<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { tabSize } from "$lib/terminal/tab-size.svelte";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import { activeConsole } from "../active.svelte";
  import type { StatusItemId } from "../layout";

  let { id }: { id: StatusItemId } = $props();

  const tabId = $derived(activeConsole.console ? activeConsole.tabId : null);
  /** What runs in the console right now: the shell itself, or what it started. */
  const process = $derived(tabId ? (tabTitleStore.byTab[tabId]?.processName ?? null) : null);
  const size = $derived(tabId ? (tabSize.byTab[tabId] ?? null) : null);
</script>

{#if id === "console.process" && process}
  <span class="status-item" title={t("statusBar.items.processHint")}>▸ {process}</span>
{:else if id === "console.size" && size}
  <span class="status-item" title={t("statusBar.items.sizeHint")}>{size.cols}×{size.rows}</span>
{/if}
