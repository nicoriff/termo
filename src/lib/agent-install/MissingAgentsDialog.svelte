<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import type { ToolId } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { cancelAgentCliInstall, installAgentCli, onAgentInstallProgress } from "$lib/core/agent-install-client";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import { missingAgents } from "./missing-agents-store.svelte";

  /**
   * Asks about the agents' CLIs a change of Node.js left behind, and puts
   * them back one after another with npm, into its own folder: where they
   * were, or they'd not have gone with the Node.js they were under.
   */
  const tools = $derived(uiState.missingAgents ?? []);

  type State = "waiting" | "installing" | "done" | "failed";
  const STATE_KEYS: Record<State, MessageKey> = {
    waiting: "agentInstall.missing.waiting",
    installing: "agentInstall.missing.installing",
    done: "agentInstall.missing.done",
    failed: "agentInstall.missing.failed",
  };

  type Screen = "ask" | "installing" | "result";
  let screen = $state<Screen>("ask");
  let states = $state<Partial<Record<ToolId, State>>>({});
  let errors = $state<Partial<Record<ToolId, string>>>({});
  /** What npm said last about the one being installed. */
  let line = $state("");
  let cancelled = false;
  let dialogEl: HTMLDivElement | undefined = $state();
  let reinstallEl: HTMLButtonElement | undefined = $state();

  /** The ones still not back, to try again. */
  const left = $derived(tools.filter((tool) => states[tool] !== "done"));

  onMount(() => {
    (reinstallEl ?? dialogEl)?.focus();
    const stop = onAgentInstallProgress((progress) => {
      if (progress.line) line = progress.line;
    });
    return () => void stop.then((unlisten) => unlisten());
  });

  async function reinstall() {
    screen = "installing";
    cancelled = false;
    const queue = left;
    errors = {};
    for (const tool of queue) states[tool] = "waiting";
    for (const tool of queue) {
      if (cancelled) break;
      states[tool] = "installing";
      line = "";
      try {
        await installAgentCli(tool, null);
        states[tool] = "done";
      } catch (err) {
        states[tool] = "failed";
        errors[tool] = err instanceof Error ? err.message : String(err);
      }
    }
    // The menus that list the agents, and what's remembered of them.
    newTabOptions.refresh();
    await missingAgents.settle();
    screen = "result";
  }

  /** Stops the one under way and leaves the rest for another time. */
  function cancel() {
    cancelled = true;
    void cancelAgentCliInstall();
  }

  function close() {
    // Mid-install it's cancelled first, with its own button.
    if (screen === "installing") return;
    uiState.closeMissingAgents();
  }

  /** They're gone on purpose: no more asking about these. */
  function forget() {
    missingAgents.forget(tools);
    uiState.closeMissingAgents();
  }

  function onKeydown(event: KeyboardEvent) {
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
    aria-modal="true"
    tabindex="-1"
    aria-label={t("agentInstall.missing.title")}
    bind:this={dialogEl}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <h2 class="dialog-title">{t("agentInstall.missing.title")}</h2>
    {#if screen === "ask"}
      <p class="dialog-text">
        {missingAgents.node ? t("agentInstall.missing.text", { node: missingAgents.node }) : t("agentInstall.missing.textNoVersion")}
      </p>
    {:else if screen === "result"}
      <p class="dialog-text">{t(left.length === 0 ? "agentInstall.missing.allDone" : "agentInstall.missing.someLeft")}</p>
    {/if}

    <ul class="agents">
      {#each tools as tool (tool)}
        {@const state = states[tool]}
        <li class="agent" class:failed={state === "failed"} class:done={state === "done"}>
          <span class="agent-icon"><ConsoleIcon kind={tool} size={16} /></span>
          <span class="agent-name">{TOOL_DISPLAY_NAMES[tool]}</span>
          {#if state}
            <span class="agent-state">{t(STATE_KEYS[state])}</span>
          {/if}
          {#if errors[tool]}
            <pre class="agent-error">{errors[tool]}</pre>
          {/if}
        </li>
      {/each}
    </ul>

    {#if screen === "installing"}
      <p class="npm-line" aria-live="polite">{line || " "}</p>
    {:else if screen === "ask"}
      <p class="dialog-hint">{t("agentInstall.missing.hint")}</p>
    {/if}

    <div class="dialog-actions">
      {#if screen === "ask"}
        <button class="dialog-button quiet" onclick={forget}>{t("agentInstall.missing.forget")}</button>
        <span class="spacer"></span>
        <button class="dialog-button" onclick={close}>{t("agentInstall.missing.later")}</button>
        <button class="dialog-button primary" bind:this={reinstallEl} onclick={reinstall}>
          {t("agentInstall.missing.reinstall")}
        </button>
      {:else if screen === "installing"}
        <button class="dialog-button" onclick={cancel}>{t("agentInstall.cancel")}</button>
      {:else}
        {#if left.length > 0}
          <button class="dialog-button" onclick={reinstall}>{t("agentInstall.missing.retry")}</button>
        {/if}
        <button class="dialog-button primary" onclick={close}>{t("agentInstall.close")}</button>
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
    width: min(480px, 90%);
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

  .dialog-text,
  .dialog-hint {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    opacity: 0.85;
  }

  .dialog-hint {
    font-size: 11px;
    opacity: 0.6;
  }

  .agents {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .agent {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    font-size: 12px;
  }

  .agent-icon {
    display: flex;
  }

  .agent-name {
    flex: 1;
  }

  .agent-state {
    font-size: 11px;
    opacity: 0.7;
  }

  .agent.done .agent-state {
    color: #4caf7d;
    opacity: 1;
  }

  .agent.failed .agent-state {
    color: #ff6b6b;
    opacity: 1;
  }

  .agent-error {
    flex-basis: 100%;
    margin: 0;
    max-height: 90px;
    overflow: auto;
    font-family: var(--terminal-font-family, monospace);
    font-size: 11px;
    white-space: pre-wrap;
    word-break: break-word;
    opacity: 0.8;
  }

  .npm-line {
    margin: 0;
    font-family: var(--terminal-font-family, monospace);
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    opacity: 0.6;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
  }

  .spacer {
    flex: 1;
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
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .dialog-button.primary:hover {
    filter: brightness(1.1);
  }

  .dialog-button.quiet {
    border-color: transparent;
    opacity: 0.7;
  }
</style>
