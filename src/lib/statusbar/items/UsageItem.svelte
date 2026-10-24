<script lang="ts">
  import { uiState } from "$lib/core/ui-state.svelte";
  import { cpuOfMachine } from "$lib/diagnostics/diagnostics";
  import { t } from "$lib/i18n/i18n.svelte";
  import { getPtyId } from "$lib/terminal/pty-registry";
  import { activeConsole } from "../active.svelte";
  import { formatBytes } from "../format";
  import type { StatusItemId } from "../layout";
  import { usageSampler } from "../usage-sampler.svelte";

  /** CPU and memory of the console, of Termo or of the machine; a click
   * opens the diagnostics panel, which has the history. */
  let { id }: { id: StatusItemId } = $props();

  $effect(() => usageSampler.hold());

  const report = $derived(usageSampler.latest);

  const text = $derived.by(() => {
    if (!report) return null;
    switch (id) {
      case "console.cpu": {
        const ptyId = activeConsole.tabId ? getPtyId(activeConsole.tabId) : undefined;
        const usage = ptyId ? report.consoles.find((console) => console.id === ptyId) : undefined;
        return usage ? t("statusBar.cpu", { value: `${Math.round(cpuOfMachine(usage.cpu, report.cores))}%` }) : null;
      }
      case "system.appMemory":
        return t("statusBar.appMemory", { value: formatBytes(report.app.memory) });
      case "system.cpu":
        return t("statusBar.systemCpu", { value: `${Math.round(report.system.cpu)}%` });
      case "system.memory":
        return t("statusBar.systemMemory", {
          value: `${formatBytes(report.system.memoryUsed)} / ${formatBytes(report.system.memoryTotal)}`,
        });
      default:
        return null;
    }
  });

  const HINTS: Partial<Record<StatusItemId, Parameters<typeof t>[0]>> = {
    "console.cpu": "statusBar.items.consoleCpuHint",
    "system.appMemory": "statusBar.items.appMemoryHint",
    "system.cpu": "statusBar.items.systemCpuHint",
    "system.memory": "statusBar.items.systemMemoryHint",
  };
</script>

{#if text}
  <button class="status-item" title={t(HINTS[id] ?? "statusBar.items.systemCpuHint")} onclick={() => uiState.toggleDiagnostics()}>
    {text}
  </button>
{/if}
