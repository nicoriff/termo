<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import {
    agentsBoard,
    improveProject,
    projectRuns,
    projectStats,
    projectVersions,
    removeBoardEntry,
    type AgentSpec,
    type BoardEntry,
    type Improvement,
    type ProjectStats,
    type ProjectVersion,
    type RunRecord,
    type Suggestion,
  } from "$lib/core/agents-client";
  import type { MessageKey } from "$lib/i18n/translate";
  import { agentsStore } from "./agents-store.svelte";
  import { AUTONOMY_TITLE_KEYS, TRIGGER_KEYS, agentState, ago, formatDuration, newAgentSpec, type AgentTone } from "./agent-spec";
  import { projectWarnings } from "./project-spec";
  import ProjectGraph from "./ProjectGraph.svelte";

  let { projectId }: { projectId: string } = $props();

  const project = $derived(agentsStore.project(projectId));
  const agents = $derived(agentsStore.agentsOf(projectId));
  const anyRunning = $derived(agents.some((agent) => agentsStore.isRunning(agent.id)));
  const allPaused = $derived(agents.length > 0 && agents.every((agent) => !agent.enabled));

  type Tab = "overview" | "activity" | "board" | "improve" | "versions";
  const TABS: { id: Tab; label: MessageKey }[] = [
    { id: "overview", label: "agents.projects.view.tabs.overview" },
    { id: "activity", label: "agents.projects.view.tabs.activity" },
    { id: "board", label: "agents.projects.view.tabs.board" },
    { id: "improve", label: "agents.projects.view.tabs.improve" },
    { id: "versions", label: "agents.projects.view.tabs.versions" },
  ];
  let tab = $state<Tab>("overview");
  let error = $state("");

  let stats = $state<ProjectStats | null>(null);
  let runs = $state<RunRecord[]>([]);
  let board = $state<Record<string, BoardEntry>>({});
  let versions = $state<ProjectVersion[]>([]);
  let confirmRestore = $state<number | null>(null);

  // Everything here moves with the runs of the project's agents and with
  // saves of them: reloaded on either.
  const signature = $derived(
    JSON.stringify([
      agents.map((agent) => [agent.id, agent.updatedAt, agentsStore.lastRun[agent.id]?.status ?? ""]),
      project?.updatedAt,
    ]),
  );
  $effect(() => {
    void signature;
    const id = projectId;
    void projectStats(id)
      .then((value) => (stats = value))
      .catch(() => {});
    if (tab === "activity") {
      void projectRuns(id, 60)
        .then((value) => (runs = value))
        .catch(() => {});
    }
    if (tab === "board" && project?.isolated) {
      void agentsBoard(id)
        .then((value) => (board = value))
        .catch(() => {});
    }
    if (tab === "versions") {
      void projectVersions(id)
        .then((value) => (versions = value))
        .catch(() => {});
    }
  });

  function toneColor(tone: AgentTone): string {
    const c = themeStore.current.colors;
    if (tone === "running") return c.blue;
    if (tone === "success") return c.green;
    if (tone === "error") return c.red;
    if (tone === "paused") return c.yellow;
    return c.brightBlack;
  }

  function statusColor(status: RunRecord["status"]): string {
    const c = themeStore.current.colors;
    if (status === "running") return c.blue;
    if (status === "success") return c.green;
    if (status === "stopped") return c.brightBlack;
    return c.red;
  }

  const agentColor = (id: string) => {
    const agent = agents.find((candidate) => candidate.id === id);
    return agent ? toneColor(agentState(agent, agentsStore.lastRun[id] ?? null).tone) : toneColor("idle");
  };

  const nameOf = (id: string) => agentsStore.agents.find((agent) => agent.id === id);

  function openAgent(id: string) {
    const agent = nameOf(id);
    if (!agent) return;
    const last = agentsStore.lastRun[id];
    if (last) workspaceStore.openAgentRun(id, last.id, agent.name);
    else uiState.openAgentWizard(agent, false);
  }

  async function act(action: () => Promise<unknown>) {
    error = "";
    try {
      await action();
    } catch (reason) {
      error = String(reason);
    }
  }

  async function run(rehearsal: boolean) {
    await act(async () => {
      const started = await agentsStore.runProject(projectId, rehearsal);
      workspaceStore.openAgentRun(started.agentId, started.id, nameOf(started.agentId)?.name ?? "");
    });
  }

  function newAgent() {
    if (!project) return;
    const spec = newAgentSpec(crypto.randomUUID());
    spec.projectId = project.id;
    spec.workDir = project.workDir;
    uiState.openAgentWizard(spec, true);
  }

  // Improving with AI.
  let improving = $state(false);
  let improvement = $state<Improvement | null>(null);
  let applied = $state<number[]>([]);

  async function improve() {
    improving = true;
    applied = [];
    await act(async () => {
      improvement = await improveProject(projectId);
    });
    improving = false;
  }

  const FIELD_KEYS: Record<Suggestion["field"], MessageKey> = {
    task: "agents.projects.view.improve.fields.task",
    instructions: "agents.projects.view.improve.fields.instructions",
    autonomy: "agents.projects.view.improve.fields.autonomy",
    timeoutMin: "agents.projects.view.improve.fields.timeoutMin",
  };

  function currentValue(agent: AgentSpec | undefined, suggestion: Suggestion): string {
    if (!agent) return "";
    if (suggestion.field === "task") return agent.task;
    if (suggestion.field === "instructions") return agent.instructions;
    if (suggestion.field === "autonomy") return t(AUTONOMY_TITLE_KEYS[agent.autonomy]);
    return String(agent.limits.timeoutMin);
  }

  function shownValue(suggestion: Suggestion): string {
    if (suggestion.field === "autonomy") return t(AUTONOMY_TITLE_KEYS[suggestion.value as AgentSpec["autonomy"]]);
    return suggestion.value;
  }

  async function apply(index: number, suggestion: Suggestion) {
    const agent = nameOf(suggestion.agentId);
    if (!agent) return;
    const spec = structuredClone($state.snapshot(agent));
    if (suggestion.field === "task") spec.task = suggestion.value;
    else if (suggestion.field === "instructions") spec.instructions = suggestion.value;
    else if (suggestion.field === "autonomy") spec.autonomy = suggestion.value as AgentSpec["autonomy"];
    else spec.limits.timeoutMin = Number(suggestion.value);
    await act(async () => {
      await agentsStore.save(spec);
      applied = [...applied, index];
    });
  }

  function discard(index: number) {
    if (!improvement) return;
    improvement = { ...improvement, suggestions: improvement.suggestions.filter((_, i) => i !== index) };
    applied = applied.filter((i) => i !== index).map((i) => (i > index ? i - 1 : i));
  }

  async function restore(at: number) {
    if (confirmRestore !== at) {
      confirmRestore = at;
      return;
    }
    confirmRestore = null;
    await act(() => agentsStore.restoreProject(projectId, at));
  }

  async function removeNote(key: string) {
    board = await removeBoardEntry(key, projectId).catch(() => board);
  }

  const time = (ms: number) => new Date(ms).toLocaleString();
