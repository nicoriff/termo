<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { tabAgentSessions } from "$lib/ai-mode/tab-sessions.svelte";
  import { layoutsStore } from "./layouts-store.svelte";

  let name = $state("");
  let saveSessions = $state(true);
  let inputEl: HTMLInputElement | undefined = $state();

  /** Only worth asking when there is something to save: Modo IA on, and at
   * least one console whose conversation Termini can identify (which needs the
   * agent hooks, or Claude's status line). */
  const canSaveSessions = $derived(appConfigStore.aiModeEnabled && tabAgentSessions.count > 0);

  $effect(() => {
    inputEl?.focus();
  });

  const trimmed = $derived(name.trim());
  /** Saving over an existing name replaces that layout — worth saying so
   * before the user hits Enter. */
  const overwrites = $derived(
    layoutsStore.layouts.some((layout) => layout.name.toLowerCase() === trimmed.toLowerCase()),
  );

  function close() {
    uiState.closeSaveLayoutPrompt();
    // The input took focus away from the terminal; hand it back.
    uiState.refocusTerminals();
  }

  function confirm() {
    if (!trimmed) return;
    void layoutsStore.save(trimmed, canSaveSessions && saveSessions);
    close();
  }

  function onKeydown(event: KeyboardEvent) {
    // This dialog owns the keyboard while it's open (MainWindow's and the AI
    // panel's window-level handlers bail out), so nothing reaches the terminal.
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
    aria-label={t("layouts.nameTitle")}
    onpointerdown={(e) => e.stopPropagation()}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">{t("layouts.nameTitle")}</h2>
    <input
      bind:this={inputEl}
      bind:value={name}
      class="dialog-input"
      placeholder={t("layouts.namePlaceholder")}
      aria-label={t("layouts.namePlaceholder")}
      onkeydown={onKeydown}
    />
    <p class="dialog-hint">{overwrites ? t("layouts.nameTaken") : t("layouts.nameHint")}</p>
    {#if canSaveSessions}
      <label class="dialog-check">
        <input type="checkbox" bind:checked={saveSessions} onkeydown={onKeydown} />
        <span>
          {t("layouts.saveSessions", { count: String(tabAgentSessions.count) })}
          <small>{t("layouts.saveSessionsHint")}</small>
        </span>
      </label>
    {/if}
    <div class="dialog-actions">
      <button class="dialog-button" onclick={close}>{t("layouts.cancel")}</button>
      <button class="dialog-button primary" disabled={trimmed === ""} onclick={confirm}>
        {t("layouts.confirm")}
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

  .dialog-title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .dialog-input {
    padding: 8px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: var(--tabbar-bg, #181818);
    color: inherit;
    font: inherit;
    font-size: 13px;
  }

  .dialog-input:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .dialog-hint {
    margin: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .dialog-check {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 12px;
    cursor: pointer;
  }

  .dialog-check input {
    margin: 2px 0 0;
    flex-shrink: 0;
  }

  .dialog-check small {
    display: block;
    margin-top: 2px;
    font-size: 11px;
    opacity: 0.6;
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

  .dialog-button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .dialog-button.primary:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
