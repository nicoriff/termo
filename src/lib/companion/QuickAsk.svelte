<script lang="ts">
  import { tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { companionLastExchange } from "$lib/core/companion-client";
  import { collectSessions } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { tabAgentSessions } from "$lib/ai-mode/tab-sessions.svelte";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { aiEngine } from "$lib/agents/ai-engine.svelte";
  import { renderMarkdownLite } from "$lib/agents/markdown-lite";
  import { stepLine } from "$lib/agents/planning-progress";
  import { getTerminal } from "$lib/terminal/terminal-registry";
  import { tabCwd } from "$lib/terminal/tab-cwd.svelte";
  import { formatUsd, quickAskEngine, sendsExchange } from "./companion";
  import { quickAskStore } from "./quick-ask-store.svelte";

  let question = $state("");
  let inputEl: HTMLTextAreaElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  /** The console the question came up in: where the answer may look, whose
   * session's last exchange can go along, and where an answer is pasted. */
  const tabId = workspaceStore.focusedSessionId;
  const agentSession = $derived(tabId ? tabAgentSessions.byTab[tabId] : undefined);
  const cwd = $derived.by(() => {
    if (!tabId) return null;
    const opened = workspaceStore.root ? collectSessions(workspaceStore.root).find((entry) => entry.session.id === tabId)?.session.cwd : undefined;
    return agentSession?.cwd ?? tabCwd.byTab[tabId] ?? opened ?? null;
  });

  /** Whose CLI answers: the one picked in Preferences when it's installed,
   * else the first that is. Null when none is. */
  const engine = $derived(quickAskEngine(appConfigStore.quickAskEngine, aiEngine.installed));

  /** Sending the session's last exchange along is only on offer where there
   * is one to read. */
  const canSendExchange = $derived(sendsExchange(agentSession?.tool));
  let withExchange = $state(appConfigStore.quickAskWithExchange);

  const asking = $derived(quickAskStore.asking !== null);
  const lastStep = $derived(quickAskStore.steps.findLast((step) => step.kind === "tool"));

  $effect(() => {
    inputEl?.focus();
  });

  // The newest question, and its answer as it arrives, stay in view.
  $effect(() => {
    void quickAskStore.entries;
    void quickAskStore.steps;
    void tick().then(() => listEl?.scrollTo({ top: listEl.scrollHeight }));
  });

  function close() {
    uiState.closeQuickAsk();
  }

  async function send() {
    const text = question.trim();
    if (!text || !engine || asking) return;
    question = "";
    // Read as the question goes, so it's the exchange as it stands now.
    const exchange =
      withExchange && canSendExchange && agentSession
        ? await companionLastExchange(agentSession.tool, agentSession.agentSessionId).catch(() => null)
        : null;
    await quickAskStore.ask(text, engine, cwd, exchange);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      void send();
    }
  }

  /** The id of the answer just copied, for a moment. */
  let copied = $state("");

  async function copy(id: string, text: string) {
    await writeText(text).catch(() => {});
    copied = id;
    setTimeout(() => {
      if (copied === id) copied = "";
    }, 1500);
  }

  /** Into the console's own prompt, for the user to send or not: this is how
   * the session gets to know an answer, and only this. */
  function paste(text: string) {
    const terminal = tabId ? getTerminal(tabId) : undefined;
    if (!terminal) return;
    close();
    terminal.paste(text);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="qa-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="qa-anchor">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="qa"
    role="dialog"
    aria-label={t("companion.quick.title")}
    tabindex="-1"
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <header class="qa-header">
      <span class="qa-title">{t("companion.quick.title")}</span>
      <span class="qa-engine">
        {engine ? t("companion.quick.answers", { engine: TOOL_DISPLAY_NAMES[engine] }) : t("companion.quick.noEngine")}
      </span>
      {#if quickAskStore.entries.length > 0}
        <button onclick={() => quickAskStore.clear()} disabled={asking}>{t("companion.quick.clear")}</button>
      {/if}
    </header>

    {#if quickAskStore.entries.length > 0}
      <div class="qa-list" bind:this={listEl}>
        {#each quickAskStore.entries as entry (entry.id)}
          <article class="qa-entry">
            <p class="qa-question">{entry.question}</p>
            {#if entry.answer !== null}
              <!-- eslint-disable-next-line svelte/no-at-html-tags -->
              <div class="qa-answer">{@html renderMarkdownLite(entry.answer)}</div>
              <div class="qa-foot">
                <span class="qa-meta">
                  {#if entry.costUsd != null}{t("companion.quick.cost", { amount: formatUsd(entry.costUsd) })}{/if}
                </span>
                <button onclick={() => void copy(entry.id, entry.answer ?? "")}>
                  {copied === entry.id ? t("companion.advice.copied") : t("companion.advice.copy")}
                </button>
                {#if tabId && getTerminal(tabId)}
                  <button onclick={() => paste(entry.answer ?? "")} title={t("companion.quick.pasteHint")}>
                    {t("companion.quick.paste")}
                  </button>
                {/if}
              </div>
            {:else if entry.error !== null}
              <p class="qa-error" role="alert">{t("companion.quick.failed", { error: entry.error })}</p>
            {:else}
              <p class="qa-working" role="status">
                <span class="qa-spinner" aria-hidden="true"></span>
                {lastStep ? stepLine(lastStep, t).text : t("companion.quick.thinking")}
              </p>
            {/if}
          </article>
        {/each}
      </div>
    {/if}

    <textarea
      bind:this={inputEl}
      bind:value={question}
      class="qa-input"
      rows="2"
      placeholder={t("companion.quick.placeholder")}
      aria-label={t("companion.quick.placeholder")}
      disabled={!engine}
    ></textarea>

    <footer class="qa-footer">
      {#if canSendExchange}
        <label class="qa-check" title={t("companion.quick.withExchangeHint")}>
          <input type="checkbox" bind:checked={withExchange} />
          <span>{t("companion.quick.withExchange")}</span>
        </label>
      {:else}
        <span class="qa-meta">{t("companion.quick.apart")}</span>
      {/if}
      <button class="primary" onclick={() => void send()} disabled={!engine || asking || question.trim() === ""}>
        {asking ? t("companion.quick.asking") : t("companion.quick.ask")}
      </button>
    </footer>
  </div>
</div>

<style>
  .qa-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.35);
  }

  /* The panel itself carries the fly transition, which animates transform —
     so the centering lives on this wrapper instead of on the panel. */
  .qa-anchor {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }

  .qa {
    pointer-events: auto;
    width: min(640px, 92%);
    max-height: min(70vh, 640px);
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    background: var(--tabbar-active-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
    overflow: hidden;
    font-size: 12px;
  }

  .qa:focus {
    outline: none;
  }

  .qa-header,
  .qa-footer,
  .qa-foot {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .qa-header {
    padding: 8px 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .qa-title {
    font-size: 13px;
    font-weight: 600;
  }

  .qa-engine,
  .qa-meta {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .qa-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .qa-entry {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .qa-question {
    margin: 0;
    font-weight: 600;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  /* The renderer keeps the answer's own line breaks (see markdown-lite.ts). */
  .qa-answer {
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .qa-answer :global(code) {
    padding: 0 3px;
    border-radius: 3px;
    background: rgba(127, 127, 127, 0.18);
  }

  .qa-working,
  .qa-error {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    opacity: 0.8;
  }

  .qa-error {
    color: #f14c4c;
    opacity: 1;
  }

  .qa-spinner {
    flex-shrink: 0;
    width: 10px;
    height: 10px;
    border: 1.5px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: qa-spin 800ms linear infinite;
  }

  @keyframes qa-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .qa-input {
    resize: none;
    padding: 10px 12px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 13px;
    line-height: 1.4;
  }

  .qa-input:focus {
    outline: none;
  }

  .qa-footer {
    padding: 8px 12px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .qa-check {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    cursor: pointer;
  }

  .qa-check input {
    accent-color: var(--pane-active-border, #3a6df0);
  }

  button {
    flex-shrink: 0;
    font: inherit;
    font-size: 12px;
    color: inherit;
    background: transparent;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    padding: 3px 9px;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  button.primary {
    border-color: var(--pane-active-border, #3a6df0);
  }
</style>
