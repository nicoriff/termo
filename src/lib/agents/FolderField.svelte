<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";

  /** A folder path: typed, or chosen with the folder browser new consoles
   * use (AgentLaunchDialog). */
  let {
    value = $bindable(""),
    id,
    placeholder = "",
    disabled = false,
  }: { value?: string; id?: string; placeholder?: string; disabled?: boolean } = $props();

  function choose() {
    uiState.pickFolder(value.trim() || null, (picked) => (value = picked));
  }
</script>

<div class="folder-field">
  <input {id} class="field" {placeholder} {disabled} bind:value spellcheck="false" />
  <button type="button" class="choose" {disabled} title={t("agents.folder.title")} onclick={choose}>
    <svg viewBox="0 0 14 14" aria-hidden="true">
      <path d="M1.5 3.5h4l1.5 1.5h5.5v6.5h-11z" fill="none" stroke="currentColor" stroke-width="1.1" />
    </svg>
    {t("agents.folder.choose")}
  </button>
</div>

<style>
  .folder-field {
    display: flex;
    flex: 1;
    min-width: 0;
    gap: 6px;
  }

  .field {
    box-sizing: border-box;
    flex: 1;
    min-width: 0;
    padding: 7px 9px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
    color: inherit;
    font: inherit;
    outline: none;
  }

  .field:focus {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .choose {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .choose:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .choose:disabled,
  .field:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .choose svg {
    width: 13px;
    height: 13px;
  }
</style>
