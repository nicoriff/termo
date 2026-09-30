<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { renderMarkdownLite } from "$lib/agents/markdown-lite";
  import { asList } from "./release-notes";

  const entries = $derived(uiState.whatsNew ?? []);
  let dialogEl: HTMLDivElement | undefined = $state();

  onMount(() => dialogEl?.focus());

  function close() {
    uiState.closeWhatsNew();
    uiState.refocusTerminals();
  }

  function onKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape" || event.key === "Enter") {
      event.preventDefault();
      close();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="dialog-anchor">
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-label={t("updates.whatsNewTitle")}
    bind:this={dialogEl}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">{t("updates.whatsNewTitle")}</h2>
    <div class="entries">
      {#each entries as entry (entry.version)}
        <section class="entry">
          <h3 class="entry-title">
            {t("updates.entryTitle", { version: entry.version })}
            <span class="entry-date">{entry.date}</span>
          </h3>
          <div class="entry-items">{@html renderMarkdownLite(asList(entry[i18n.locale]))}</div>
        </section>
      {/each}
    </div>
    <div class="dialog-actions">
      <button class="dialog-button primary" onclick={close}>{t("updates.done")}</button>
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
    width: min(480px, 92%);
    margin-top: 10vh;
    display: flex;
    flex-direction: column;
    padding: 20px 20px 16px;
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
    font-size: 15px;
    font-weight: 600;
  }

  .entries {
    margin-top: 10px;
    max-height: 55vh;
    overflow-y: auto;
  }

  .entry + .entry {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .entry-title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .entry-date {
    font-size: 11px;
    font-weight: 400;
    opacity: 0.6;
    font-variant-numeric: tabular-nums;
  }

  .entry-items {
    margin-top: 6px;
    font-size: 12px;
    line-height: 1.55;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 18px;
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

  .dialog-button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }
</style>
