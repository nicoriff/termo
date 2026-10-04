<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import type { AgentEngine } from "$lib/core/agents-client";
  import type { Plan, PlannedAgent, PlannedTask } from "$lib/core/orchestration-client";
  import { AUTONOMY_TITLE_KEYS, type AgentTone } from "./agent-spec";
  import { orchestrationStore } from "./orchestration-store.svelte";
  import {
    ENGINE_NAMES,
    HOLD_KEYS,
    STATUS_KEYS,
    TASK_STATUS_KEYS,
    agentOf,
    agentTotals,
    canResume,
    changeLabel,
    costRange,
    defaultBudget,
    dependencyTitles,
    formatTokens,
    formatUsd,
    modelsOf,
    pendingRevision,
    planTotals,
    progressOf,
    projectedUsd,
    spentUsd,
    statusTone,
    taskEstimate,
    taskTone,
    treeOf,
    withModel,
  } from "./orchestration-spec";

  let { orchestrationId }: { orchestrationId: string } = $props();

  const orchestration = $derived(orchestrationStore.get(orchestrationId));
  const catalog = $derived(orchestrationStore.catalog);
  const isDraft = $derived(orchestration?.status === "draft");

  // A draft is edited here before it's approved: the strategy picked, the
  // model of each agent, the budget. Nothing is saved until "Approve".
  let chosen = $state("");
  let edited = $state<Record<string, Plan>>({});
  let budget = $state(0);
  /** Once the user types a budget, it stops following the estimate. */
  let budgetTouched = $state(false);
  let loadedFor = "";

  type Selection = { kind: "orchestrator" } | { kind: "agent"; key: string } | { kind: "task"; key: string };
  let selected = $state<Selection | null>(null);
  let error = $state("");
  let busy = $state(false);
  let confirmDelete = $state(false);

  $effect(() => {
    void orchestrationStore.loadCatalog();
  });

  $effect(() => {
    if (!orchestration || loadedFor === orchestration.id) return;
    loadedFor = orchestration.id;
    chosen = orchestration.chosen;
    edited = Object.fromEntries(
      orchestration.strategies.map((strategy) => [strategy.key, structuredClone($state.snapshot(strategy.plan))]),
    );
    budget = orchestration.budgetUsd;
    budgetTouched = false;
    selected = null;
  });

  // Once it runs, the budget shown is the saved one (a change the user
  // approved may have raised it) until the user types another to resume with.
  $effect(() => {
    if (orchestration && !isDraft && !budgetTouched) budget = orchestration.budgetUsd;
  });

  const plan = $derived<Plan>(
    orchestration ? (isDraft ? (edited[chosen] ?? orchestration.plan) : orchestration.plan) : { agents: [], tasks: [] },
  );
  const tree = $derived(treeOf(plan));
  const totals = $derived(planTotals(plan, catalog));
  const range = $derived(costRange(totals.usd));
  const progress = $derived(progressOf(plan));
  const pending = $derived(orchestration ? pendingRevision(orchestration) : null);
  const spent = $derived(orchestration ? spentUsd(plan) + orchestration.overheadUsd : 0);
  const projected = $derived(orchestration ? projectedUsd(plan, catalog) + orchestration.overheadUsd : 0);
  const selectedAgent = $derived.by(() => {
    const key = selected?.kind === "agent" ? selected.key : null;
    return key === null ? null : (plan.agents.find((agent) => agent.key === key) ?? null);
  });
  const selectedTask = $derived.by(() => {
    const key = selected?.kind === "task" ? selected.key : null;
    return key === null ? null : (plan.tasks.find((task) => task.key === key) ?? null);
  });
  const engines = $derived(catalog.filter((entry) => entry.installed).map((entry) => entry.engine));

  function toneColor(tone: AgentTone): string {
    const c = themeStore.current.colors;
    if (tone === "running") return c.blue;
    if (tone === "success") return c.green;
    if (tone === "error") return c.red;
    if (tone === "paused") return c.yellow;
    return c.brightBlack;
  }

  function modelLabel(agent: PlannedAgent): string {
    const name = modelsOf(catalog, agent.engine).find((option) => option.id === agent.model)?.name;
    return `${ENGINE_NAMES[agent.engine]} · ${name ?? (agent.model || t("agents.orchestration.detail.modelDefault"))}`;
  }

  function estimateLabel(usd: number | null): string {
    return usd === null ? t("agents.orchestration.view.noPrice") : `~${formatUsd(usd)}`;
  }

  function agentCost(agent: PlannedAgent): string {
    const own = agentTotals(plan, agent.key, catalog);
    return own.tasks > 0 && own.unpriced === own.tasks ? t("agents.orchestration.view.noPrice") : `~${formatUsd(own.usd)}`;
  }

  function followBudget() {
    if (!budgetTouched) budget = defaultBudget(plan, catalog);
  }

  function pickStrategy(key: string) {
    if (!isDraft || key === chosen) return;
    chosen = key;
    selected = null;
    followBudget();
  }

  function setModel(agent: PlannedAgent, engine: AgentEngine, model: string) {
    edited = { ...edited, [chosen]: withModel(plan, agent.key, engine, model) };
    followBudget();
  }

  /** Another engine: its first model, which each CLI lists as its main one. */
  function setEngine(agent: PlannedAgent, engine: AgentEngine) {
    setModel(agent, engine, modelsOf(catalog, engine)[0]?.id ?? "");
  }

  async function act(action: () => Promise<void>) {
    if (busy) return;
    busy = true;
    error = "";
    try {
      await action();
    } catch (reason) {
      error = String(reason);
    } finally {
      busy = false;
    }
  }

  const approve = () => act(() => orchestrationStore.approve(orchestrationId, chosen, plan, Math.max(0, budget || 0)));
  const stop = () => act(() => orchestrationStore.stop(orchestrationId));
  const resume = () => act(() => orchestrationStore.resume(orchestrationId, Math.max(0, budget || 0)));
  const resolve = (revisionId: string, approved: boolean) =>
    act(() => orchestrationStore.resolve(orchestrationId, revisionId, approved));

  function remove() {
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    void act(() => orchestrationStore.remove(orchestrationId));
  }

  function openRun(task: PlannedTask) {
    const agent = agentOf(plan, task);
    if (agent?.agentId && task.runId) workspaceStore.openAgentRun(agent.agentId, task.runId, agent.name);
  }
