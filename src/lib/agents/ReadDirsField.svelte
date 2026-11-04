<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";

  /** An orchestration's read-only folders, as chips: each with its remove
   * button, and one more added from the folder picker. */
  let { dirs, disabled = false, onchange }: { dirs: string[]; disabled?: boolean; onchange: (next: string[]) => void } =
    $props();

  function add() {
    uiState.pickFolder(null, (picked) => {
      if (dirs.some((dir) => dir.toLowerCase() === picked.toLowerCase())) return;
      onchange([...dirs, picked]);
    });
  }
</script>

<div class="read-dir-list">
  {#each dirs as dir (dir)}
    <span class="read-dir" title={dir}>
      <span class="read-dir-path">{dir}</span>
      <button
        class="read-dir-remove"
        {disabled}
        title={t("agents.orchestration.readDirs.remove")}
        aria-label={t("agents.orchestration.readDirs.remove")}
        onclick={() => onchange(dirs.filter((other) => other !== dir))}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2 2l6 6M8 2L2 8" stroke="currentColor" stroke-width="1.2" /></svg>
      </button>
    </span>
  {/each}
  <button class="read-dir-add" {disabled} onclick={add}>+ {t("agents.orchestration.readDirs.add")}</button>
</div>

<style>
  .read-dir-list {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  /* A folder as a chip: the path, cut from the start when it's long (its
     end says which one it is), and its remove button. */
  .read-dir {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    max-width: 340px;
    height: 24px;
    padding: 0 2px 0 8px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 12px;
    font-size: 11.5px;
  }

  .read-dir-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }

  .read-dir-remove {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: inherit;
    opacity: 0.6;
    cursor: pointer;
  }

  .read-dir-remove svg {
    width: 9px;
    height: 9px;
  }

  .read-dir-remove:hover:not(:disabled) {
    opacity: 1;
    background: var(--tab-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .read-dir-add {
    height: 24px;
    padding: 0 10px;
    border: 1px dashed var(--tabbar-border, #3a3a3a);
    border-radius: 12px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .read-dir-add:hover:not(:disabled) {
    border-style: solid;
    background: var(--tab-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .read-dir-add:disabled,
  .read-dir-remove:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
