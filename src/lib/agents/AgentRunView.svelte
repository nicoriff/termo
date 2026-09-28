<script lang="ts">
  import { onMount, tick } from "svelte";
  import { agentRunLog, agentRuns, onAgentRunEvent, type RunEvent, type RunRecord } from "$lib/core/agents-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { agentsStore } from "./agents-store.svelte";
  import { ago, formatDuration, withoutEchoes } from "./agent-spec";
  import { renderMarkdownLite } from "./markdown-lite";
  import type { MessageKey } from "$lib/i18n/translate";

  let { agentId, runId }: { agentId: string; runId: string } = $props();

  const agent = $derived(agentsStore.agents.find((candidate) => candidate.id === agentId));
  let runs = $state<RunRecord[]>([]);
  let events = $state<RunEvent[]>([]);
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

  const TRIGGER_KEYS: Record<RunRecord["trigger"], MessageKey> = {
    manual: "agents.run.trigger.manual",
    test: "agents.run.trigger.test",
    schedule: "agents.run.trigger.schedule",
    watch: "agents.run.trigger.watch",
    git: "agents.run.trigger.git",
    webhook: "agents.run.trigger.webhook",
  };

  function statusColor(status: RunRecord["status"]): string {
    const c = themeStore.current.colors;
    if (status === "running") return c.blue;
    if (status === "success") return c.green;
    if (status === "stopped") return c.brightBlack;
    return c.red;
  }

  async function loadRuns() {
    runs = await agentRuns(agentId).catch(() => []);
  }

  // A different run on screen: its log from disk, then live steps on top.
  $effect(() => {
    const id = runId;
    let cancelled = false;
    events = [];
    stickToBottom = true;
    void agentRunLog(agentId, id).then((log) => {
      if (cancelled) return;
      events = log;
      void scrollDown();
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
</script>

<div class="run-view">
  <aside class="run-history">
    <div class="history-title">{t("agents.run.history")}</div>
    {#each runs as item (item.id)}
      {@const age = ago(item.startedAt, agentsStore.now)}
      <button class="history-row" class:selected={item.id === runId} onclick={() => show(item.id)}>
        <span class="dot" style="background:{statusColor(item.status)}"></span>
        <span class="history-text">
          <span>{t(age.key, age.params)}</span>
          <span class="history-sub">{t(TRIGGER_KEYS[item.trigger])}</span>
        </span>
      </button>
    {:else}
      <p class="muted">{t("agents.run.noRuns")}</p>
    {/each}
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
          </span>
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
        {:else if event.kind === "started"}
          <div class="step marker">{t("agents.run.started")} · {time(event.at)}</div>
        {:else if event.kind === "ended"}
          <div class="step marker">{t("agents.run.ended")} · {time(event.at)}</div>
        {/if}
      {:else}
        <p class="muted">{run?.status === "running" ? t("agents.run.waiting") : t("agents.run.emptyLog")}</p>
      {/each}
      {#if run?.status === "running"}
        <div class="step marker working">{run.activity || t("agents.run.working")}</div>
      {/if}
      {#if run && run.status !== "running" && run.status !== "success" && run.summary}
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