</script>

<div class="project-view">
  {#if !project}
    <p class="muted center">{t("agents.projects.view.deleted")}</p>
  {:else}
    <header class="header">
      <span class="header-icon">{project.icon}</span>
      <div class="heading">
        <span class="name">{project.name}</span>
        {#if project.goal}<span class="goal">{project.goal}</span>{/if}
      </div>
      <div class="actions">
        {#if anyRunning}
          <button class="button" onclick={() => void act(() => agentsStore.stopProject(projectId))}>
            {t("agents.projects.view.stop")}
          </button>
        {/if}
        <button class="button" title={t("agents.projects.view.rehearseHint")} disabled={!project.entryAgentId} onclick={() => void run(true)}>
          🎭 {t("agents.projects.view.rehearse")}
        </button>
        <button class="button primary" disabled={!project.entryAgentId} onclick={() => void run(false)}>
          ▶ {t("agents.projects.view.run")}
        </button>
        <button
          class="button"
          disabled={agents.length === 0}
          onclick={() => void act(() => agentsStore.setProjectEnabled(projectId, allPaused))}
        >
          {allPaused ? t("agents.projects.menu.resumeAll") : t("agents.projects.menu.pauseAll")}
        </button>
        <button class="button" onclick={() => uiState.openProjectDialog(project)}>{t("agents.projects.menu.settings")}</button>
      </div>
    </header>
    {#if error}<p class="error">{error}</p>{/if}

    <nav class="tabs">
      {#each TABS as item (item.id)}
        <button class="tab" class:current={tab === item.id} onclick={() => (tab = item.id)}>{t(item.label)}</button>
      {/each}
    </nav>

    <div class="body">
      {#if tab === "overview"}
        {#if stats}
          <div class="stats">
            <span>
              {stats.maxRunsPerDay > 0
                ? t("agents.projects.view.runsTodayOf", { n: String(stats.runsToday), max: String(stats.maxRunsPerDay) })
                : t("agents.projects.view.runsToday", { n: String(stats.runsToday) })}
            </span>
            <span>{t("agents.projects.view.costToday", { cost: stats.costToday.toFixed(2) })}</span>
            <span>{t("agents.projects.view.costWeek", { cost: stats.costWeek.toFixed(2) })}</span>
          </div>
        {/if}
        {#if agents.length === 0}
          <p class="muted">{t("agents.projects.view.empty")}</p>
        {:else}
          <ProjectGraph {agents} entryId={project.entryAgentId} colorOf={agentColor} onselect={openAgent} />
          {#each projectWarnings(project, agents) as warning (warning.key + JSON.stringify(warning.params ?? {}))}
            <p class={warning.serious ? "warning" : "note"}>{warning.serious ? "⚠" : "ℹ"} {t(warning.key, warning.params)}</p>
          {/each}
          <div class="agent-list">
            {#each agents as agent (agent.id)}
              {@const state = agentState(agent, agentsStore.lastRun[agent.id] ?? null)}
              <button class="agent-row" onclick={() => openAgent(agent.id)}>
                <span class="dot" style="background:{toneColor(state.tone)}"></span>
                <span class="agent-name">{agent.icon || "🤖"} {agent.name}</span>
                <span class="muted small">{t(state.key, state.params)}</span>
              </button>
            {/each}
          </div>
        {/if}
        <button class="button add" onclick={newAgent}>+ {t("agents.projects.newAgentHere")}</button>
      {:else if tab === "activity"}
        {#each runs as item (item.id)}
          {@const agent = nameOf(item.agentId)}
          {@const age = ago(item.startedAt, agentsStore.now)}
          <button class="run-row" onclick={() => workspaceStore.openAgentRun(item.agentId, item.id, agent?.name ?? "")}>
            <span class="dot" style="background:{statusColor(item.status)}"></span>
            <span class="agent-name">{agent?.icon ?? "🤖"} {agent?.name ?? t("agents.run.deleted")}</span>
            <span class="muted small">
              {t(TRIGGER_KEYS[item.trigger])}{#if item.rehearsal}{" · "}{t("agents.run.rehearsal")}{/if}
            </span>
            <span class="muted small right">
              {t(age.key, age.params)}{#if item.endedAt}{" · "}{formatDuration(item.endedAt - item.startedAt)}{/if}{#if item.costUsd != null}{" · "}US$ {item.costUsd.toFixed(2)}{/if}
            </span>
          </button>
        {:else}
          <p class="muted">{t("agents.projects.view.activityEmpty")}</p>
        {/each}
      {:else if tab === "board"}
        {#if !project.isolated}
          <p class="muted">{t("agents.projects.view.boardNotTeam")}</p>
        {:else}
          {#each Object.entries(board) as [key, entry] (key)}
            {@const age = ago(entry.at, agentsStore.now)}
            <div class="note-card">
              <div class="note-head">
                <b>{key}</b>
                <span class="muted small">{t("agents.board.by", { agent: entry.by, age: t(age.key, age.params) })}</span>
                <button class="link" onclick={() => void removeNote(key)}>{t("agents.board.remove")}</button>
              </div>
              <div class="note-value">{entry.value}</div>
            </div>
          {:else}
            <p class="muted">{t("agents.board.empty")}</p>
          {/each}
        {/if}
      {:else if tab === "improve"}
        <p class="explain">{t("agents.projects.view.improve.intro")}</p>
        <div class="row">
          <button class="button primary" disabled={improving || agents.length === 0} onclick={() => void improve()}>
            ✨ {t("agents.projects.view.improve.button")}
          </button>
          {#if improving}<span class="muted working">{t("agents.projects.view.improve.thinking")}</span>{/if}
        </div>
        {#if improvement}
          <div class="ai-note">{improvement.summary}</div>
          {#each improvement.suggestions as suggestion, index (index)}
            {@const agent = nameOf(suggestion.agentId)}
            <div class="suggestion">
              <div class="suggestion-head">
                <b>{agent?.icon ?? "🤖"} {agent?.name ?? "?"}</b>
                <span class="muted small">{t(FIELD_KEYS[suggestion.field])}</span>
              </div>
              <p class="reason">{suggestion.reason}</p>
              <div class="diff">
                <div>
                  <div class="muted small">{t("agents.projects.view.improve.before")}</div>
                  <pre>{currentValue(agent, suggestion)}</pre>
                </div>
                <div>
                  <div class="muted small">{t("agents.projects.view.improve.after")}</div>
                  <pre>{shownValue(suggestion)}</pre>
                </div>
              </div>
              <div class="row">
                {#if applied.includes(index)}
                  <span class="muted">✓ {t("agents.projects.view.improve.applied")}</span>
                {:else}
                  <button class="button primary small" onclick={() => void apply(index, suggestion)}>
                    {t("agents.projects.view.improve.apply")}
                  </button>
                  <button class="button small" onclick={() => discard(index)}>{t("agents.projects.view.improve.discard")}</button>
                {/if}
              </div>
            </div>
          {:else}
            <p class="muted">{t("agents.projects.view.improve.none")}</p>
          {/each}
        {/if}
      {:else if tab === "versions"}
        <p class="explain">{t("agents.projects.view.versions.intro")}</p>
        {#each versions as version, index (version.at)}
          <div class="version-row">
            <span>{time(version.at)}</span>
            <span class="muted small grow">{version.agents.join(", ")}</span>
            {#if index === 0}
              <span class="muted small">{t("agents.projects.view.versions.current")}</span>
            {:else}
              <button class="button small" onclick={() => void restore(version.at)}>
                {confirmRestore === version.at
                  ? t("agents.projects.view.versions.confirm")
                  : t("agents.projects.view.versions.restore")}
              </button>
            {/if}
          </div>
        {:else}
          <p class="muted">{t("agents.projects.view.versions.none")}</p>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .project-view {
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

  .goal {
    font-size: 11.5px;
    opacity: 0.65;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }

  .tabs {
    display: flex;
    gap: 2px;
    padding: 0 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .tab {
    padding: 6px 10px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: inherit;
    font: inherit;
    opacity: 0.6;
    cursor: pointer;
  }

  .tab.current {
    border-bottom-color: var(--pane-active-border, #3a6df0);
    opacity: 1;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 14px 16px 24px;
    scrollbar-width: thin;
  }

  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    margin-bottom: 10px;
    font-size: 11.5px;
    opacity: 0.8;
  }

  .agent-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 12px;
  }

  .agent-row,
  .run-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .agent-row:hover,
  .run-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .agent-name {
    white-space: nowrap;
  }

  .right {
    margin-left: auto;
  }

  .grow {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
  }

  .muted {
    opacity: 0.6;
  }

  .small {
    font-size: 11px;
  }

  .center {
    margin: auto;
  }

  .working {
    animation: pulse 1.4s ease-in-out infinite;
  }

  .explain {
    margin: 0 0 10px;
    line-height: 1.5;
    opacity: 0.75;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
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

  .button.add {
    margin-top: 14px;
  }

  .link {
    margin-left: auto;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .error {
    margin: 0 16px 6px;
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }

  .warning,
  .note {
    margin: 6px 0 0;
    padding: 7px 10px;
    border-radius: 5px;
    font-size: 11.5px;
    line-height: 1.5;
  }

  .warning {
    border-left: 3px solid var(--warning-fg, #d7ba7d);
    background: rgba(215, 186, 125, 0.1);
  }

  .note {
    border-left: 3px solid var(--tabbar-border, #3a3a3a);
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.04));
  }

  .ai-note {
    margin: 12px 0;
    padding: 10px 12px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.12));
    line-height: 1.5;
  }

  .note-card,
  .suggestion {
    margin-bottom: 10px;
    padding: 8px 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
  }

  .note-head,
  .suggestion-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .note-value {
    margin-top: 4px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .reason {
    margin: 4px 0 8px;
    line-height: 1.5;
  }

  .diff {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  pre {
    margin: 2px 0 0;
    padding: 6px 8px;
    max-height: 180px;
    overflow: auto;
    border-radius: 5px;
    background: var(--tabbar-bg, rgba(0, 0, 0, 0.25));
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .version-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 4px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
