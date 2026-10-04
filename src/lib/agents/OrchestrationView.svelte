<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import type { AgentAutonomy, AgentEngine } from "$lib/core/agents-client";
  import type { Plan, PlannedAgent, PlannedTask } from "$lib/core/orchestration-client";
  import { AUTONOMY_TITLE_KEYS, type AgentTone } from "./agent-spec";
  import { agentsStore } from "./agents-store.svelte";
  import { orchestrationStore } from "./orchestration-store.svelte";
  import {
    ENGINE_NAMES,
    HOLD_KEYS,
    STATUS_KEYS,
    TASK_STATUS_KEYS,
    addAgent,
    addTask,
    agentHasRun,
    agentOf,
    agentTotals,
    canDependOn,
    canEditPlan,
    canResume,
    changeLabel,
    costRange,
    defaultBudget,
    dependencyTitles,
    formatTokens,
    formatUsd,
    isTaskEditable,
    modelsOf,
    pendingRevision,
    planTotals,
    progressOf,
    projectedUsd,
    removeAgent,
    removeTask,
    spentUsd,
    statusTone,
    taskEstimate,
    taskTone,
    tasksWithoutRequest,
    treeOf,
    withAgentFields,
    withInstructions,
    withModel,
    withTaskFields,
    withTaskRequest,
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
  /** Editing the plan of one that already ran (see canEditPlan): a copy of
   * it, saved with "Save changes". */
  let editing = $state(false);
  let editPlan = $state<Plan | null>(null);
  /** "Modify with AI": what the user asks for, while asking, and what the
   * orchestrator said about the plan it gave back. */
  let reviseOpen = $state(false);
  let reviseText = $state("");
  let revising = $state(false);
  let reviseNote = $state("");

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
    editing = false;
    editPlan = null;
  });

  // Started again (or resumed) from elsewhere: the edit no longer applies.
  $effect(() => {
    if (editing && orchestration && !canEditPlan(orchestration.status)) {
      editing = false;
      editPlan = null;
    }
  });

  // Once it runs, the budget shown is the saved one (a change the user
  // approved may have raised it) until the user types another to resume with.
  $effect(() => {
    if (orchestration && !isDraft && !budgetTouched) budget = orchestration.budgetUsd;
  });

  const plan = $derived<Plan>(
    !orchestration
      ? { agents: [], tasks: [] }
      : isDraft
        ? (edited[chosen] ?? orchestration.plan)
        : editing && editPlan
          ? editPlan
          : orchestration.plan,
  );
  /** Whether what the plan says can be changed here: a draft, or an edit. */
  const editable = $derived(isDraft || editing);
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
  /** The agents with a task running now: lit up in the diagram. */
  const working = $derived(new Set(plan.tasks.filter((task) => task.status === "running").map((task) => task.agent)));
  /** The agents that did every task they had: shown as done. */
  const finished = $derived(
    new Set(
      plan.agents
        .map((agent) => agent.key)
        .filter((key) => {
          const own = plan.tasks.filter((task) => task.agent === key && task.status !== "dropped");
          return own.length > 0 && own.every((task) => task.status === "done");
        }),
    ),
  );
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

  /** A change to the plan being reviewed: the draft's strategy, or the edit. */
  function update(next: Plan) {
    if (isDraft) {
      edited = { ...edited, [chosen]: next };
      followBudget();
    } else {
      editPlan = next;
    }
  }

  function setModel(agent: PlannedAgent, engine: AgentEngine, model: string) {
    update(withModel(plan, agent.key, engine, model));
  }

  function setInstructions(agent: PlannedAgent, instructions: string) {
    update(withInstructions(plan, agent.key, instructions));
  }

  function setTaskRequest(task: PlannedTask, description: string, doneWhen: string) {
    update(withTaskRequest(plan, task.key, description, doneWhen));
  }

  function startEditing() {
    if (!orchestration) return;
    editPlan = structuredClone($state.snapshot(orchestration.plan));
    editing = true;
    error = "";
  }

  function cancelEditing() {
    editing = false;
    editPlan = null;
    reviseNote = "";
    error = "";
  }

  /** A run would skip a task with nothing asked of it: said before it goes. */
  function checkRequests(): boolean {
    const missing = tasksWithoutRequest(plan);
    if (missing.length === 0) return true;
    error = t("agents.orchestration.edit.missingRequest", { tasks: missing.map((task) => task.title).join(", ") });
    selected = { kind: "task", key: missing[0].key };
    return false;
  }

  const saveEditing = () => {
    if (!checkRequests()) return;
    return act(async () => {
      await orchestrationStore.editPlan(orchestrationId, plan);
      editing = false;
      editPlan = null;
      reviseNote = "";
    });
  };

  // ---- Structure, by hand
  function newAgent() {
    const engine = engines[0] ?? "claude";
    const added = addAgent(plan, engine, t("agents.orchestration.edit.newAgentName"));
    update(added.plan);
    selected = { kind: "agent", key: added.key };
  }

  function dropAgent(agent: PlannedAgent) {
    update(removeAgent(plan, agent.key));
    selected = null;
  }

  function newTask(agent: PlannedAgent) {
    const added = addTask(plan, agent.key, t("agents.orchestration.edit.newTaskTitle"));
    update(added.plan);
    selected = { kind: "task", key: added.key };
  }

  function dropTask(task: PlannedTask) {
    update(removeTask(plan, task.key));
    selected = { kind: "agent", key: task.agent };
  }

  function setAgentFields(agent: PlannedAgent, fields: Parameters<typeof withAgentFields>[2]) {
    update(withAgentFields(plan, agent.key, fields));
  }

  function setTaskFields(task: PlannedTask, fields: Parameters<typeof withTaskFields>[2]) {
    update(withTaskFields(plan, task.key, fields));
  }

  function toggleDependency(task: PlannedTask, otherKey: string, on: boolean) {
    const dependsOn = on ? [...task.dependsOn, otherKey] : task.dependsOn.filter((key) => key !== otherKey);
    setTaskFields(task, { dependsOn });
  }

  // ---- Structure, with the orchestrator's help
  function openRevise() {
    reviseOpen = true;
    reviseNote = "";
  }

  async function revise() {
    const instructions = reviseText.trim();
    if (!instructions || revising) return;
    // After it ran, changes go on a copy saved with "Save changes".
    if (!isDraft && !editing) startEditing();
    revising = true;
    error = "";
    try {
      const revised = await orchestrationStore.revise(orchestrationId, plan, instructions);
      update(revised.plan);
      reviseNote = revised.note;
      reviseText = "";
      reviseOpen = false;
      selected = null;
    } catch (reason) {
      error = String(reason);
    } finally {
      revising = false;
    }
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

  const approve = () => {
    if (!checkRequests()) return;
    return act(() => orchestrationStore.approve(orchestrationId, chosen, plan, Math.max(0, budget || 0)));
  };
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
  {@const ran = agentHasRun(plan, agent.key)}
  <header class="detail-head">
    <span class="detail-icon">{agent.icon || "🤖"}</span>
    <div class="heading">
      <span class="name">{agent.name}</span>
      <span class="sub">{agent.specialty}</span>
    </div>
    {#if editable}
      <button class="button small" onclick={() => newTask(agent)}>+ {t("agents.orchestration.edit.addTask")}</button>
      <button
        class="button small danger-text"
        disabled={ran}
        title={ran ? t("agents.orchestration.edit.agentRan") : undefined}
        onclick={() => dropAgent(agent)}
      >
        {t("agents.orchestration.edit.removeAgent")}
      </button>
    {/if}
  </header>

  <div class="detail-grid">
    {#if editable}
      <div class="cell">
        <label class="label" for="agent-name">{t("agents.orchestration.edit.name")}</label>
        <input
          id="agent-name"
          class="field"
          maxlength="60"
          value={agent.name}
          oninput={(e) => setAgentFields(agent, { name: e.currentTarget.value })}
        />
      </div>
      <div class="cell span-2">
        <label class="label" for="agent-specialty">{t("agents.orchestration.detail.specialty")}</label>
        <input
          id="agent-specialty"
          class="field"
          maxlength="200"
          value={agent.specialty}
          oninput={(e) => setAgentFields(agent, { specialty: e.currentTarget.value })}
        />
      </div>
    {/if}
    {#if editable}
      <div class="cell">
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
      </div>
      <div class="cell">
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
      </div>
    {:else}
      <div class="cell">
        <div class="label">{t("agents.orchestration.detail.model")}</div>
        <p class="value">{modelLabel(agent)}</p>
      </div>
    {/if}

    {#if editable}
      <div class="cell">
        <label class="label" for="agent-autonomy">{t("agents.orchestration.detail.autonomy")}</label>
        <select
          id="agent-autonomy"
          class="field"
          value={agent.autonomy}
          onchange={(e) => setAgentFields(agent, { autonomy: e.currentTarget.value as AgentAutonomy })}
        >
          {#each ["read", "edit", "run"] as const as level (level)}
            <option value={level}>{t(AUTONOMY_TITLE_KEYS[level])}</option>
          {/each}
        </select>
        <label class="check">
          <input type="checkbox" checked={agent.web} onchange={(e) => setAgentFields(agent, { web: e.currentTarget.checked })} />
          {t("agents.orchestration.detail.web")}
        </label>
      </div>
    {:else}
      <div class="cell">
        <div class="label">{t("agents.orchestration.detail.autonomy")}</div>
        <p class="value">
          {t(AUTONOMY_TITLE_KEYS[agent.autonomy])}{#if agent.web}{" · "}{t("agents.orchestration.detail.web")}{/if}
        </p>
      </div>
    {/if}

    <div class="cell">
      <div class="label">{t("agents.orchestration.detail.cost")}</div>
      <p class="value">
        {agentCost(agent)}
        <span class="muted small">
          · {t("agents.orchestration.view.tokens", { in: formatTokens(own.tokensIn), out: formatTokens(own.tokensOut) })}
        </span>
      </p>
    </div>

    {#if agent.whyModel}
      <div class="cell wide">
        <div class="label">{t("agents.orchestration.detail.whyModel")}</div>
        <p class="value">{agent.whyModel}</p>
      </div>
    {/if}

    <div class="cell wide">
      <div class="label">{t("agents.orchestration.detail.tasks")}</div>
      <div class="chips">
        {#each plan.tasks.filter((task) => task.agent === agent.key) as task (task.key)}
          <button class="chip" onclick={() => (selected = { kind: "task", key: task.key })}>
            <span class="dot" style="background:{toneColor(taskTone(task.status))}"></span>
            <span class="grow">{task.title}</span>
            <span class="muted small">{estimateLabel(taskEstimate(plan, task, catalog))}</span>
          </button>
        {:else}
          <p class="muted small">{t("agents.orchestration.view.noTasks")}</p>
        {/each}
      </div>
    </div>

    {#if editable}
      <div class="cell wide">
        <label class="label" for="agent-instructions">{t("agents.orchestration.detail.instructions")}</label>
        <textarea
          id="agent-instructions"
          class="field area"
          rows="7"
          placeholder={t("agents.orchestration.detail.instructionsPlaceholder")}
          value={agent.instructions}
          oninput={(e) => setInstructions(agent, e.currentTarget.value)}
        ></textarea>
      </div>
    {:else if agent.instructions}
      <div class="cell wide">
        <div class="label">{t("agents.orchestration.detail.instructions")}</div>
        <pre>{agent.instructions}</pre>
      </div>
    {/if}
  </div>
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
    {#if task.runId && agent?.agentId}
      <button class="button small" onclick={() => openRun(task)}>{t("agents.orchestration.detail.openRun")}</button>
    {/if}
    {#if editable && isTaskEditable(task)}
      <button class="button small danger-text" onclick={() => dropTask(task)}>{t("agents.orchestration.edit.removeTask")}</button>
    {/if}
  </header>

  <div class="detail-grid">
    {#if editable && isTaskEditable(task)}
      <div class="cell">
        <label class="label" for="task-title">{t("agents.orchestration.edit.title")}</label>
        <input
          id="task-title"
          class="field"
          maxlength="120"
          value={task.title}
          oninput={(e) => setTaskFields(task, { title: e.currentTarget.value })}
        />
      </div>
      <div class="cell">
        <label class="label" for="task-agent">{t("agents.orchestration.detail.assignee")}</label>
        <select
          id="task-agent"
          class="field"
          value={task.agent}
          onchange={(e) => setTaskFields(task, { agent: e.currentTarget.value })}
        >
          {#each plan.agents as option (option.key)}
            <option value={option.key}>{option.icon || "🤖"} {option.name}</option>
          {/each}
        </select>
      </div>
      <div class="cell wide">
        <div class="label">{t("agents.orchestration.detail.dependsOn")}</div>
        <div class="chips">
          {#each plan.tasks.filter((other) => other.key !== task.key) as other (other.key)}
            {@const on = task.dependsOn.includes(other.key)}
            {@const allowed = on || canDependOn(plan, task.key, other.key)}
            <label class="chip check" class:disabled={!allowed} title={allowed ? undefined : t("agents.orchestration.edit.wouldLoop")}>
              <input
                type="checkbox"
                checked={on}
                disabled={!allowed}
                onchange={(e) => toggleDependency(task, other.key, e.currentTarget.checked)}
              />
              <span class="grow">{other.title}</span>
            </label>
          {:else}
            <p class="muted small">{t("agents.orchestration.edit.noOtherTasks")}</p>
          {/each}
        </div>
      </div>
    {:else if agent}
      <div class="cell">
        <div class="label">{t("agents.orchestration.detail.assignee")}</div>
        <button class="link" onclick={() => (selected = { kind: "agent", key: agent.key })}>
          {agent.icon || "🤖"} {agent.name} · {modelLabel(agent)}
        </button>
      </div>
    {/if}

    <div class="cell">
      <div class="label">{t("agents.orchestration.detail.cost")}</div>
      <p class="value">
        {estimateLabel(taskEstimate(plan, task, catalog))}
        <span class="muted small">
          · {t("agents.orchestration.view.tokens", { in: formatTokens(task.tokensIn), out: formatTokens(task.tokensOut) })}
        </span>
      </p>
    </div>

    {#if task.costUsd !== null}
      <div class="cell">
        <div class="label">{t("agents.orchestration.detail.realCost")}</div>
        <p class="value">{formatUsd(task.costUsd)}</p>
      </div>
    {/if}

    {#if after.length > 0 && !(editable && isTaskEditable(task))}
      <div class="cell">
        <div class="label">{t("agents.orchestration.detail.dependsOn")}</div>
        <p class="value">{after.join(" · ")}</p>
      </div>
    {/if}

    {#if editable && isTaskEditable(task)}
      <div class="cell wide">
        <label class="label" for="task-request">{t("agents.orchestration.detail.request")}</label>
        <textarea
          id="task-request"
          class="field area"
          rows="5"
          value={task.description}
          oninput={(e) => setTaskRequest(task, e.currentTarget.value, task.doneWhen)}
        ></textarea>
      </div>
      <div class="cell wide">
        <label class="label" for="task-done-when">{t("agents.orchestration.detail.doneWhen")}</label>
        <input
          id="task-done-when"
          class="field"
          value={task.doneWhen}
          oninput={(e) => setTaskRequest(task, task.description, e.currentTarget.value)}
        />
      </div>
    {:else}
      <div class="cell wide">
        <div class="label">{t("agents.orchestration.detail.request")}</div>
        <p class="value pre">{task.description}</p>
        {#if editable}
          <p class="muted small">{t("agents.orchestration.detail.requestLocked")}</p>
        {/if}
      </div>
      {#if task.doneWhen}
        <div class="cell wide">
          <div class="label">{t("agents.orchestration.detail.doneWhen")}</div>
          <p class="value">{task.doneWhen}</p>
        </div>
      {/if}
    {/if}

    {#if task.summary}
      <div class="cell wide">
        <div class="label">{t("agents.orchestration.detail.result")}</div>
        <pre>{task.summary}</pre>
      </div>
    {/if}
  </div>
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
        {#if !isDraft}
          <button
            class="button"
            onclick={() =>
              workspaceStore.openOrchestration(
                orchestration.id,
                t("agents.orchestration.console.tabTitle", { name: orchestration.name }),
                true,
              )}
          >
            {t("agents.orchestration.console.open")}
          </button>
        {/if}
        {#if (isDraft || canEditPlan(orchestration.status)) && !reviseOpen}
          <button class="button" disabled={busy || revising} onclick={openRevise}>
            ✨ {t("agents.orchestration.edit.modifyWithAi")}
          </button>
        {/if}
        {#if editing}
          <button class="button" disabled={busy} onclick={cancelEditing}>{t("agents.orchestration.view.cancelEdit")}</button>
          <button class="button primary" disabled={busy} onclick={() => void saveEditing()}>
            {t("agents.orchestration.view.saveEdit")}
          </button>
        {:else if canEditPlan(orchestration.status)}
          <button class="button" disabled={busy} onclick={startEditing}>{t("agents.orchestration.view.editPlan")}</button>
        {/if}
        {#if editing}
          <!-- Nothing else while editing: save or cancel first. -->
        {:else if orchestration.status === "running"}
          <button class="button" disabled={busy} onclick={() => void stop()}>{t("agents.orchestration.view.stop")}</button>
        {:else if canResume(orchestration.status)}
          <button class="button primary" disabled={busy} onclick={() => void resume()}>
            {t("agents.orchestration.view.resume")}
          </button>
        {/if}
        {#if orchestration.status !== "running" && !editing}
          <button class="button" class:danger={confirmDelete} disabled={busy} onclick={remove} onblur={() => (confirmDelete = false)}>
            {confirmDelete ? t("agents.orchestration.view.confirmDelete") : t("agents.orchestration.view.delete")}
          </button>
        {/if}
      </div>
    </header>
    {#if error}<p class="error">{error}</p>{/if}
    {#if editing}<p class="editing-note">{t("agents.orchestration.view.editingHint")}</p>{/if}
    {#if reviseOpen}
      <div class="revise">
        <label class="label" for="revise-text">{t("agents.orchestration.edit.reviseLabel")}</label>
        <textarea
          id="revise-text"
          class="field"
          rows="3"
          placeholder={t("agents.orchestration.edit.revisePlaceholder")}
          bind:value={reviseText}
          disabled={revising}
          onkeydown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void revise();
          }}
        ></textarea>
        <div class="row">
          <button class="button primary" disabled={revising || !reviseText.trim()} onclick={() => void revise()}>
            {revising ? t("agents.orchestration.edit.revising") : t("agents.orchestration.edit.reviseSend")}
          </button>
          <button class="button" disabled={revising} onclick={() => (reviseOpen = false)}>
            {t("agents.orchestration.view.cancelEdit")}
          </button>
          <span class="muted small">{t("agents.orchestration.edit.reviseHint")}</span>
        </div>
      </div>
    {/if}
    {#if reviseNote}
      <p class="ai-note revised">✨ {reviseNote}</p>
    {/if}

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
        <div class="diagram-pane">
          <h3 class="section">{t("agents.orchestration.view.tree")}</h3>
          <!-- An org chart: the orchestrator on top, its agents hanging from
               it, and each agent's tasks below it in the order they run. -->
          <div class="diagram-frame">
          {#if editable}
            <button class="button small add-agent" onclick={newAgent}>+ {t("agents.orchestration.edit.addAgent")}</button>
          {/if}
          <div class="diagram" style="--live:{themeStore.current.colors.blue};--done:{themeStore.current.colors.green}">
            <div class="org">
              <button
                class="box root"
                class:current={selected?.kind === "orchestrator"}
                onclick={() => (selected = { kind: "orchestrator" })}
              >
                <span class="box-icon">🧭</span>
                <span class="box-main">
                  <span class="box-title">{t("agents.orchestration.view.orchestrator")}</span>
                  <span class="box-sub">
                    {t("agents.orchestration.view.orchestratorModel", {
                      model: orchestration.orchestratorModel || t("agents.orchestration.view.defaultModel"),
                    })}
                  </span>
                </span>
              </button>
              {#if tree.length > 0}
                <div class="org-children">
                  {#each tree as branch (branch.agent.key)}
                    {@const current = selected?.kind === "agent" && selected.key === branch.agent.key}
                    {@const live = working.has(branch.agent.key)}
                    {@const done = !live && finished.has(branch.agent.key)}
                    <div class="org-branch">
                      <button
                        class="box agent"
                        class:current
                        class:live
                        class:done
                        onclick={() => (selected = { kind: "agent", key: branch.agent.key })}
                      >
                        <span class="box-icon">{branch.agent.icon || "🤖"}</span>
                        {#if done}<span class="check" title={t(TASK_STATUS_KEYS.done)}>✓</span>{/if}
                        <span class="box-main">
                          <span class="box-title">{branch.agent.name}</span>
                          {#if live}
                            <span class="box-sub activity">
                              {agentsStore.lastRun[branch.agent.agentId]?.activity || t("agents.run.working")}
                            </span>
                          {:else}
                            <span class="box-sub">{modelLabel(branch.agent)}</span>
                          {/if}
                          <span class="box-cost">{agentCost(branch.agent)}</span>
                        </span>
                      </button>
                      {#each branch.tasks as task (task.key)}
                        {@const taskCurrent = selected?.kind === "task" && selected.key === task.key}
                        {@const after = dependencyTitles(plan, task)}
                        <div class="task-link">
                          <button
                            class="box task"
                            class:current={taskCurrent}
                            class:dropped={task.status === "dropped"}
                            class:live={task.status === "running"}
                            class:done={task.status === "done"}
                            title={after.length > 0 ? t("agents.orchestration.view.after", { tasks: after.join(", ") }) : undefined}
                            onclick={() => (selected = { kind: "task", key: task.key })}
                          >
                            {#if task.status === "done"}
                              <span class="check small-check" title={t(TASK_STATUS_KEYS.done)}>✓</span>
                            {:else}
                              <span
                                class="dot"
                                class:working={task.status === "running"}
                                style="background:{toneColor(taskTone(task.status))}"
                                title={t(TASK_STATUS_KEYS[task.status])}
                              ></span>
                            {/if}
                            <span class="box-main">
                              <span class="box-title">{task.title}</span>
                              <span class="box-cost">
                                {task.costUsd !== null ? formatUsd(task.costUsd) : estimateLabel(taskEstimate(plan, task, catalog))}
                                {#if after.length > 0}<span class="after"> · ⤴ {after.length}</span>{/if}
                              </span>
                            </span>
                          </button>
                        </div>
                      {/each}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          </div>
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
            <div class="detail-grid">
              <div class="cell">
                <div class="label">{t("agents.orchestration.detail.model")}</div>
                <p class="value">
                  {t("agents.orchestration.view.orchestratorModel", {
                    model: orchestration.orchestratorModel || t("agents.orchestration.view.defaultModel"),
                  })}
                </p>
              </div>
              <div class="cell">
                <div class="label">{t("agents.orchestration.detail.realCost")}</div>
                <p class="value">{formatUsd(orchestration.overheadUsd)}</p>
              </div>
              <div class="cell wide">
                <div class="label">{t("agents.orchestration.detail.request")}</div>
                <p class="value pre">{orchestration.prompt}</p>
              </div>
            </div>
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
    font-size: 11px;
    line-height: 1.4;
    opacity: 0.75;
  }

  .badge {
    padding: 1px 7px;
    border-radius: 9px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 10.5px;
  }

  /* The diagram across the whole width, the detail of what's picked under
     it, laid out across too. */
  .columns {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 6px;
  }

  .orchestration-view {
    container-type: inline-size;
  }

  .diagram {
    min-height: 340px;
    max-height: 62vh;
    overflow: auto;
    padding: 18px 12px 22px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: rgba(128, 128, 128, 0.04);
    scrollbar-width: thin;
  }

  /* Centered while it fits, scrolls sideways when it doesn't. */
  .org {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: max-content;
    min-width: 100%;
    --line: var(--tabbar-border, #4a4a4a);
  }

  .org-children {
    position: relative;
    display: flex;
    padding-top: 20px;
  }

  /* From the orchestrator down to the bar its agents hang from. */
  .org-children::before {
    content: "";
    position: absolute;
    top: 0;
    left: 50%;
    height: 20px;
    border-left: 1px solid var(--line);
  }

  .org-branch {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 20px 8px 0;
  }

  /* The bar across the agents, and the drop to each one: half a bar on
     each side of its box, the ends rounded. */
  .org-branch::before,
  .org-branch::after {
    content: "";
    position: absolute;
    top: 0;
    width: 50%;
    height: 20px;
    border-top: 1px solid var(--line);
  }

  .org-branch::before {
    right: 50%;
  }

  .org-branch::after {
    left: 50%;
    border-left: 1px solid var(--line);
  }

  .org-branch:first-child::before,
  .org-branch:last-child::after {
    border-top: none;
  }

  .org-branch:last-child::before {
    border-right: 1px solid var(--line);
    border-radius: 0 6px 0 0;
  }

  .org-branch:first-child::after {
    border-radius: 6px 0 0 0;
  }

  .org-branch:only-child::before {
    display: none;
  }

  .org-branch:only-child::after {
    border-radius: 0;
  }

  /* Each task hangs from the box above it. */
  .task-link {
    position: relative;
    display: flex;
    justify-content: center;
    padding-top: 12px;
  }

  .task-link::before {
    content: "";
    position: absolute;
    top: 0;
    left: 50%;
    height: 12px;
    border-left: 1px solid var(--line);
  }

  .box {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--tabbar-border, #4a4a4a);
    border-radius: 8px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .box:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .box.current {
    border-color: var(--pane-active-border, #3a6df0);
    box-shadow: 0 0 0 2px rgba(58, 109, 240, 0.25);
  }

  /* Working now: lit in the running color, breathing. */
  .box.live {
    border-color: var(--live, #3a8ef0);
    border-style: solid;
    background: color-mix(in srgb, var(--live, #3a8ef0) 14%, var(--tabbar-active-bg-solid, #1e1e1e));
    animation: live-glow 1.8s ease-in-out infinite;
  }

  /* Did all it had to: green, with a tick. */
  .box.done {
    border-color: var(--done, #4ec27a);
    border-style: solid;
    background: color-mix(in srgb, var(--done, #4ec27a) 12%, var(--tabbar-active-bg-solid, #1e1e1e));
  }

  .check {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    order: 3;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--done, #4ec27a);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
  }

  .check.small-check {
    order: 0;
    width: 14px;
    height: 14px;
    font-size: 9px;
  }

  .activity {
    color: var(--live, #3a8ef0);
    opacity: 0.95;
  }

  @keyframes live-glow {
    0%,
    100% {
      box-shadow: 0 0 0 2px color-mix(in srgb, var(--live, #3a8ef0) 25%, transparent);
    }
    50% {
      box-shadow: 0 0 10px 3px color-mix(in srgb, var(--live, #3a8ef0) 45%, transparent);
    }
  }

  .box.root {
    min-width: 220px;
    padding: 10px 14px;
    border-width: 1.5px;
  }

  .box.root .box-title {
    font-weight: 600;
  }

  .box.agent {
    width: 200px;
  }

  .box.agent .box-title {
    font-weight: 600;
  }

  .box.task {
    width: 184px;
    padding: 6px 9px;
    border-style: dashed;
  }

  .box.dropped {
    opacity: 0.5;
    text-decoration: line-through;
  }

  .box-icon {
    font-size: 16px;
  }

  .box-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .box-title,
  .box-sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .box-sub,
  .box-cost {
    font-size: 11px;
    opacity: 0.65;
  }

  .after {
    opacity: 0.8;
  }

  .field.area {
    resize: vertical;
    min-height: 60px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
    line-height: 1.45;
  }

  .editing-note {
    margin: 6px 16px 0;
    padding: 6px 10px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
    border-radius: 4px;
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.12));
    font-size: 12px;
  }

  .ai-note.revised {
    margin: 8px 16px 0;
  }

  .diagram-frame {
    position: relative;
  }

  .add-agent {
    position: absolute;
    top: 8px;
    right: 10px;
    z-index: 1;
  }

  .revise {
    margin: 8px 16px 0;
    padding: 10px 12px;
    border: 1px solid var(--pane-active-border, #3a6df0);
    border-radius: 8px;
  }

  .revise .field {
    resize: vertical;
  }

  .cell.span-2 {
    grid-column: span 2;
  }

  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: 12px;
    cursor: pointer;
  }

  .chip.check {
    margin-top: 0;
  }

  .chip.disabled {
    opacity: 0.45;
    cursor: default;
  }

  .button.danger-text:not(:disabled) {
    color: var(--error-fg, #f48771);
  }

  .detail-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 2px 18px;
  }

  .cell {
    min-width: 0;
  }

  .cell.wide {
    grid-column: 1 / -1;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 320px;
    padding: 4px 9px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .chip:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .detail {
    padding: 12px 16px 14px;
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
