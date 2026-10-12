<script lang="ts">
  import { tick } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
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
  import { stepLine } from "$lib/agents/planning-progress";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { getTerminal } from "$lib/terminal/terminal-registry";
  import { tabCwd } from "$lib/terminal/tab-cwd.svelte";
  import { formatUsd, quickAskEngine, quickAskModel, sendsExchange } from "./companion";
  import { renderMarkdown } from "./markdown";
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
  /** The model it's asked for, where one is; otherwise the CLI's own pick. */
  const askedModel = $derived(engine ? quickAskModel(engine) : null);

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
<div class="qa-backdrop" onpointerdown={close} transition:fade={{ duration: 140 }}></div>

<!-- A dialog in the middle of the window, unlike the palette's strip along
     its top: this one is read, not just typed into. -->
<div class="qa-anchor">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="qa"
    role="dialog"
    aria-modal="true"
    aria-label={t("companion.quick.title")}
    tabindex="-1"
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:scale={{ start: 0.96, duration: 160, easing: cubicOut }}
  >
    <header class="qa-header">
      <span class="qa-mark" aria-hidden="true">?</span>
      <div class="qa-heading">
        <span class="qa-title">{t("companion.quick.title")}</span>
        <span class="qa-sub">
          {#if engine}
            <ConsoleIcon kind={engine} size={12} />
            {t("companion.quick.answers", {
              engine: TOOL_DISPLAY_NAMES[engine],
              model: askedModel ?? t("companion.quick.defaultModel"),
            })}
          {:else}
            {t("companion.quick.noEngine")}
          {/if}
        </span>
      </div>
      {#if quickAskStore.entries.length > 0}
        <button onclick={() => quickAskStore.clear()} disabled={asking}>{t("companion.quick.clear")}</button>
      {/if}
      <button class="icon" onclick={close} aria-label={t("companion.close")}>✕</button>
    </header>

    <div class="qa-list" bind:this={listEl}>
      {#if quickAskStore.entries.length === 0}
        <p class="qa-empty">{t("companion.quick.intro")}</p>
      {/if}
      {#each quickAskStore.entries as entry (entry.id)}
        <article class="qa-entry">
          <p class="qa-question">{entry.question}</p>
          {#if entry.answer !== null}
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            <div class="qa-answer">{@html renderMarkdown(entry.answer)}</div>
            <div class="qa-foot">
              <span class="qa-meta">
                {entry.model ?? TOOL_DISPLAY_NAMES[entry.engine]}
                {#if entry.costUsd != null}· {t("companion.quick.cost", { amount: formatUsd(entry.costUsd) })}{/if}
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

    <div class="qa-compose">
      <textarea
        bind:this={inputEl}
        bind:value={question}
        class="qa-input"
        rows="3"
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
</div>

<style>
  .qa-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.5);
  }

  /* The dialog itself carries the scale transition, which animates
     transform — so the centering lives on this wrapper instead. */
  .qa-anchor {
    position: absolute;
    inset: 0;
    z-index: 400;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .qa {
    pointer-events: auto;
    width: min(720px, 92%);
    max-height: min(80%, 720px);
    display: flex;
    flex-direction: column;
    background: var(--tabbar-active-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    border: 1px solid var(--pane-active-border, #3a6df0);
    border-radius: 12px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.55);
    overflow: hidden;
    font-size: 13px;
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
    padding: 12px 14px;
    gap: 10px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .qa-mark {
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    font-weight: 700;
    color: #fff;
    background: var(--pane-active-border, #3a6df0);
  }

  .qa-heading {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .qa-title {
    font-size: 14px;
    font-weight: 600;
  }

  .qa-sub {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    opacity: 0.7;
  }

  .qa-meta {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .qa-list {
    flex: 1;
    min-height: 96px;
    overflow-y: auto;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .qa-empty {
    margin: auto;
    max-width: 420px;
    text-align: center;
    line-height: 1.5;
    opacity: 0.6;
  }

  .qa-entry {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  /* What was asked, set apart from what was answered. */
  .qa-question {
    align-self: flex-end;
    max-width: 85%;
    margin: 0;
    padding: 7px 10px;
    border-radius: 10px 10px 2px 10px;
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 22%, transparent);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .qa-answer {
    line-height: 1.55;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .qa-answer :global(p),
  .qa-answer :global(ul),
  .qa-answer :global(ol),
  .qa-answer :global(pre),
  .qa-answer :global(blockquote),
  .qa-answer :global(h4) {
    margin: 0 0 8px;
  }

  .qa-answer :global(:last-child) {
    margin-bottom: 0;
  }

  .qa-answer :global(h4) {
    font-size: 13px;
    font-weight: 600;
  }

  .qa-answer :global(ul),
  .qa-answer :global(ol) {
    padding-left: 20px;
  }

  .qa-answer :global(li) {
    margin-bottom: 3px;
  }

  .qa-answer :global(code) {
    padding: 1px 4px;
    border-radius: 3px;
    font-family: var(--terminal-font-family, ui-monospace, Consolas, monospace);
    font-size: 12px;
    background: rgba(127, 127, 127, 0.2);
  }

  .qa-answer :global(pre) {
    padding: 10px;
    overflow-x: auto;
    border-radius: 6px;
    background: rgba(127, 127, 127, 0.14);
  }

  .qa-answer :global(pre code) {
    padding: 0;
    background: none;
    white-space: pre;
  }

  .qa-answer :global(blockquote) {
    padding-left: 10px;
    border-left: 2px solid var(--tabbar-border, #2a2a2a);
    opacity: 0.85;
  }

  .qa-answer :global(hr) {
    border: none;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
    margin: 8px 0;
  }

  .qa-answer :global(.link) {
    text-decoration: underline;
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

  .qa-compose {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
    background: rgba(127, 127, 127, 0.06);
  }

  .qa-input {
    resize: none;
    padding: 12px 14px 4px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 13px;
    line-height: 1.45;
  }

  .qa-input:focus {
    outline: none;
  }

  .qa-footer {
    padding: 6px 14px 12px;
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

  button.icon {
    border: none;
    padding: 2px 6px;
  }

  button.primary {
    padding: 4px 14px;
    color: #fff;
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
  }

  button.primary:hover:not(:disabled) {
    background: var(--pane-active-border, #3a6df0);
    filter: brightness(1.1);
  }
</style>
