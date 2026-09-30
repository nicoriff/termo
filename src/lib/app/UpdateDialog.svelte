<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { getVersion } from "@tauri-apps/api/app";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { renderMarkdownLite } from "$lib/agents/markdown-lite";
  import { notesFor } from "./release-notes";
  import { updater } from "./updater.svelte";

  let version = $state<string | null>(null);
  let dialogEl: HTMLDivElement | undefined = $state();

  const status = $derived(updater.status);
  const notes = $derived(updater.found ? notesFor(updater.found.body, i18n.locale) : "");
  const percent = $derived(
    status.kind === "downloading" && status.total ? Math.min(100, Math.round((status.received / status.total) * 100)) : null,
  );

  onMount(() => {
    void getVersion().then((value) => (version = value));
    dialogEl?.focus();
    // Opening the dialog is the check: nothing to press twice. One still
    // under way from before (closed mid-download) just shows its progress.
    if (!updater.working) {
      updater.reset();
      void updater.checkAndInstall();
    }
  });

  function close() {
    uiState.closeUpdates();
    // A download under way carries on (and restarts Termo) without the dialog.
    updater.reset();
    uiState.refocusTerminals();
  }

  function onKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  }

  function megabytes(bytes: number): string {
    return (bytes / 1024 / 1024).toFixed(1);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="dialog-anchor">
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-label={t("updates.title")}
    bind:this={dialogEl}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">{t("updates.title")}</h2>

    {#if status.kind === "checking" || status.kind === "idle"}
      <p class="line" aria-live="polite">{t("updates.checking")}</p>
    {:else if status.kind === "upToDate"}
      <p class="line" aria-live="polite">{t("updates.upToDate", { version: version ?? "" })}</p>
    {:else if status.kind === "error" && !updater.found}
      <p class="line error" aria-live="polite">{t("updates.failed", { error: status.message })}</p>
    {/if}

    {#if updater.found}
      <p class="found">{t("updates.found", { version: updater.found.version })}</p>
      {#if notes}
        <div class="notes">{@html renderMarkdownLite(notes)}</div>
      {/if}

      {#if status.kind === "waiting"}
        <ul class="busy">
          {#if status.consoles > 0}
            <li>{t("quit.busyConsoles", { count: String(status.consoles) })}</li>
          {/if}
          {#if status.agents > 0}
            <li>{t("updates.busyAgents", { count: String(status.agents) })}</li>
          {/if}
        </ul>
        <p class="line warning">{t("updates.busyWarning")}</p>
      {:else if status.kind === "downloading"}
        <div
          class="progress"
          role="progressbar"
          aria-label={t("updates.downloading")}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent ?? undefined}
        >
          <div class="bar" class:unknown={percent === null} style:width={percent === null ? "100%" : `${percent}%`}></div>
        </div>
        <p class="line" aria-live="polite">
          {percent === null
            ? t("updates.downloadingSize", { size: megabytes(status.received) })
            : t("updates.downloadingPercent", { percent: String(percent) })}
        </p>
      {:else if status.kind === "installing"}
        <p class="line" aria-live="polite">{t("updates.installing")}</p>
      {:else if status.kind === "error"}
        <p class="line error" aria-live="polite">{t("updates.failed", { error: status.message })}</p>
      {/if}
    {/if}

    <div class="dialog-actions">
      {#if status.kind === "waiting"}
        <button class="dialog-button" onclick={close}>{t("updates.later")}</button>
        <button class="dialog-button primary" onclick={() => void updater.install()}>{t("updates.installAnyway")}</button>
      {:else if status.kind === "error"}
        <button class="dialog-button" onclick={close}>{t("updates.close")}</button>
        <button
          class="dialog-button primary"
          onclick={() => {
            updater.reset();
            void updater.checkAndInstall();
          }}>{t("updates.retry")}</button
        >
      {:else}
        <button class="dialog-button primary" onclick={close}>{t("updates.close")}</button>
      {/if}
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
     centering lives on this wrapper instead, same as About. */
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
    width: min(440px, 92%);
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

  .line {
    margin: 12px 0 0;
    font-size: 12px;
    line-height: 1.5;
    opacity: 0.85;
    overflow-wrap: anywhere;
  }

  .line.error {
    color: var(--ansi-red, #f48771);
    opacity: 1;
  }

  .line.warning {
    opacity: 1;
  }

  .found {
    margin: 12px 0 0;
    font-size: 13px;
    font-weight: 600;
  }

  .notes {
    margin-top: 8px;
    max-height: 40vh;
    overflow-y: auto;
    padding: 10px 12px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    font-size: 12px;
    line-height: 1.55;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .busy {
    margin: 12px 0 0;
    padding-left: 18px;
    font-size: 12px;
    line-height: 1.6;
  }

  .progress {
    margin-top: 14px;
    height: 6px;
    border-radius: 3px;
    background: var(--tabbar-border, #2a2a2a);
    overflow: hidden;
  }

  .bar {
    height: 100%;
    background: var(--pane-active-border, #3a6df0);
    transition: width 120ms linear;
  }

  /* No size from the server: a bar that breathes instead of one that lies. */
  .bar.unknown {
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bar,
    .bar.unknown {
      transition: none;
      animation: none;
    }
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
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

  .dialog-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .dialog-button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }
</style>
