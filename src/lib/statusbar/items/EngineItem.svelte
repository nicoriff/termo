<script lang="ts">
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { t } from "$lib/i18n/i18n.svelte";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import { activeConsole } from "../active.svelte";

  const tool = $derived(activeConsole.tool);
</script>

{#if activeConsole.console}
  <!-- The console's type: the tab shows it only as an icon. -->
  <span class="status-item" title={tool ? t("statusBar.engineHint") : t("statusBar.consoleHint")}>
    <ConsoleIcon kind={tabTitleStore.kindFor(activeConsole.console)} size={11} />
    {tool ? TOOL_DISPLAY_NAMES[tool] : tabTitleStore.typeFor(activeConsole.console)}
  </span>
{/if}
