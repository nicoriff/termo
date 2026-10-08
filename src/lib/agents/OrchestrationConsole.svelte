<script lang="ts">
  import { onMount, tick } from "svelte";
  import { agentRunLog, onAgentRunEvent, type RunEvent } from "$lib/core/agents-client";
  import type { PlannedAgent, PlannedTask } from "$lib/core/orchestration-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { agentsStore } from "./agents-store.svelte";
  import { renderMarkdownLite } from "./markdown-lite";
  import RunFailureCard from "./RunFailureCard.svelte";
  import RunNotice from "./RunNotice.svelte";
  import { orchestrationStore } from "./orchestration-store.svelte";
  import { STATUS_KEYS, agentOf, mergeRunLogs, pendingQuestions, progressOf, statusTone } from "./orchestration-spec";

  /** Everything an orchestration's agents did, as one console: every task's
   * run, step by step in the order it happened, live while it runs. */
  let { orchestrationId }: { orchestrationId: string } = $props();

  const orchestration = $derived(orchestrationStore.get(orchestrationId));
  const plan = $derived(orchestration?.plan ?? { agents: [], tasks: [] });
  const progress = $derived(progressOf(plan));
  const waiting = $derived(orchestration ? pendingQuestions(orchestration) : []);

  interface Source {
    task: PlannedTask;
    agent: PlannedAgent;
    runId: string;
  }

  /** The tasks that ran or are running, with the run that did it. */
  const sources = $derived(
    plan.tasks.flatMap((task): Source[] => {
      const agent = agentOf(plan, task);
      return task.runId && agent?.agentId ? [{ task, agent, runId: task.runId }] : [];
    }),
  );
  const byRun = $derived(new Map(sources.map((source) => [source.runId, source])));

  /** Each run's steps: read from disk once, and heard live after that. Live
   * steps of the plan's agents are kept even before the plan names their run,
   * since the run starts before the plan is saved with it. */
  let loaded = $state<Record<string, RunEvent[]>>({});
  let live = $state<Record<string, RunEvent[]>>({});
  const requested = new Set<string>();

  $effect(() => {
    for (const source of sources) {
      if (requested.has(source.runId)) continue;
      requested.add(source.runId);
      void agentRunLog(source.agent.agentId, source.runId)
        .catch(() => [])
        .then((log) => {
          loaded = { ...loaded, [source.runId]: log };
          void scrollDown();
        });
    }
  });

  /** Only the agents shown; none hidden at first. */
  let hidden = $state<Record<string, boolean>>({});

  const lines = $derived(
    mergeRunLogs(
      sources
        .filter((source) => !hidden[source.agent.key])
        .map((source) => ({
          source: source.runId,
          events: [...(loaded[source.runId] ?? []), ...(live[source.runId] ?? [])],
        })),
    ),
  );

  const running = $derived(sources.filter((source) => source.task.status === "running"));

  let logEl: HTMLDivElement | undefined = $state();
  let stickToBottom = true;

  onMount(() => {
    let unlisten: (() => void) | null = null;
    void onAgentRunEvent((step) => {
      if (!plan.agents.some((agent) => agent.agentId === step.agentId)) return;
      live = { ...live, [step.runId]: [...(live[step.runId] ?? []), step.event] };
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

  /** One color per agent, in the plan's order, from the theme. */
  function agentColor(agent: PlannedAgent): string {
    const c = themeStore.current.colors;
    const palette = [c.cyan, c.magenta, c.yellow, c.green, c.blue, c.brightCyan, c.brightMagenta, c.brightYellow];
    const index = plan.agents.findIndex((candidate) => candidate.key === agent.key);
    return palette[Math.max(0, index) % palette.length];
  }

  function statusColor(): string {
    const c = themeStore.current.colors;
    const tone = orchestration ? statusTone(orchestration.status) : "idle";
    if (tone === "running") return c.blue;
    if (tone === "success") return c.green;
    if (tone === "error") return c.red;
    if (tone === "paused") return c.yellow;
    return c.brightBlack;
  }

  function time(ms: number): string {
    return new Date(ms).toLocaleTimeString();
  }

  function openRun(source: Source) {
    workspaceStore.openAgentRun(source.agent.agentId, source.runId, source.agent.name);
  }

  function toggle(agent: PlannedAgent) {
    hidden = { ...hidden, [agent.key]: !hidden[agent.key] };
  }
</script>

<section class="console-view">
  {#if !orchestration}
    <p class="muted center">{t("agents.orchestration.view.missing")}</p>
  {:else}
    <header class="header">
      <span class="header-icon">{orchestration.icon || "🧭"}</span>
      <div class="heading">
        <span class="name">{t("agents.orchestration.console.title", { name: orchestration.name })}</span>
        <span class="status">
          <span class="dot" class:working={orchestration.status === "running"} style="background:{statusColor()}"></span>
          {t(STATUS_KEYS[orchestration.status])}
          <span class="muted">
            · {t("agents.orchestration.progress", { done: String(progress.done), total: String(progress.total) })}
          </span>
        </span>
      </div>
      <button class="button" onclick={() => workspaceStore.openOrchestration(orchestration.id, orchestration.name)}>
        {t("agents.orchestration.console.backToPlan")}
      </button>
    </header>

    {#each waiting as question (question.id)}
      <button class="asked" onclick={() => workspaceStore.openOrchestration(orchestration.id, orchestration.name)}>
        <strong>❓ {t("agents.orchestration.questions.asks", { agent: question.agent })}</strong>
        <span class="asked-text">{question.question}</span>
        <span class="asked-go">{t("agents.orchestration.questions.answerInPlan")}</span>
      </button>
    {/each}

    <div class="filters" role="group" aria-label={t("agents.orchestration.console.filter")}>
      {#each plan.agents as agent (agent.key)}
        {@const working = running.some((source) => source.agent.key === agent.key)}
        <button
          class="filter"
          class:off={hidden[agent.key]}
          aria-pressed={!hidden[agent.key]}
          title={t("agents.orchestration.console.toggle")}
          onclick={() => toggle(agent)}
        >
          <span class="swatch" class:working style="background:{agentColor(agent)}"></span>
          {agent.icon || "🤖"}
          {agent.name}
        </button>
      {/each}
    </div>

    <div class="log" bind:this={logEl} onscroll={onScroll}>
      {#each lines as line, index (index)}
        {@const source = byRun.get(line.source)}
        {@const first = index === 0 || lines[index - 1].source !== line.source}
        {#if source}
          {#if first}
            <button class="block-head" style="--agent:{agentColor(source.agent)}" onclick={() => openRun(source)}>
              <span class="agent-name">{source.agent.icon || "🤖"} {source.agent.name}</span>
              <span class="task-title">· {source.task.title}</span>
            </button>
          {/if}
          <div class="line" style="--agent:{agentColor(source.agent)}">
            <span class="time">{time(line.event.at)}</span>
            {#if line.event.kind === "tool"}
              <span class="step tool"><span class="tool-name">{line.event.tool}</span> {line.event.text}</span>
            {:else if line.event.kind === "text"}
              <span class="step text">{@html renderMarkdownLite(line.event.text)}</span>
            {:else if line.event.kind === "result"}
              <span class="step result">
                <span class="label">{t("agents.run.result")}</span>
                {@html renderMarkdownLite(line.event.text)}
              </span>
            {:else if line.event.kind === "stderr"}
              <span class="step stderr">{line.event.text}</span>
            {:else if line.event.kind === "started"}
              <span class="step marker">
                {t("agents.run.started")}{#if line.event.session}{" · "}{line.event.session.model || t("agents.diagnosis.defaultModel")}{/if}
              </span>
            {:else if line.event.kind === "session" || line.event.kind === "usage"}
              <span class="step wide"><RunNotice event={line.event} /></span>
            {:else if line.event.kind === "failure" && line.event.failure}
              <span class="step wide"><RunFailureCard failure={line.event.failure} at={line.event.at} /></span>
            {:else}
              <span class="step marker">{t("agents.run.ended")}</span>
            {/if}
          </div>
        {/if}
      {:else}
        <p class="muted">
          {orchestration.status === "running" ? t("agents.run.waiting") : t("agents.orchestration.console.empty")}
        </p>
      {/each}
      {#each running as source (source.runId)}
        {#if !hidden[source.agent.key]}
          <div class="line working-line" style="--agent:{agentColor(source.agent)}">
            <span class="agent-name">{source.agent.icon || "🤖"} {source.agent.name}</span>
            <span class="step marker working">{agentsStore.lastRun[source.agent.agentId]?.activity || t("agents.run.working")}</span>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</section>

<style>
  .console-view {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--terminal-bg, transparent);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12.5px;
  }

  .header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px 8px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .header-icon {
    font-size: 22px;
  }

  .heading {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .name {
    font-size: 14px;
    font-weight: 600;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
  }

  .asked {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 16px 0;
    padding: 8px 10px;
    border: 1px solid #d7a72a;
    border-radius: 6px;
    background: rgba(215, 167, 42, 0.12);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .asked-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.85;
  }

  .asked-go {
    flex-shrink: 0;
    font-size: 11.5px;
    text-decoration: underline;
  }

  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .filter {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 9px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .filter.off {
    opacity: 0.4;
  }

  .swatch {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .log {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 16px 18px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
    scrollbar-width: thin;
  }

  .block-head {
    display: flex;
    gap: 6px;
    margin: 10px 0 2px;
    padding: 0;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .block-head:hover .task-title {
    text-decoration: underline;
  }

  .agent-name {
    color: var(--agent);
    white-space: nowrap;
  }

  .task-title {
    opacity: 0.75;
  }

  .line {
    display: flex;
    gap: 10px;
    padding: 1px 0 1px 8px;
    border-left: 2px solid var(--agent);
    line-height: 1.45;
  }

  .time {
    flex-shrink: 0;
    opacity: 0.4;
  }

  .step {
    min-width: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .step :global(p) {
    margin: 0;
  }

  .tool-name {
    padding: 0 5px;
    border-radius: 3px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
    font-weight: 600;
  }

  .step.tool {
    opacity: 0.8;
  }

  .step.result {
    display: block;
    flex: 1;
    margin: 2px 0;
    padding: 6px 10px;
    border-radius: 4px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .label {
    display: block;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.6;
  }

  .step.wide {
    flex: 1;
  }

  .step.stderr {
    opacity: 0.5;
  }

  .step.marker {
    opacity: 0.45;
  }

  .working-line {
    margin-top: 8px;
  }

  .dot {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
  }

  .working {
    animation: pulse 1.4s ease-in-out infinite;
  }

  .step.marker.working {
    opacity: 0.75;
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

  .button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .muted {
    opacity: 0.6;
  }

  .center {
    margin: auto;
  }

  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }
</style>
