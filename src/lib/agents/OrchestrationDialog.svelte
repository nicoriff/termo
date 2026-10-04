<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import { sessionFolder, workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { collectTiles } from "$lib/layout/split-tree";
  import { orchestrationStore } from "./orchestration-store.svelte";
  import { modelsOf } from "./orchestration-spec";
  import FolderField from "./FolderField.svelte";

  let prompt = $state("");
  let workDir = $state("");
  let model = $state("");
  let error = $state("");
  let dialogEl: HTMLDivElement | undefined = $state();

  const planning = $derived(orchestrationStore.planning);
  const claudeInstalled = $derived(newTabOptions.agentClis?.claude !== false);
  const models = $derived(modelsOf(orchestrationStore.catalog, "claude"));

  /** The folder the focused console is in: most likely the project. */
  const hereFolder = $derived.by(() => {
    const root = workspaceStore.root;
    if (!root) return null;
    const tile = collectTiles(root).find((candidate) => candidate.id === workspaceStore.focusedTileId);
    const active = tile?.sessions.find((session) => session.id === tile.activeSessionId);
    return active ? (sessionFolder(active) ?? null) : null;
  });

  onMount(() => {
    newTabOptions.refresh();
    void orchestrationStore.loadCatalog();
    if (hereFolder) workDir = hereFolder;
    void tick().then(() => dialogEl?.querySelector<HTMLElement>("textarea")?.focus());
  });

  // Planning carries on with the dialog closed: its draft shows up in the
  // panel when it's ready.
  function close() {
    uiState.closeOrchestrationDialog();
  }

  async function plan() {
    if (!prompt.trim() || !workDir.trim() || planning || !claudeInstalled) return;
    error = "";
    try {
      const orchestration = await orchestrationStore.plan(prompt.trim(), workDir.trim(), model);
      workspaceStore.openOrchestration(orchestration.id, orchestration.name);
      if (uiState.orchestrationDialog) close();
    } catch (reason) {
      error = String(reason);
    }
  }

  function onKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void plan();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-backdrop" onpointerdown={close} transition:fade={{ duration: 140 }}></div>
<div class="dialog-anchor">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={t("agents.orchestration.dialog.title")}
    tabindex="-1"
    bind:this={dialogEl}
    transition:fly={{ y: -16, duration: 180, easing: cubicOut }}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
  >
    <header class="dialog-header">
      <span class="dialog-title">{t("agents.orchestration.dialog.title")}</span>
      <button class="close-button" aria-label={t("agents.orchestration.dialog.close")} onclick={close}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    <div class="dialog-body">
      <label class="label first" for="orchestration-prompt">{t("agents.orchestration.dialog.promptLabel")}</label>
      <textarea
        id="orchestration-prompt"
        class="field"
        rows="6"
        placeholder={t("agents.orchestration.dialog.promptPlaceholder")}
        bind:value={prompt}
        disabled={planning}
      ></textarea>

      <label class="label" for="orchestration-folder">{t("agents.orchestration.dialog.folderLabel")}</label>
      <div class="row">
        <FolderField id="orchestration-folder" bind:value={workDir} disabled={planning} />
        {#if hereFolder && hereFolder !== workDir}
          <button class="button small" title={hereFolder} disabled={planning} onclick={() => (workDir = hereFolder!)}>
            {t("agents.wizard.engine.useHere")}
          </button>
        {/if}
      </div>
      <p class="hint">{t("agents.orchestration.dialog.folderHint")}</p>

      <label class="label" for="orchestration-model">{t("agents.orchestration.dialog.modelLabel")}</label>
      <select id="orchestration-model" class="field narrow" bind:value={model} disabled={planning}>
        <option value="">{t("agents.orchestration.dialog.modelDefault")}</option>
        {#each models as option (option.id)}
          <option value={option.id}>{option.name}</option>
        {/each}
      </select>
      <p class="hint">{t("agents.orchestration.dialog.modelHint")}</p>
    </div>

    <footer class="dialog-footer">
      {#if error}
        <span class="error">{error}</span>
      {:else}
        <span class="footer-hint" class:working={planning} aria-live="polite">
          {planning
            ? t("agents.orchestration.dialog.planning")
            : claudeInstalled
              ? t("agents.orchestration.dialog.note")
              : t("agents.wizard.idea.aiNeedsClaude")}
        </span>
      {/if}
      <button class="button" onclick={close}>
        {planning ? t("agents.orchestration.dialog.close") : t("agents.orchestration.dialog.cancel")}
      </button>
      <button class="button primary" disabled={planning || !prompt.trim() || !workDir.trim() || !claudeInstalled} onclick={() => void plan()}>
        {t("agents.orchestration.dialog.analyze")}
      </button>
    </footer>
  </div>
</div>

<style>
  .dialog-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.55);
  }

  .dialog-anchor {
    position: absolute;
    inset: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    pointer-events: none;
  }

  .dialog {
    pointer-events: auto;
    display: flex;
    flex-direction: column;
    width: min(640px, 94%);
    max-height: 88%;
    margin-top: 7vh;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 10px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
    font-size: 12.5px;
    outline: none;
    overflow: hidden;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px 10px 18px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .dialog-title {
    flex: 1;
    font-weight: 600;
    font-size: 14px;
  }

  .close-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    opacity: 0.7;
    cursor: pointer;
  }

  .close-button svg {
    width: 11px;
    height: 11px;
  }

  .close-button:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .dialog-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 22px 18px;
    scrollbar-width: thin;
  }

  .label {
    display: block;
    margin: 14px 0 5px;
    font-weight: 600;
  }

  .label.first {
    margin-top: 0;
  }

  .hint {
    margin: 5px 0 0;
    font-size: 11.5px;
    line-height: 1.5;
    opacity: 0.6;
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    padding: 7px 9px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
    color: inherit;
    font: inherit;
    resize: vertical;
    outline: none;
  }

  .field:focus {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .field.narrow {
    width: min(320px, 100%);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dialog-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 18px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .footer-hint,
  .error {
    flex: 1;
    min-width: 0;
    font-size: 11.5px;
    line-height: 1.45;
  }

  .footer-hint {
    opacity: 0.6;
  }

  .footer-hint.working {
    opacity: 1;
    animation: pulse 1.4s ease-in-out infinite;
  }

  .error {
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }

  .button {
    padding: 5px 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
    white-space: nowrap;
  }

  .button.small {
    padding: 3px 10px;
    font-size: 11.5px;
  }

  .button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .button.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
