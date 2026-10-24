<script lang="ts">
  import { homeDir } from "@tauri-apps/api/path";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { gitStore } from "$lib/git/git-store.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { activeConsole } from "../active.svelte";
  import { formatPath, type PathFormat } from "../format";
  import { optionValue, type StatusItemRef } from "../layout";

  let { item }: { item: StatusItemRef } = $props();

  let home = $state<string | null>(null);
  void homeDir()
    .then((dir) => (home = dir))
    .catch(() => {});

  const folder = $derived(activeConsole.folder);
  const format = $derived(optionValue(item, "format") as PathFormat);
  const shown = $derived(folder ? formatPath(folder, format, home, gitStore.snapshot?.root ?? null) : "");

  let copied = $state(false);
  let copiedTimer: ReturnType<typeof setTimeout> | null = null;

  /** A click copies the whole path, whatever part of it shows. */
  async function copy() {
    if (!folder) return;
    try {
      await writeText(folder);
    } catch {
      return;
    }
    copied = true;
    if (copiedTimer) clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied = false), 1200);
  }

  $effect(() => () => {
    if (copiedTimer) clearTimeout(copiedTimer);
  });
</script>

{#if folder}
  <button class="status-item status-path" title={`${folder}\n${t("statusBar.copyPathHint")}`} onclick={() => void copy()}>
    <span class="path-text">{copied ? t("statusBar.copied") : shown}</span>
  </button>
{/if}
