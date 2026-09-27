<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { fileTabs } from "./file-tabs.svelte";

  const target = $derived(uiState.closeFileTarget);

  let saveEl: HTMLButtonElement | undefined = $state();

  // Save is where focus lands: Enter keeps the work, Escape changes nothing.
  $effect(() => {
    saveEl?.focus();
  });

  function cancel() {
    uiState.cancelCloseFile();
    uiState.refocusTerminals();
  }

  function discard() {
    const current = uiState.closeFileTarget;
    uiState.cancelCloseFile();
    if (current) workspaceStore.closeTab(current.tileId, current.sessionId);
  }

  /** Closes only once it's really saved; a failed save leaves the tab open,
   * with the reason shown in its editor. */
  async function saveAndClose() {
    const current = uiState.closeFileTarget;
    uiState.cancelCloseFile();
    if (current && (await fileTabs.save(current.sessionId))) {
      workspaceStore.closeTab(current.tileId, current.sessionId);
    }
  }

  function onKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape") cancel();
    else if (event.key === "Enter" && document.activeElement === saveEl) void saveAndClose();
    else return;
    event.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-backdrop" onpointerdown={cancel} transition:fade={{ duration: 120 }}></div>

<div class="dialog-anchor">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-label={t("editor.closeTitle", { name: target?.name ?? "" })}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">{t("editor.closeTitle", { name: target?.name ?? "" })}</h2>
    <p class="dialog-text">{t("editor.closeHint")}</p>
    <div class="dialog-actions">
      <button class="dialog-button" onclick={cancel}>{t("editor.cancel")}</button>
      <button class="dialog-button danger" onclick={discard}>{t("editor.discard")}</button>
      <button class="dialog-button primary" bind:this={saveEl} onclick={() => void saveAndClose()}>
        {t("editor.save")}
      </button>
    </div>
  </div>
</div>

<style>
  .dialog-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.35);
  }

  /* The panel carries the fly transition, which animates transform — so the
     centering lives on this wrapper, same as the other dialogs. */
  .dialog-anchor {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }

  .dialog {
    pointer-events: auto;
    width: min(420px, 90%);
    margin-top: 12vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
    color: var(--titlebar-fg, #d4d4d4);
  }

  .dialog:focus {
    outline: none;
  }

  .dialog-title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .dialog-text {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    opacity: 0.85;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .dialog-button {
    padding: 6px 14px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .dialog-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .dialog-button.danger {
    color: #ff6b6b;
    border-color: rgba(255, 107, 107, 0.4);
  }

  .dialog-button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .dialog-button.primary:focus-visible {
    outline: 2px solid #fff;
    outline-offset: 1px;
  }
</style>
