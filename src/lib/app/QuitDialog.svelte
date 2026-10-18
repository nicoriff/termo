<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { closeWindow, quitApp } from "$lib/core/window-client";
  import { t } from "$lib/i18n/i18n.svelte";

  const counts = $derived(uiState.quitPrompt);
  /** Closing one window (its X) rather than quitting Termo. */
  const closingWindow = $derived(counts?.scope === "window");
  /** Work closing ends, and work it leaves running in the background ("Keep
   * consoles running"): only the first is a warning. */
  const losing = $derived(!!counts && counts.consoles + counts.agents > 0);
  const keeping = $derived(!!counts && counts.keptConsoles + counts.keptAgents > 0);

  let cancelEl: HTMLButtonElement | undefined = $state();
  let confirmEl: HTMLButtonElement | undefined = $state();

  // Focus starts on Cancel, not on Quit, when work would be lost: that's
  // irreversible, so a reflex Enter must not be the one that kills the
  // consoles. With nothing lost, Quit is the likely answer.
  $effect(() => {
    (losing ? cancelEl : confirmEl)?.focus();
  });

  function close() {
    uiState.cancelQuit();
    // The dialog took focus away from the terminal; hand it back.
    uiState.refocusTerminals();
  }

  function confirm() {
    const window = closingWindow;
    uiState.cancelQuit();
    void (window ? closeWindow() : quitApp());
  }

  function onKeydown(event: KeyboardEvent) {
    // This dialog owns the keyboard while it's up (uiState.overlayOpen keeps
    // the window-level handlers out). Enter is left alone: it activates
    // whatever button has focus, which starts as Cancel.
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
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
    aria-label={t(closingWindow ? "quit.windowTitle" : "quit.title")}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">{t(closingWindow ? "quit.windowTitle" : "quit.title")}</h2>
    {#if counts && losing}
      <ul class="dialog-list">
        {#if counts.consoles > 0}
          <li>{t("quit.busyConsoles", { count: String(counts.consoles) })}</li>
        {/if}
        {#if counts.agents > 0}
          <li>{t("quit.busyAgents", { count: String(counts.agents) })}</li>
        {/if}
      </ul>
      <p class="dialog-text">
        {#if keeping}
          {t(closingWindow ? "quit.windowWarningSome" : "quit.warningSome")}
        {:else}
          {t(closingWindow ? "quit.windowWarning" : "quit.warning")}
        {/if}
      </p>
    {/if}
    {#if counts && keeping}
      <div class="dialog-info">
        <svg class="info-icon" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" stroke-width="1.2" />
          <path d="M8 7.2v4M8 4.8v.1" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
        </svg>
        <div class="info-body">
          <p class="info-title">{t("quit.keptTitle")}</p>
          <ul class="dialog-list">
            {#if counts.keptConsoles > 0}
              <li>{t("quit.busyConsoles", { count: String(counts.keptConsoles) })}</li>
            {/if}
            {#if counts.keptAgents > 0}
              <li>{t("quit.busyAgents", { count: String(counts.keptAgents) })}</li>
            {/if}
          </ul>
          <p class="dialog-text">{t(closingWindow ? "quit.windowKeptInfo" : "quit.keptInfo")}</p>
        </div>
      </div>
    {/if}
    <div class="dialog-actions">
      <button class="dialog-button" bind:this={cancelEl} onclick={close}>{t("quit.cancel")}</button>
      <button class="dialog-button" class:danger={losing} class:primary={!losing} bind:this={confirmEl} onclick={confirm}>
        {#if losing}
          {t(closingWindow ? "quit.windowConfirm" : "quit.confirm")}
        {:else}
          {t(closingWindow ? "quit.windowClosePlain" : "quit.quitPlain")}
        {/if}
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

  .dialog-list {
    margin: 0;
    padding-left: 18px;
    font-size: 12px;
    line-height: 1.6;
  }

  .dialog-text {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    opacity: 0.85;
  }

  /* What keeps running: said as information, not as a warning. */
  .dialog-info {
    display: flex;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 6px;
    border: 1px solid color-mix(in srgb, var(--pane-active-border, #3a6df0) 35%, transparent);
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 10%, transparent);
  }

  .info-icon {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    margin-top: 1px;
    color: var(--pane-active-border, #3a6df0);
  }

  .info-body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .info-title {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
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

  .dialog-button:focus-visible {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .dialog-button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .dialog-button.primary:hover {
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 85%, #fff);
  }

  .dialog-button.danger {
    color: #ff6b6b;
    border-color: rgba(255, 107, 107, 0.4);
  }

  .dialog-button.danger:hover {
    background: var(--titlebar-close-hover-bg, #e81123);
    border-color: var(--titlebar-close-hover-bg, #e81123);
    color: #fff;
  }
</style>
