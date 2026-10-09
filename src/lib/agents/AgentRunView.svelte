<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    agentConversation,
    agentRunLog,
    agentRuns,
    conversationOf,
    onAgentRunEvent,
    type RunEvent,
    type RunRecord,
  } from "$lib/core/agents-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { agentsStore } from "./agents-store.svelte";
  import { TRIGGER_KEYS, ago, conversationTree, formatDuration, withoutEchoes } from "./agent-spec";
  import { renderMarkdownLite } from "./markdown-lite";
  import RunFailureCard from "./RunFailureCard.svelte";
  import RunNotice from "./RunNotice.svelte";
  import { ENGINE_NAMES } from "./orchestration-spec";
  import type { MessageKey } from "$lib/i18n/translate";

  let { agentId, runId }: { agentId: string; runId: string } = $props();

  const agent = $derived(agentsStore.agents.find((candidate) => candidate.id === agentId));
  let runs = $state<RunRecord[]>([]);
  /** Whether the history has come from disk yet: until then, the last run
   * the store already holds stands in for it. */
  let runsLoaded = $state(false);
  let events = $state<RunEvent[]>([]);
  /** The log of the run on screen is still being read. */
  let logLoading = $state(true);
  let logEl: HTMLDivElement | undefined = $state();
  let stickToBottom = true;
  let error = $state("");

  /** The run on screen, kept current by the `agent-run` event through the store. */
  const run = $derived.by(() => {
    const live = agentsStore.lastRun[agentId];
    if (live?.id === runId) return live;
    return runs.find((candidate) => candidate.id === runId) ?? null;
  });

  const STATUS_KEYS: Record<RunRecord["status"], MessageKey> = {
    running: "agents.run.status.running",
    success: "agents.run.status.success",
    error: "agents.run.status.error",
    stopped: "agents.run.status.stopped",
    timedOut: "agents.run.status.timedOut",
  };

  const agentNamed = (id: string) => agentsStore.agents.find((candidate) => candidate.id === id);

  /** The other runs of the conversation between agents this run is part of. */
  let conversation = $state<RunRecord[]>([]);
  const conversationId = $derived(run ? conversationOf(run) : null);
  // Reloaded as any agent's run starts or ends: the conversation's runs
  // belong to several agents.
  const liveSignature = $derived(
    Object.values(agentsStore.lastRun)
      .map((candidate) => `${candidate.id}:${candidate.status}`)
      .join(),
  );
  $effect(() => {
    const id = conversationId;
    void liveSignature;
    if (!id) {
      conversation = [];
      return;
    }
    let cancelled = false;
    void agentConversation(id)
      .then((runs) => {
        if (!cancelled) conversation = runs;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  function showRun(other: RunRecord) {
    workspaceStore.openAgentRun(other.agentId, other.id, agentNamed(other.agentId)?.name ?? "");
  }

  function statusColor(status: RunRecord["status"]): string {
    const c = themeStore.current.colors;
    if (status === "running") return c.blue;
    if (status === "success") return c.green;
    if (status === "stopped") return c.brightBlack;
    return c.red;
  }

  async function loadRuns() {
    runs = await agentRuns(agentId).catch(() => []);
    runsLoaded = true;
  }

  /** What the history shows: before it's read, the run the store has. */
  const history = $derived.by(() => {
    if (runsLoaded) return runs;
    const live = agentsStore.lastRun[agentId];
    return live ? [live] : [];
  });

  // A different run on screen: its log from disk, then live steps on top.
  $effect(() => {
    const id = runId;
    let cancelled = false;
    events = [];
    logLoading = true;
    stickToBottom = true;
    void agentRunLog(agentId, id)
      .then((log) => {
        if (cancelled) return;
        events = log;
        void scrollDown();
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) logLoading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  // New runs of this agent show up in the history as they start and end.
  $effect(() => {
    void agentsStore.lastRun[agentId]?.status;
    void loadRuns();
  });

  onMount(() => {
    let unlisten: (() => void) | null = null;
    void onAgentRunEvent((step) => {
      if (step.agentId !== agentId || step.runId !== runId) return;
      events = [...events, step.event];
      void scrollDown();
    }).then((stop) => (unlisten = stop));
    return () => unlisten?.();
  });

  async function scrollDown() {
    if (!stickToBottom) return;
    await tick();
    logEl?.scrollTo({ top: logEl.scrollHeight });
  }

  function onScroll() {
    if (!logEl) return;
    stickToBottom = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < 40;
  }

  async function runAgain() {
    if (!agent) return;
    error = "";
    try {
      const started = await agentsStore.run(agent);
      workspaceStore.openAgentRun(agentId, started.id, agent.name);
    } catch (reason) {
      error = String(reason);
    }
  }

  function show(id: string) {
    workspaceStore.openAgentRun(agentId, id, agent?.name ?? "");
  }

  function time(ms: number): string {
    return new Date(ms).toLocaleString();
  }

  const elapsed = $derived(run ? (run.endedAt ?? agentsStore.now) - run.startedAt : 0);

  /** What this run is waiting on the user for. */
  const question = $derived(agentsStore.questions.find((candidate) => candidate.runId === runId) ?? null);
  const waitsUntil = $derived(
    question ? new Date(question.expiresAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "",
  );
  let reply = $state("");
  let sending = $state(false);
  // Another question: what was being written was for the one before.
  $effect(() => {
    void question?.id;
    reply = "";
  });

  async function sendAnswer() {
    const text = reply.trim();
    if (!question || !text || sending) return;
    sending = true;
    error = "";
    try {
      if (!(await agentsStore.answerQuestion(question.id, text))) error = t("agents.run.question.gone");
    } catch (reason) {
      error = String(reason);
    } finally {
      sending = false;
    }
  }
</script>

<div class="run-view" style="--question:{themeStore.current.colors.yellow}">
  <aside class="run-history">
    <div class="history-title">{t("agents.run.history")}</div>
    {#each history as item (item.id)}
      {@const age = ago(item.startedAt, agentsStore.now)}
      <button class="history-row" class:selected={item.id === runId} onclick={() => show(item.id)}>
        <span class="dot" style="background:{statusColor(item.status)}"></span>
        <span class="history-text">
          <span>{t(age.key, age.params)}</span>
          <span class="history-sub">{t(TRIGGER_KEYS[item.trigger])}</span>
        </span>
      </button>
    {:else}
      <p class="muted">{runsLoaded ? t("agents.run.noRuns") : t("agents.run.loading")}</p>
    {/each}
    {#if conversation.length > 1}
      <div class="history-title conversation-title">{t("agents.run.conversation")}</div>
      {#each conversationTree(conversation) as { run: item, depth } (item.id)}
        {@const other = agentNamed(item.agentId)}
        <button
          class="history-row"
          class:selected={item.id === runId}
          style="padding-left:{10 + depth * 12}px"
          title={t("agents.run.openRun")}
          onclick={() => showRun(item)}
        >
          <span class="dot" style="background:{statusColor(item.status)}"></span>
          <span class="history-text">
            <span>{other?.icon ?? "🤖"} {other?.name ?? t("agents.run.deleted")}</span>
            <span class="history-sub">{t(TRIGGER_KEYS[item.trigger])}</span>
          </span>
        </button>
      {/each}
    {/if}
  </aside>

  <section class="run-main">
    <header class="run-header">
      <span class="run-icon">{agent?.icon ?? "🤖"}</span>
      <div class="run-heading">
        <span class="run-name">{agent?.name ?? t("agents.run.deleted")}</span>
        {#if run}
          <span class="run-meta">
            <span class="dot" style="background:{statusColor(run.status)}"></span>
            {t(STATUS_KEYS[run.status])} · {time(run.startedAt)} · {formatDuration(elapsed)}
            {#if run.costUsd != null}· US$ {run.costUsd.toFixed(2)}{/if}
            {#if run.rehearsal}<span class="rehearsal-badge">{t("agents.run.rehearsal")}</span>{/if}
          </span>
          {#if run.link}
            {@const link = run.link}
            <button
              class="requested-by"
              onclick={() =>
                workspaceStore.openAgentRun(link.parentAgentId, link.parentRunId, agentNamed(link.parentAgentId)?.name ?? "")}
            >
              ↳ {t("agents.run.requestedBy", {
                agent: agentNamed(link.parentAgentId)?.name ?? t("agents.run.deleted"),
              })}
            </button>
          {/if}
        {/if}
      </div>
      {#if run?.status === "running"}
        <button class="run-button" onclick={() => void agentsStore.stop(agentId)}>{t("agents.actions.stop")}</button>
      {:else if agent}
        <button class="run-button primary" onclick={() => void runAgain()}>{t("agents.actions.runAgain")}</button>
      {/if}
    </header>
    {#if error}
      <p class="run-error">{error}</p>
    {/if}
    {#if question}
      <section class="question" aria-label={t("agents.orchestration.questions.title")}>
        <header class="question-head">
          <span class="question-icon">?</span>
          <span class="question-kicker">
            {t("agents.orchestration.questions.asks", { agent: agent?.name ?? t("agents.run.deleted") })}
          </span>
        </header>
        <div class="question-text">{@html renderMarkdownLite(question.question)}</div>
        {#if question.options.length > 0}
          <div class="question-options">
            {#each question.options as option (option)}
              <button class="run-button" class:picked={reply === option} onclick={() => (reply = option)}>{option}</button>
            {/each}
          </div>
        {/if}
        <textarea
          class="question-reply"
          rows="2"
          placeholder={t("agents.orchestration.questions.placeholder")}
          aria-label={t("agents.orchestration.questions.placeholder")}
          bind:value={reply}
          onkeydown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void sendAnswer();
          }}
        ></textarea>
        <div class="question-actions">
          <button class="run-button primary" disabled={sending || !reply.trim()} onclick={() => void sendAnswer()}>
            {t("agents.orchestration.questions.send")}
          </button>
          <span class="question-hint">{t("agents.run.question.hint", { time: waitsUntil })}</span>
        </div>
        <p class="question-hint">⚠ {t("agents.orchestration.questions.noSecrets")}</p>
      </section>
    {/if}

    <div class="run-log" bind:this={logEl} onscroll={onScroll}>
      {#each withoutEchoes(events) as event, index (index)}
        {#if event.kind === "tool"}
          <div class="step tool">
            <span class="step-tool">{event.tool}</span>
            <span class="step-detail">{event.text}</span>
          </div>
        {:else if event.kind === "text"}
          <div class="step text">{@html renderMarkdownLite(event.text)}</div>
        {:else if event.kind === "result"}
          <div class="step result">
            <div class="step-label">{t("agents.run.result")}</div>
            {@html renderMarkdownLite(event.text)}
          </div>
        {:else if event.kind === "stderr"}
          <div class="step stderr">{event.text}</div>
        {:else if event.kind === "question"}
          <div class="step asked">
            <div class="step-label">❓ {t("agents.run.question.asked")}</div>
            {@html renderMarkdownLite(event.text)}
          </div>
        {:else if event.kind === "answer"}
          <div class="step asked" class:unanswered={!event.text}>
            <div class="step-label">↳ {t(event.text ? "agents.run.question.answered" : "agents.orchestration.questions.expired")}</div>
            {event.text}
          </div>
        {:else if event.kind === "started"}
          <div class="step marker">
            {t("agents.run.started")} · {time(event.at)}{#if event.session}{" · "}{ENGINE_NAMES[event.session.engine]} · {event.session.model || t("agents.diagnosis.defaultModel")}{/if}
          </div>
        {:else if event.kind === "session" || event.kind === "usage"}
          <RunNotice {event} />
        {:else if event.kind === "failure" && event.failure}
          <RunFailureCard failure={event.failure} at={event.at} />
        {:else if event.kind === "ended"}
          <div class="step marker">{t("agents.run.ended")} · {time(event.at)}</div>
        {/if}
      {:else}
        <p class="muted">
          {logLoading ? t("agents.run.loading") : run?.status === "running" ? t("agents.run.waiting") : t("agents.run.emptyLog")}
        </p>
      {/each}
      {#if run?.status === "running"}
        <div class="step marker working">{run.activity || t("agents.run.working")}</div>
      {/if}
      {#if run && run.status !== "running" && run.status !== "success" && run.summary && !run.failure}
        <div class="step failure" style="border-color:{statusColor(run.status)}">{run.summary}</div>
      {/if}
    </div>
  </section>
</div>

<style>
  .run-view {
    position: absolute;
    inset: 0;
    display: flex;
    background: var(--terminal-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12.5px;
  }

  .run-history {
    width: 180px;
    flex-shrink: 0;
    overflow-y: auto;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    padding: 8px 0;
    scrollbar-width: thin;
  }

  .history-title {
    padding: 0 12px 6px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.55;
  }

  .conversation-title {
    margin-top: 14px;
  }

  .rehearsal-badge {
    padding: 0 6px;
    border: 1px solid currentColor;
    border-radius: 999px;
    font-size: 10px;
  }

  .requested-by {
    align-self: flex-start;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .requested-by:hover {
    text-decoration: underline;
  }

  .history-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 12px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .history-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .history-row.selected {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.25));
  }

  .history-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .history-sub {
    font-size: 10.5px;
    opacity: 0.55;
  }

  .dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
  }

  .run-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  /* The right padding keeps clear of the tile's floating toolbar. */
  .run-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 124px 10px 14px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .run-icon {
    font-size: 20px;
  }

  .run-heading {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .run-name {
    font-weight: 600;
    font-size: 13.5px;
  }

  .run-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    opacity: 0.7;
  }

  .run-button {
    padding: 5px 12px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .run-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .run-button.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .run-error {
    margin: 0;
    padding: 6px 14px;
    color: var(--error-fg, #f48771);
  }

  .question {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 10px 14px 0;
    padding: 10px 12px;
    border: 1px solid var(--question, #d7ba7d);
    border-left: 4px solid var(--question, #d7ba7d);
    border-radius: 5px;
    background: color-mix(in srgb, var(--question, #d7ba7d) 9%, transparent);
  }

  .question-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .question-icon {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--question, #d7ba7d);
    color: #1e1e1e;
    font-weight: 700;
    font-size: 12px;
  }

  .question-kicker {
    font-weight: 600;
  }

  .question-text {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: 1.45;
  }

  .question-options {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .question-options .picked {
    border-color: var(--question, #d7ba7d);
    background: color-mix(in srgb, var(--question, #d7ba7d) 22%, transparent);
  }

  .question-reply {
    resize: vertical;
    padding: 6px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, #1e1e1e);
    color: inherit;
    font: inherit;
  }

  .question-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .question-hint {
    margin: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .run-button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .step.asked {
    padding: 8px 12px;
    border-left: 3px solid var(--question, #d7ba7d);
    border-radius: 4px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .step.unanswered {
    opacity: 0.6;
  }

  .run-log {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 10px 14px 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    scrollbar-width: thin;
  }

  .step {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: 1.45;
  }

  .step :global(code) {
    padding: 0 4px;
    border-radius: 3px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 0.92em;
  }

  .step.tool {
    display: flex;
    gap: 8px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
    opacity: 0.85;
  }

  .step-tool {
    flex-shrink: 0;
    padding: 0 6px;
    border-radius: 3px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
    font-weight: 600;
  }

  .step-detail {
    opacity: 0.8;
  }

  .step.result {
    margin-top: 4px;
    padding: 10px 12px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
    border-radius: 4px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .step-label {
    margin-bottom: 4px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.6;
  }

  .step.stderr {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    opacity: 0.5;
  }

  .step.marker {
    font-size: 11px;
    opacity: 0.45;
  }

  .step.working {
    opacity: 0.75;
    animation: pulse 1.4s ease-in-out infinite;
  }

  .step.failure {
    padding: 8px 12px;
    border-left: 3px solid;
    border-radius: 4px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .muted {
    margin: 0;
    padding: 4px 12px;
    opacity: 0.55;
  }

  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }
</style>
