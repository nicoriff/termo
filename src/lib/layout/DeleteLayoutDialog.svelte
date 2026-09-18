<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { layoutsStore } from "./layouts-store.svelte";

  const target = $derived(uiState.deleteLayoutTarget);

  let confirmEl: HTMLButtonElement | undefined = $state();

  // Focus goes to the confirm button, same reasoning as RestoreLayoutDialog:
  // it takes the keyboard away from the terminal so Enter and Escape are ours.
  $effect(() => {
    confirmEl?.focus();
  });

  function close() {
    uiState.cancelDeleteLayout();
    uiState.refocusTerminals();
  }

  function confirm() {
    const layout = uiState.deleteLayoutTarget;
    uiState.cancelDeleteLayout();
    if (layout) void layoutsStore.remove(layout.name);
    uiState.refocusTerminals();
  }

  function onKeydown(event: KeyboardEvent) {
    // This dialog owns the keyboard while it's up (uiState.overlayOpen keeps
    // the window-level handlers out).
    event.stopPropagation();
    if (event.key === "Escape") {
      close();
    } else if (event.key === "Enter") {
      confirm();
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="dialog-anchor">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-label={t("layouts.deleteTitle")}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">{t("layouts.deleteTitle")}</h2>
    <p class="dialog-text">{t("layouts.deleteWarning", { name: target?.name ?? "" })}</p>
    <div class="dialog-actions">
      <button class="dialog-button" onclick={close}>{t("layouts.cancel")}</button>
      <button class="dialog-button danger" bind:this={confirmEl} onclick={confirm}>
        {t("layouts.deleteConfirm")}
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
     centering lives on this wrapper instead, same as the command palette. */
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

  .dialog-button.danger:focus-visible {
    outline: 2px solid #ff6b6b;
    outline-offset: 1px;
  }
</style>