</script>

{#snippet agentDetail(agent: PlannedAgent)}
  {@const own = agentTotals(plan, agent.key, catalog)}
  <header class="detail-head">
    <span class="detail-icon">{agent.icon || "🤖"}</span>
    <div class="heading">
      <span class="name">{agent.name}</span>
      <span class="sub">{agent.specialty}</span>
    </div>
  </header>

  {#if isDraft}
    <label class="label" for="agent-engine">{t("agents.orchestration.detail.engine")}</label>
    <select
      id="agent-engine"
      class="field"
      value={agent.engine}
      onchange={(e) => setEngine(agent, e.currentTarget.value as AgentEngine)}
    >
      {#each engines as engine (engine)}
        <option value={engine}>{ENGINE_NAMES[engine]}</option>
      {/each}
      {#if !engines.includes(agent.engine)}
        <option value={agent.engine}>
          {t("agents.orchestration.detail.notInstalled", { engine: ENGINE_NAMES[agent.engine] })}
        </option>
      {/if}
    </select>
    <label class="label" for="agent-model">{t("agents.orchestration.detail.model")}</label>
    <select
      id="agent-model"
      class="field"
      value={agent.model}
      onchange={(e) => setModel(agent, agent.engine, e.currentTarget.value)}
    >
      <option value="">{t("agents.orchestration.detail.modelDefault")}</option>
      {#each modelsOf(catalog, agent.engine) as option (option.id)}
        <option value={option.id}>
          {option.name}{option.price ? ` — US$ ${option.price.input} / ${option.price.output}` : ""}
        </option>
      {/each}
    </select>
  {:else}
    <div class="label">{t("agents.orchestration.detail.model")}</div>
    <p class="value">{modelLabel(agent)}</p>
  {/if}

  {#if agent.whyModel}
    <div class="label">{t("agents.orchestration.detail.whyModel")}</div>
    <p class="value">{agent.whyModel}</p>
  {/if}

  <div class="label">{t("agents.orchestration.detail.autonomy")}</div>
  <p class="value">
    {t(AUTONOMY_TITLE_KEYS[agent.autonomy])}{#if agent.web}{" · "}{t("agents.orchestration.detail.web")}{/if}
  </p>

  <div class="label">{t("agents.orchestration.detail.cost")}</div>
  <p class="value">
    {agentCost(agent)}
    <span class="muted">
      · {t("agents.orchestration.view.tokens", { in: formatTokens(own.tokensIn), out: formatTokens(own.tokensOut) })}
    </span>
  </p>

  <div class="label">{t("agents.orchestration.detail.tasks")}</div>
  <div class="mini-list">
    {#each plan.tasks.filter((task) => task.agent === agent.key) as task (task.key)}
      <button class="mini-row" onclick={() => (selected = { kind: "task", key: task.key })}>
        <span class="dot" style="background:{toneColor(taskTone(task.status))}"></span>
        <span class="grow">{task.title}</span>
        <span class="muted small">{estimateLabel(taskEstimate(plan, task, catalog))}</span>
      </button>
    {:else}
      <p class="muted small">{t("agents.orchestration.view.noTasks")}</p>
    {/each}
  </div>

  {#if agent.instructions}
    <div class="label">{t("agents.orchestration.detail.instructions")}</div>
    <pre>{agent.instructions}</pre>
  {/if}
{/snippet}

{#snippet taskDetail(task: PlannedTask)}
  {@const agent = agentOf(plan, task)}
  {@const after = dependencyTitles(plan, task)}
  <header class="detail-head">
    <span class="dot big" style="background:{toneColor(taskTone(task.status))}"></span>
    <div class="heading">
      <span class="name">{task.title}</span>
      <span class="sub">
        {t(TASK_STATUS_KEYS[task.status])}{#if task.attempts > 1}{" · "}{t("agents.orchestration.view.attempt", {
            n: String(task.attempts),
          })}{/if}
      </span>
    </div>
  </header>

  {#if agent}
    <div class="label">{t("agents.orchestration.detail.assignee")}</div>
    <button class="link" onclick={() => (selected = { kind: "agent", key: agent.key })}>
      {agent.icon || "🤖"} {agent.name} · {modelLabel(agent)}
    </button>
  {/if}

  <div class="label">{t("agents.orchestration.detail.request")}</div>
  <p class="value pre">{task.description}</p>

  {#if task.doneWhen}
    <div class="label">{t("agents.orchestration.detail.doneWhen")}</div>
    <p class="value">{task.doneWhen}</p>
  {/if}

  {#if after.length > 0}
    <div class="label">{t("agents.orchestration.detail.dependsOn")}</div>
    <p class="value">{after.join(" · ")}</p>
  {/if}

  <div class="label">{t("agents.orchestration.detail.cost")}</div>
  <p class="value">
    {estimateLabel(taskEstimate(plan, task, catalog))}
    <span class="muted">
      · {t("agents.orchestration.view.tokens", { in: formatTokens(task.tokensIn), out: formatTokens(task.tokensOut) })}
    </span>
  </p>
  {#if task.costUsd !== null}
    <div class="label">{t("agents.orchestration.detail.realCost")}</div>
    <p class="value">{formatUsd(task.costUsd)}</p>
  {/if}

  {#if task.summary}
    <div class="label">{t("agents.orchestration.detail.result")}</div>
    <pre>{task.summary}</pre>
  {/if}
  {#if task.runId && agent?.agentId}
    <button class="button small spaced" onclick={() => openRun(task)}>{t("agents.orchestration.detail.openRun")}</button>
  {/if}
{/snippet}

<section class="orchestration-view">
  {#if !orchestration}
    <p class="muted center">{t("agents.orchestration.view.missing")}</p>
  {:else}
    <header class="header">
      <span class="header-icon">{orchestration.icon || "🧭"}</span>
      <div class="heading">
        <span class="name">{orchestration.name}</span>
        <span class="goal" title={orchestration.prompt}>{orchestration.prompt}</span>
      </div>
      <span class="status">
        <span class="dot" class:working={orchestration.status === "running"} style="background:{toneColor(statusTone(orchestration.status))}"
        ></span>
        {t(STATUS_KEYS[orchestration.status])}
        {#if !isDraft}
          <span class="muted">
            · {t("agents.orchestration.progress", { done: String(progress.done), total: String(progress.total) })}
          </span>
        {/if}
      </span>
      <div class="actions">
        {#if orchestration.status === "running"}
          <button class="button" disabled={busy} onclick={() => void stop()}>{t("agents.orchestration.view.stop")}</button>
        {:else if canResume(orchestration.status)}
          <button class="button primary" disabled={busy} onclick={() => void resume()}>
            {t("agents.orchestration.view.resume")}
          </button>
        {/if}
        {#if orchestration.status !== "running"}
          <button class="button" class:danger={confirmDelete} disabled={busy} onclick={remove} onblur={() => (confirmDelete = false)}>
            {confirmDelete ? t("agents.orchestration.view.confirmDelete") : t("agents.orchestration.view.delete")}
          </button>
        {/if}
      </div>
    </header>
    {#if error}<p class="error">{error}</p>{/if}

    <div class="body">
      {#if orchestration.note && !isDraft}
        <p class="ai-note">{orchestration.note}</p>
      {/if}

      {#if pending}
        <div class="revision pending">
          <div class="revision-head">
            <strong>{t("agents.orchestration.revisions.pending")}</strong>
            {#if pending.hold !== "none"}<span class="muted">· {t(HOLD_KEYS[pending.hold])}</span>{/if}
          </div>
          {#if pending.note}<p class="reason">{pending.note}</p>{/if}
          <ul class="changes">
            {#each pending.changes as change, index (index)}
              {@const label = changeLabel(change, plan)}
              <li>{t(label.key, label.params)}</li>
            {/each}
          </ul>
          <p class="muted small">
            {t("agents.orchestration.revisions.estimate", { cost: formatUsd(pending.estimateUsd) })}
          </p>
          <div class="row">
            <button class="button primary" disabled={busy} onclick={() => void resolve(pending.id, true)}>
              {t("agents.orchestration.revisions.approve")}
            </button>
            <button class="button" disabled={busy} onclick={() => void resolve(pending.id, false)}>
              {t("agents.orchestration.revisions.reject")}
            </button>
          </div>
        </div>
      {/if}

      {#if isDraft}
        <h3 class="section">{t("agents.orchestration.view.strategies")}</h3>
        <div class="strategies" role="radiogroup" aria-label={t("agents.orchestration.view.strategies")}>
          {#each orchestration.strategies as strategy (strategy.key)}
            {@const own = planTotals(edited[strategy.key] ?? strategy.plan, catalog)}
            <button
              class="strategy"
              class:current={strategy.key === chosen}
              role="radio"
              aria-checked={strategy.key === chosen}
              onclick={() => pickStrategy(strategy.key)}
            >
              <span class="strategy-head">
                <span class="strategy-name">{strategy.name}</span>
                {#if strategy.key === orchestration.recommended}
                  <span class="badge">{t("agents.orchestration.view.recommended")}</span>
                {/if}
              </span>
              <span class="strategy-cost">~{formatUsd(own.usd)}</span>
              <span class="muted small">
                {t("agents.orchestration.view.agents", { n: String((edited[strategy.key] ?? strategy.plan).agents.length) })}
                · {t("agents.orchestration.view.tasks", { n: String(own.tasks) })}
                · {t("agents.orchestration.view.tokensTotal", { n: formatTokens(own.tokensIn + own.tokensOut) })}
              </span>
              <span class="strategy-text">{strategy.summary}</span>
            </button>
          {/each}
        </div>
        {#if orchestration.why}
          <p class="ai-note">{orchestration.why}</p>
        {/if}
      {/if}

      <div class="columns">
        <div class="tree-pane">
          <h3 class="section">{t("agents.orchestration.view.tree")}</h3>
          <ul class="tree" role="tree">
            <li role="treeitem" aria-selected={selected?.kind === "orchestrator"}>
              <button class="node root" class:current={selected?.kind === "orchestrator"} onclick={() => (selected = { kind: "orchestrator" })}>
                <span class="node-icon">🧭</span>
                <span class="node-main">
                  <span class="node-title">{t("agents.orchestration.view.orchestrator")}</span>
                  <span class="node-sub">
                    {t("agents.orchestration.view.orchestratorModel", {
                      model: orchestration.orchestratorModel || t("agents.orchestration.view.defaultModel"),
                    })}
                  </span>
                </span>
              </button>
              <ul role="group">
                {#each tree as branch (branch.agent.key)}
                  {@const current = selected?.kind === "agent" && selected.key === branch.agent.key}
                  <li role="treeitem" aria-selected={current}>
                    <button class="node" class:current onclick={() => (selected = { kind: "agent", key: branch.agent.key })}>
                      <span class="node-icon">{branch.agent.icon || "🤖"}</span>
                      <span class="node-main">
                        <span class="node-title">{branch.agent.name}</span>
                        <span class="node-sub">{modelLabel(branch.agent)}</span>
                      </span>
                      <span class="node-cost">{agentCost(branch.agent)}</span>
                    </button>
                    <ul role="group">
                      {#each branch.tasks as task (task.key)}
                        {@const taskCurrent = selected?.kind === "task" && selected.key === task.key}
                        {@const after = dependencyTitles(plan, task)}
                        <li role="treeitem" aria-selected={taskCurrent}>
                          <button
                            class="node task"
                            class:current={taskCurrent}
                            class:dropped={task.status === "dropped"}
                            onclick={() => (selected = { kind: "task", key: task.key })}
                          >
                            <span
                              class="dot"
                              class:working={task.status === "running"}
                              style="background:{toneColor(taskTone(task.status))}"
                              title={t(TASK_STATUS_KEYS[task.status])}
                            ></span>
                            <span class="node-main">
                              <span class="node-title">{task.title}</span>
                              {#if after.length > 0}
                                <span class="node-sub">{t("agents.orchestration.view.after", { tasks: after.join(", ") })}</span>
                              {/if}
                            </span>
                            <span class="node-cost">
                              {task.costUsd !== null ? formatUsd(task.costUsd) : estimateLabel(taskEstimate(plan, task, catalog))}
                            </span>
                          </button>
                        </li>
                      {/each}
                    </ul>
                  </li>
                {/each}
              </ul>
            </li>
          </ul>
        </div>

        <aside class="detail">
          {#if selectedAgent}
            {@render agentDetail(selectedAgent)}
          {:else if selectedTask}
            {@render taskDetail(selectedTask)}
          {:else if selected?.kind === "orchestrator"}
            <header class="detail-head">
              <span class="detail-icon">🧭</span>
              <div class="heading">
                <span class="name">{t("agents.orchestration.view.orchestrator")}</span>
                <span class="sub">{t("agents.orchestration.view.orchestratorRole")}</span>
              </div>
            </header>
            <div class="label">{t("agents.orchestration.detail.model")}</div>
            <p class="value">
              {t("agents.orchestration.view.orchestratorModel", {
                model: orchestration.orchestratorModel || t("agents.orchestration.view.defaultModel"),
              })}
            </p>
            <div class="label">{t("agents.orchestration.detail.realCost")}</div>
            <p class="value">{formatUsd(orchestration.overheadUsd)}</p>
            <div class="label">{t("agents.orchestration.detail.request")}</div>
            <p class="value pre">{orchestration.prompt}</p>
          {:else}
            <p class="muted">{t("agents.orchestration.view.selectHint")}</p>
          {/if}
        </aside>
      </div>

      {#if orchestration.revisions.some((revision) => revision.state !== "pending")}
        <h3 class="section">{t("agents.orchestration.revisions.title")}</h3>
        {#each orchestration.revisions.filter((revision) => revision.state !== "pending").toReversed() as revision (revision.id)}
          <div class="revision">
            <div class="revision-head">
              <strong>
                {revision.state === "applied"
                  ? t("agents.orchestration.revisions.applied")
                  : t("agents.orchestration.revisions.rejected")}
              </strong>
              <span class="muted small">· {new Date(revision.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
            </div>
            {#if revision.note}<p class="reason">{revision.note}</p>{/if}
            <ul class="changes">
              {#each revision.changes as change, index (index)}
                {@const label = changeLabel(change, plan)}
                <li>{t(label.key, label.params)}</li>
              {/each}
            </ul>
          </div>
        {/each}
      {/if}
    </div>

    <footer class="summary">
      <div class="figures">
        <span class="figure">
          <span class="figure-label">{t("agents.orchestration.view.estimate")}</span>
          <span class="figure-value">~{formatUsd(totals.usd)}</span>
          <span class="muted small">
            {t("agents.orchestration.view.estimateRange", { low: formatUsd(range.low), high: formatUsd(range.high) })}
          </span>
        </span>
        <span class="figure">
          <span class="figure-label">{t("agents.orchestration.view.tokensLabel")}</span>
          <span class="figure-value">{formatTokens(totals.tokensIn + totals.tokensOut)}</span>
          <span class="muted small">
            {t("agents.orchestration.view.tokens", { in: formatTokens(totals.tokensIn), out: formatTokens(totals.tokensOut) })}
          </span>
        </span>
        {#if !isDraft}
          <span class="figure">
            <span class="figure-label">{t("agents.orchestration.view.spent")}</span>
            <span class="figure-value">{formatUsd(spent)}</span>
            <span class="muted small">
              {t("agents.orchestration.view.overhead")}: {formatUsd(orchestration.overheadUsd)}
            </span>
          </span>
          <span class="figure">
            <span class="figure-label">{t("agents.orchestration.view.projected")}</span>
            <span class="figure-value">~{formatUsd(projected)}</span>
          </span>
        {/if}
        <span class="figure">
          <label class="figure-label" for="orchestration-budget">{t("agents.orchestration.view.budget")}</label>
          {#if isDraft || canResume(orchestration.status)}
            <input
              id="orchestration-budget"
              class="field budget"
              type="number"
              min="0"
              step="0.5"
              bind:value={budget}
              oninput={() => (budgetTouched = true)}
            />
            <span class="muted small">{t("agents.orchestration.view.budgetHint")}</span>
          {:else}
            <span class="figure-value">{orchestration.budgetUsd > 0 ? formatUsd(orchestration.budgetUsd) : "—"}</span>
          {/if}
        </span>
        {#if isDraft}
          <button class="button primary approve" disabled={busy || plan.tasks.length === 0} onclick={() => void approve()}>
            {t("agents.orchestration.view.approve")}
          </button>
        {/if}
      </div>
      <p class="muted small fine">
        {t("agents.orchestration.view.approx")}{#if totals.unpriced > 0}{" "}{t("agents.orchestration.view.unpriced", {
            n: String(totals.unpriced),
          })}{/if}
      </p>
    </footer>
  {/if}
</section>

<style>
  .orchestration-view {
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

  .header-icon,
  .detail-icon {
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

  .goal,
  .sub {
    font-size: 11.5px;
    opacity: 0.65;
  }

  .goal {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 6px;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px 16px 20px;
    scrollbar-width: thin;
  }

  .section {
    margin: 14px 0 8px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    opacity: 0.6;
  }

  .section:first-child {
    margin-top: 0;
  }

  .strategies {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    gap: 10px;
  }

  .strategy {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .strategy:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .strategy.current {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.12));
  }

  .strategy-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .strategy-name {
    font-weight: 600;
  }

  .strategy-cost {
    font-size: 16px;
    font-weight: 600;
  }

  .strategy-text {
    margin-top: 2px;
    line-height: 1.45;
    opacity: 0.8;
  }

  .badge {
    padding: 1px 7px;
    border-radius: 9px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 10.5px;
  }

  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
    gap: 18px;
    margin-top: 6px;
  }

  @container (max-width: 640px) {
    .columns {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  .orchestration-view {
    container-type: inline-size;
  }

  .tree,
  .tree ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* The lines of the tree: each group hangs from its parent by a vertical
     rule, and each item reaches it with a short horizontal one. */
  .tree ul {
    margin-left: 16px;
    padding-left: 12px;
    border-left: 1px solid var(--tabbar-border, #3a3a3a);
  }

  .tree ul > li {
    position: relative;
  }

  .tree ul > li::before {
    content: "";
    position: absolute;
    left: -12px;
    top: 17px;
    width: 10px;
    border-top: 1px solid var(--tabbar-border, #3a3a3a);
  }

  .node {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    margin: 2px 0;
    padding: 5px 8px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .node:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .node.current {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.12));
  }

  .node.root .node-title {
    font-weight: 600;
  }

  .node.dropped {
    opacity: 0.5;
    text-decoration: line-through;
  }

  .node-icon {
    font-size: 15px;
  }

  .node-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .node-title,
  .node-sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .node-sub {
    font-size: 11px;
    opacity: 0.6;
  }

  .node-cost {
    font-size: 11px;
    opacity: 0.75;
    white-space: nowrap;
  }

  .detail {
    align-self: start;
    padding: 12px 14px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
  }

  .detail-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 4px;
  }

  .label {
    display: block;
    margin: 12px 0 4px;
    font-size: 11px;
    font-weight: 600;
    opacity: 0.6;
  }

  .value {
    margin: 0;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .value.pre {
    white-space: pre-wrap;
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
    color: inherit;
    font: inherit;
    outline: none;
  }

  .field:focus {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .field.budget {
    width: 92px;
    padding: 3px 6px;
  }

  .mini-list {
    display: flex;
    flex-direction: column;
  }

  .mini-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .mini-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
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

  .dot.big {
    width: 10px;
    height: 10px;
  }

  .working {
    animation: pulse 1.4s ease-in-out infinite;
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

  pre {
    margin: 0;
    padding: 6px 8px;
    max-height: 220px;
    overflow: auto;
    border-radius: 5px;
    background: var(--tabbar-bg, rgba(0, 0, 0, 0.25));
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .ai-note {
    margin: 10px 0;
    padding: 10px 12px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.12));
    line-height: 1.5;
  }

  .revision {
    margin-bottom: 10px;
    padding: 8px 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
  }

  .revision.pending {
    border-color: var(--warning-fg, #d7ba7d);
    background: rgba(215, 186, 125, 0.08);
  }

  .revision-head {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }

  .reason {
    margin: 4px 0;
    line-height: 1.5;
  }

  .changes {
    margin: 4px 0;
    padding-left: 18px;
    line-height: 1.6;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }

  .summary {
    padding: 10px 16px 12px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .figures {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 22px;
  }

  .figure {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .figure-label {
    font-size: 11px;
    opacity: 0.6;
  }

  .figure-value {
    font-size: 15px;
    font-weight: 600;
  }

  .approve {
    margin-left: auto;
    padding: 7px 16px;
  }

  .fine {
    margin: 8px 0 0;
    line-height: 1.45;
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

  .button.spaced {
    margin-top: 10px;
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

  .button.danger {
    border-color: var(--error-fg, #f48771);
    color: var(--error-fg, #f48771);
  }

  .link {
    padding: 0;
    border: none;
    background: transparent;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .error {
    margin: 6px 16px 0;
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
