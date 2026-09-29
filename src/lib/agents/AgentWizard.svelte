<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import {
    deleteAgent,
    draftAgent,
    agentsWebhook,
    forgetAgentSecrets,
    missingSecrets,
    setAgentSecret,
    onAgentRunEvent,
    agentModels,
    type AgentAutonomy,
    type AgentDraft,
    type AgentEngine,
    type McpServer,
    type ModelOption,
    type RunEvent,
  } from "$lib/core/agents-client";
  import FolderField from "./FolderField.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import IntegrationPicker from "./IntegrationPicker.svelte";
  import CustomConnectorForm from "./CustomConnectorForm.svelte";
  import GithubDeployPanel from "./GithubDeployPanel.svelte";
  import { AGENT_TEMPLATES, applyTemplate, type AgentTemplate } from "./agent-templates";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import { sessionFolder, workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { collectTiles } from "$lib/layout/split-tree";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import type { MessageKey } from "$lib/i18n/translate";
  import { agentsStore } from "./agents-store.svelte";
  import {
    AGENT_ICONS,
    STEP_LABEL_KEYS,
    applyDraft,
    secretsOf,
    WIZARD_STEPS,
    newAgentSpec,
    specProblems,
    stepProblems,
    strongerContacts,
    withoutEchoes,
    type WizardStep,
  } from "./agent-spec";
  import { renderMarkdownLite } from "./markdown-lite";

  const SLIDE_PX = 60;
  const SLIDE_MS = 170;

  const target = uiState.agentWizard!;
  const isNew = target.isNew;
  let spec = $state(target.spec ? structuredClone($state.snapshot(target.spec)) : newAgentSpec(crypto.randomUUID()));

  let stepIndex = $state(0);
  let direction = $state(1);
  const step = $derived<WizardStep>(WIZARD_STEPS[stepIndex]);
  let showProblems = $state(false);
  const problems = $derived(stepProblems(step, spec));
  let saving = $state(false);
  let saveError = $state("");
  let dialogEl: HTMLDivElement | undefined = $state();

  // The test run (step "test").
  let testRunId = $state<string | null>(null);
  let testEvents = $state<RunEvent[]>([]);
  let testError = $state("");
  let testedOnce = false;
  const testRun = $derived(testRunId ? agentsStore.lastRun[spec.id] : null);
  const testRunning = $derived(testRun?.id === testRunId && testRun?.status === "running");

  /** The folder the focused console is in, offered as the working folder. */
  const hereFolder = $derived.by(() => {
    const root = workspaceStore.root;
    if (!root) return null;
    const tile = collectTiles(root).find((candidate) => candidate.id === workspaceStore.focusedTileId);
    const active = tile?.sessions.find((session) => session.id === tile.activeSessionId);
    return active ? (sessionFolder(active) ?? null) : null;
  });

  const ENGINES: { id: AgentEngine; label: string; hint: MessageKey }[] = [
    { id: "claude", label: "Claude Code", hint: "agents.wizard.engine.claudeHint" },
    { id: "codex", label: "Codex", hint: "agents.wizard.engine.codexHint" },
  ];

  const AUTONOMY: { id: AgentAutonomy; title: MessageKey; text: MessageKey }[] = [
    { id: "read", title: "agents.wizard.autonomy.readTitle", text: "agents.wizard.autonomy.readText" },
    { id: "edit", title: "agents.wizard.autonomy.editTitle", text: "agents.wizard.autonomy.editText" },
    { id: "run", title: "agents.wizard.autonomy.runTitle", text: "agents.wizard.autonomy.runText" },
  ];

  const WEEKDAY_KEYS: MessageKey[] = [
    "agents.wizard.triggers.days.sun",
    "agents.wizard.triggers.days.mon",
    "agents.wizard.triggers.days.tue",
    "agents.wizard.triggers.days.wed",
    "agents.wizard.triggers.days.thu",
    "agents.wizard.triggers.days.fri",
    "agents.wizard.triggers.days.sat",
  ];

  const installed = (engine: AgentEngine) => newTabOptions.agentClis?.[engine] !== false;

  /** The models each engine's CLI offers (loaded once; missing until then). */
  let models = $state<Partial<Record<AgentEngine, ModelOption[]>>>({});
  for (const engine of ["claude", "codex"] as const) {
    void agentModels(engine)
      .then((list) => (models = { ...models, [engine]: list }))
      .catch(() => (models = { ...models, [engine]: [] }));
  }
  const engineModels = $derived(models[spec.engine] ?? []);

  function modelLabel(model: ModelOption): string {
    return model.note ? `${model.name} · ${model.note}` : model.name;
  }

  /** A model belongs to its engine: switching engines drops one the other
   * doesn't have. */
  function chooseEngine(engine: AgentEngine) {
    spec.engine = engine;
    const list = models[engine];
    if (spec.model && list && !list.some((model) => model.id === spec.model)) spec.model = "";
  }

  /** The other agents: contacts to choose (step "tools") and ones to
   * follow (step "triggers"). In a team, only its own members; outside,
   * nobody who's in a team. */
  const teams = $derived(new Set(agentsStore.projects.filter((project) => project.isolated).map((project) => project.id)));
  const inTeam = $derived(teams.has(spec.projectId));
  const others = $derived(
    agentsStore.agents.filter(
      (other) =>
        other.id !== spec.id &&
        (inTeam || teams.has(other.projectId) ? other.projectId === spec.projectId : true),
    ),
  );
  const stronger = $derived(strongerContacts(spec, others));
  const AUTONOMY_TITLE = Object.fromEntries(AUTONOMY.map((level) => [level.id, level.title])) as Record<
    AgentAutonomy,
    MessageKey
  >;

  function toggleContact(id: string, on: boolean) {
    const contacts = spec.collaboration.contacts.filter((contact) => contact !== id);
    spec.collaboration.contacts = on ? [...contacts, id] : contacts;
  }

  // "Build it with AI" (step "idea").
  let drafting = $state(false);
  let draftError = $state("");
  /** The AI's word on its choices, shown until the user moves on. */
  let draftNote = $state("");
  let suggestions = $state<AgentDraft["integrations"]>([]);
  /** The agent's secrets with no value on this machine (an imported agent),
   * and what's being typed for each. */
  let missing = $state<string[]>([]);
  let secretDrafts = $state<Record<string, string>>({});
  void missingSecrets(spec.id, $state.snapshot(spec.secrets)).then((names) => (missing = names)).catch(() => {});

  async function storeSecret(name: string) {
    const value = secretDrafts[name];
    if (!value) return;
    try {
      await setAgentSecret(spec.id, name, value);
      missing = missing.filter((other) => other !== name);
      delete secretDrafts[name];
    } catch (reason) {
      saveError = String(reason);
    }
  }

  /** The local webhook (step "triggers"): its address and a ready-made call. */
  let webhookInfo = $state<{ port: number; listening: boolean } | null>(null);
  void agentsWebhook()
    .then((info) => (webhookInfo = info))
    .catch(() => {});
  let copiedCurl = $state(false);
  const curlExample = $derived(
    spec.triggers.webhook
      ? `curl -X POST -H "Authorization: Bearer ${spec.triggers.webhook.token}" -d "…" http://127.0.0.1:${webhookInfo?.port ?? 47821}/agents/${spec.id}/run`
      : "",
  );

  /** A webhook token: long and random, like a password. */
  function newToken(): string {
    return crypto.randomUUID().replaceAll("-", "") + crypto.randomUUID().replaceAll("-", "");
  }

  async function copyCurl() {
    await writeText(curlExample).catch(() => {});
    copiedCurl = true;
    setTimeout(() => (copiedCurl = false), 1500);
  }

  /** Text from outside (a commit, a webhook call, a web page) reaching an
   * agent that can run commands: the combination worth a warning. */
  const outsideRisk = $derived(
    spec.autonomy === "run" &&
      Boolean(spec.triggers.git || spec.triggers.webhook || spec.triggers.after || spec.tools.web),
  );

  /** Which way of adding an integration is open, if any. */
  let picking = $state<"registry" | "custom" | null>(null);
  /** The GitHub Actions panel of the last step. */
  let showGithub = $state(Boolean(target.spec?.deploy.github));
  /** Every secret stored while the wizard is open, kept or not. */
  const addedSecrets: string[] = [];

  async function buildWithAi() {
    if (!spec.description.trim() || drafting) return;
    drafting = true;
    draftError = "";
    try {
      const draft = await draftAgent(spec.description.trim(), spec.workDir);
      applyDraft(spec, draft);
      suggestions = draft.integrations;
      draftNote = draft.explanation;
      direction = 1;
      stepIndex = WIZARD_STEPS.indexOf("logic");
    } catch (reason) {
      draftError = String(reason);
    } finally {
      drafting = false;
    }
  }

  function useTemplate(chosen: AgentTemplate) {
    applyTemplate(spec, chosen, (key) => t(key), hereFolder);
    draftNote = "";
    direction = 1;
    stepIndex = WIZARD_STEPS.indexOf("logic");
  }

  function addIntegration(server: McpServer, secrets: string[]) {
    spec.tools.mcp = [...spec.tools.mcp.filter((existing) => existing.name !== server.name), server];
    spec.secrets = [...new Set([...spec.secrets, ...secrets])];
    addedSecrets.push(...secrets);
    picking = null;
  }

  function removeIntegration(name: string) {
    spec.tools.mcp = spec.tools.mcp.filter((server) => server.name !== name);
    // Only dropped from the keychain once the agent is saved without it.
    const gone = secretsOf(name, spec.secrets);
    spec.secrets = spec.secrets.filter((secret) => !gone.includes(secret));
  }

  /** The secrets the agent had when the wizard opened. */
  const savedSecrets = [...(target.spec?.secrets ?? [])];

  onMount(() => {
    newTabOptions.refresh();
    void tick().then(() => dialogEl?.querySelector<HTMLElement>("textarea, input")?.focus());
    let unlisten: (() => void) | null = null;
    void onAgentRunEvent((stepEvent) => {
      if (stepEvent.agentId !== spec.id || stepEvent.runId !== testRunId) return;
      testEvents = [...testEvents, stepEvent.event];
    }).then((stop) => (unlisten = stop));
    return () => unlisten?.();
  });

  function go(index: number) {
    if (index > stepIndex) {
      // Forward only past a complete step; back and to done steps always.
      for (let i = stepIndex; i < index; i++) {
        if (stepProblems(WIZARD_STEPS[i], spec).length > 0) {
          stepIndex = i;
          showProblems = true;
          return;
        }
      }
    }
    // Moving on from the idea: its description seeds the task.
    if (step === "idea" && !spec.task.trim() && spec.description.trim()) spec.task = spec.description.trim();
    direction = index > stepIndex ? 1 : -1;
    showProblems = false;
    if (step !== "idea") draftNote = "";
    stepIndex = index;
    void tick().then(() => dialogEl?.querySelector<HTMLElement>(".step-body textarea, .step-body input")?.focus());
  }

  function next() {
    if (problems.length > 0) {
      showProblems = true;
      return;
    }
    if (stepIndex < WIZARD_STEPS.length - 1) go(stepIndex + 1);
  }

  async function close() {
    if (testRunning) await agentsStore.stop(spec.id);
    // A new agent that was only tried out leaves no trace.
    if (isNew && testedOnce && !agentsStore.agents.some((agent) => agent.id === spec.id)) {
      void deleteAgent(spec.id).catch(() => {});
    }
    // Secrets stored during this session that nothing saved refers to.
    const created = [...new Set([...spec.secrets, ...addedSecrets])].filter((name) => !savedSecrets.includes(name));
    void forgetAgentSecrets(spec.id, created).catch(() => {});
    uiState.closeAgentWizard();
  }

  async function save() {
    const missing = specProblems(spec);
    if (missing.length > 0) {
      const at = WIZARD_STEPS.findIndex((candidate) => stepProblems(candidate, spec).length > 0);
      direction = -1;
      stepIndex = at;
      showProblems = true;
      return;
    }
    saving = true;
    saveError = "";
    try {
      spec.name = spec.name.trim();
      await agentsStore.save(spec);
      const unused = [...new Set([...savedSecrets, ...addedSecrets])].filter((name) => !spec.secrets.includes(name));
      void forgetAgentSecrets(spec.id, unused).catch(() => {});
      agentsStore.setOpen(true);
      uiState.closeAgentWizard();
    } catch (reason) {
      saveError = String(reason);
    } finally {
      saving = false;
    }
  }

  async function test() {
    testError = "";
    testEvents = [];
    try {
      const started = await agentsStore.run(spec, "test");
      testedOnce = true;
      testRunId = started.id;
    } catch (reason) {
      testError = String(reason);
    }
  }

  function setSchedule(kind: "none" | "interval" | "daily") {
    if (kind === "none") spec.triggers.schedule = null;
    else if (kind === "interval") spec.triggers.schedule = { kind: "interval", minutes: 60 };
    else spec.triggers.schedule = { kind: "daily", at: "09:00", weekdays: [] };
  }

  function toggleDay(day: number) {
    const schedule = spec.triggers.schedule;
    if (schedule?.kind !== "daily") return;
    schedule.weekdays = schedule.weekdays.includes(day)
      ? schedule.weekdays.filter((d) => d !== day)
      : [...schedule.weekdays, day].sort();
  }

  function onKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      void close();
    } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      if (stepIndex === WIZARD_STEPS.length - 1) void save();
      else next();
    }
  }
</script>

<!-- On the window rather than the dialog: a button that swaps out under the
     pointer (Try it → Stop) drops focus to <body>, and the keys still have
     to work then. -->
<svelte:window onkeydown={onKeydown} />

<div class="wizard-backdrop" transition:fade={{ duration: 140 }}></div>
<div class="wizard-anchor">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="wizard"
    role="dialog"
    aria-modal="true"
    aria-label={isNew ? t("agents.wizard.titleNew") : t("agents.wizard.titleEdit")}
    tabindex="-1"
    bind:this={dialogEl}
    transition:fly={{ y: -16, duration: 180, easing: cubicOut }}
    onpointerdown={(e) => e.stopPropagation()}
  >
    <header class="wizard-header">
      <span class="wizard-icon">{spec.icon}</span>
      <div class="wizard-heading">
        <span class="wizard-title">{isNew ? t("agents.wizard.titleNew") : t("agents.wizard.titleEdit")}</span>
        {#if spec.name}<span class="wizard-sub">{spec.name}</span>{/if}
      </div>
      <button class="close-button" aria-label={t("agents.wizard.cancel")} onclick={() => void close()}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    <nav class="stepper" aria-label={t("agents.wizard.progress")}>
      {#each WIZARD_STEPS as candidate, index (candidate)}
        <button
          class="stepper-item"
          class:current={index === stepIndex}
          class:done={index < stepIndex}
          aria-current={index === stepIndex ? "step" : undefined}
          onclick={() => go(index)}
        >
          <span class="stepper-number">{index < stepIndex ? "✓" : index + 1}</span>
          <span class="stepper-label">{t(STEP_LABEL_KEYS[candidate])}</span>
        </button>
      {/each}
    </nav>

    <div class="step-viewport">
      {#key step}
        <div class="step-body" in:fly={{ x: SLIDE_PX * direction, duration: SLIDE_MS, easing: cubicOut, opacity: 0 }}>
          {#if step === "idea"}
            <h2>{t("agents.wizard.idea.title")}</h2>
            <p class="explain">{t("agents.wizard.idea.explain")}</p>
            <textarea
              class="field big"
              rows="5"
              placeholder={t("agents.wizard.idea.placeholder")}
              bind:value={spec.description}
              disabled={drafting}
            ></textarea>
            <div class="row">
              <button
                class="button primary"
                disabled={drafting || !spec.description.trim() || !installed("claude")}
                onclick={() => void buildWithAi()}
              >
                ✨ {t("agents.wizard.idea.buildWithAi")}
              </button>
              <span class="hint" class:working={drafting}>
                {drafting
                  ? t("agents.wizard.idea.thinking")
                  : installed("claude")
                    ? t("agents.wizard.idea.aiHint")
                    : t("agents.wizard.idea.aiNeedsClaude")}
              </span>
            </div>
            {#if draftError}<p class="error">{draftError}</p>{/if}

            <div class="label">{t("agents.wizard.idea.templates")}</div>
            <div class="templates">
              {#each AGENT_TEMPLATES as template (template.id)}
                <button class="template" disabled={drafting} onclick={() => useTemplate(template)}>
                  <span class="template-icon">{template.icon}</span>
                  <span class="template-text">
                    <span class="card-title">{t(template.name)}</span>
                    <span class="card-text">{t(template.description)}</span>
                  </span>
                </button>
              {/each}
            </div>
            <div class="label">{t("agents.wizard.idea.icon")}</div>
            <div class="icons">
              {#each AGENT_ICONS as icon (icon)}
                <button class="icon-choice" class:selected={spec.icon === icon} onclick={() => (spec.icon = icon)}>{icon}</button>
              {/each}
            </div>
          {:else if step === "logic"}
            {#if draftNote}
              <div class="ai-note">
                <span class="ai-note-title">✨ {t("agents.wizard.idea.drafted")}</span>
                {draftNote}
              </div>
            {/if}
            <h2>{t("agents.wizard.logic.title")}</h2>
            <p class="explain">{t("agents.wizard.logic.explain")}</p>
            <label class="label" for="agent-name">{t("agents.wizard.logic.name")}</label>
            <input id="agent-name" class="field" placeholder={t("agents.wizard.logic.namePlaceholder")} bind:value={spec.name} />
            <label class="label" for="agent-task">{t("agents.wizard.logic.task")}</label>
            <p class="hint">{t("agents.wizard.logic.taskHint")}</p>
            <textarea id="agent-task" class="field" rows="3" placeholder={t("agents.wizard.logic.taskPlaceholder")} bind:value={spec.task}
            ></textarea>
            <label class="label" for="agent-logic">{t("agents.wizard.logic.instructions")}</label>
            <p class="hint">{t("agents.wizard.logic.instructionsHint")}</p>
            <textarea
              id="agent-logic"
              class="field mono"
              rows="7"
              placeholder={t("agents.wizard.logic.instructionsPlaceholder")}
              bind:value={spec.instructions}
            ></textarea>
          {:else if step === "engine"}
            <h2>{t("agents.wizard.engine.title")}</h2>
            <p class="explain">{t("agents.wizard.engine.explain")}</p>
            <div class="cards two">
              {#each ENGINES as engine (engine.id)}
                <button
                  class="card"
                  class:selected={spec.engine === engine.id}
                  disabled={!installed(engine.id)}
                  onclick={() => chooseEngine(engine.id)}
                >
                  <span class="card-title"><ConsoleIcon kind={engine.id} size={14} /> {engine.label}</span>
                  <span class="card-text">
                    {installed(engine.id) ? t(engine.hint) : t("agents.wizard.engine.notInstalled")}
                  </span>
                </button>
              {/each}
            </div>

            <div class="label">{t("agents.wizard.engine.workDir")}</div>
            <p class="hint">{t("agents.wizard.engine.workDirHint")}</p>
            <FolderField placeholder={t("agents.wizard.engine.workDirOwn")} bind:value={spec.workDir} />
            <div class="chips">
              <button class="chip" class:selected={!spec.workDir.trim()} onclick={() => (spec.workDir = "")}>
                {t("agents.wizard.engine.useOwn")}
              </button>
              {#if hereFolder}
                <button class="chip" class:selected={spec.workDir === hereFolder} title={hereFolder} onclick={() => (spec.workDir = hereFolder!)}>
                  {t("agents.wizard.engine.useHere")}
                </button>
              {/if}
            </div>

            {#if agentsStore.projects.length > 0}
              <label class="label" for="agent-project">{t("agents.wizard.engine.project")}</label>
              <select id="agent-project" class="field" bind:value={spec.projectId}>
                <option value="">{t("agents.projects.loose")}</option>
                {#each agentsStore.projects as project (project.id)}
                  <option value={project.id}>{project.icon} {project.name}</option>
                {/each}
              </select>
            {/if}

            <div class="label">{t("agents.wizard.engine.autonomy")}</div>
            <p class="hint">{t("agents.wizard.engine.autonomyHint")}</p>
            <div class="cards three">
              {#each AUTONOMY as level (level.id)}
                <button class="card" class:selected={spec.autonomy === level.id} onclick={() => (spec.autonomy = level.id)}>
                  <span class="card-title">{t(level.title)}</span>
                  <span class="card-text">{t(level.text)}</span>
                </button>
              {/each}
            </div>

            {#if outsideRisk}
              <p class="warning">⚠ {t("agents.wizard.untrustedWarning")}</p>
            {/if}

            <details class="advanced">
              <summary>{t("agents.wizard.advanced")}</summary>
              <div class="row">
                <label class="label inline" for="agent-model">{t("agents.wizard.engine.model")}</label>
                <select id="agent-model" class="field" bind:value={spec.model}>
                  <option value="">{t("agents.wizard.engine.modelDefault")}</option>
                  {#if spec.model && !engineModels.some((model) => model.id === spec.model)}
                    <option value={spec.model}>{t("agents.wizard.engine.modelUnlisted", { model: spec.model })}</option>
                  {/if}
                  {#each engineModels.filter((model) => model.main) as model (model.id)}
                    <option value={model.id} title={model.description}>{modelLabel(model)}</option>
                  {/each}
                  {#if engineModels.some((model) => !model.main)}
                    <optgroup label={t("agents.wizard.engine.modelOthers")}>
                      {#each engineModels.filter((model) => !model.main) as model (model.id)}
                        <option value={model.id} title={model.description}>{modelLabel(model)}</option>
                      {/each}
                    </optgroup>
                  {/if}
                </select>
              </div>
              {#if models[spec.engine] && engineModels.length === 0}
                <p class="hint">{t("agents.wizard.engine.modelsMissing", { engine: spec.engine === "claude" ? "Claude Code" : "Codex" })}</p>
              {/if}
              <div class="row">
                <label class="label inline" for="agent-timeout">{t("agents.wizard.engine.timeout")}</label>
                <input id="agent-timeout" class="field narrow" type="number" min="1" bind:value={spec.limits.timeoutMin} />
                <span class="hint">{t("agents.wizard.engine.minutes")}</span>
              </div>
            </details>
          {:else if step === "tools"}
            <h2>{t("agents.wizard.tools.title")}</h2>
            <p class="explain">{t("agents.wizard.tools.explain")}</p>
            <div class="capabilities">
              <div class="capability on">✓ {t("agents.wizard.tools.read")}</div>
              <div class="capability" class:on={spec.autonomy !== "read"}>
                {spec.autonomy !== "read" ? "✓" : "✗"}
                {t("agents.wizard.tools.edit")}
              </div>
              <div class="capability" class:on={spec.autonomy === "run"}>
                {spec.autonomy === "run" ? "✓" : "✗"}
                {t("agents.wizard.tools.run")}
              </div>
              <p class="hint">{t("agents.wizard.tools.fromAutonomy")}</p>
            </div>
            <label class="toggle">
              <input type="checkbox" bind:checked={spec.tools.web} />
              <span>
                <span class="toggle-title">{t("agents.wizard.tools.web")}</span>
                <span class="hint">{t("agents.wizard.tools.webHint")}</span>
              </span>
            </label>
            <div class="label">{t("agents.wizard.tools.integrations")}</div>
            <p class="hint">{t("agents.wizard.tools.integrationsHint")}</p>
            {#each spec.tools.mcp as server (server.name)}
              <div class="integration">
                <span>🔌 {server.name}</span>
                <button class="link" onclick={() => removeIntegration(server.name)}>{t("agents.integrations.remove")}</button>
              </div>
              {#each secretsOf(server.name, missing) as name (name)}
                <!-- An imported agent, or a keychain that lost the value: the
                     integration can't authenticate until it's entered here. -->
                <div class="missing-secret">
                  <span class="hint">🔒 {t("agents.integrations.missingSecret", { name })}</span>
                  <div class="row">
                    <input class="field" type="password" bind:value={secretDrafts[name]} />
                    <button class="button" disabled={!secretDrafts[name]} onclick={() => void storeSecret(name)}>
                      {t("agents.integrations.saveSecret")}
                    </button>
                  </div>
                </div>
              {/each}
            {/each}
            {#if picking === "registry"}
              <IntegrationPicker agentId={spec.id} {suggestions} onadd={addIntegration} oncancel={() => (picking = null)} />
            {:else if picking === "custom"}
              <CustomConnectorForm agentId={spec.id} onadd={addIntegration} oncancel={() => (picking = null)} />
            {:else}
              <div class="row">
                <button class="button" onclick={() => (picking = "registry")}>+ {t("agents.integrations.addOne")}</button>
                <button class="button" onclick={() => (picking = "custom")}>+ {t("agents.custom.addOwn")}</button>
              </div>
              {#if suggestions.length > 0}
                <p class="hint note">✨ {t("agents.integrations.aiSuggests", { list: suggestions.map((s) => s.query).join(", ") })}</p>
              {/if}
            {/if}

            <div class="label">{t("agents.wizard.tools.team")}</div>
            <p class="hint">{t("agents.wizard.tools.teamHint")}</p>
            {#if inTeam}
              <p class="hint">{t("agents.wizard.tools.teamOnly")}</p>
            {/if}
            {#each others as other (other.id)}
              <label class="integration contact">
                <input
                  type="checkbox"
                  checked={spec.collaboration.contacts.includes(other.id)}
                  onchange={(e) => toggleContact(other.id, e.currentTarget.checked)}
                />
                <span class="contact-name">{other.icon || "🤖"} {other.name}</span>
                <span class="hint contact-level">
                  {t(AUTONOMY_TITLE[other.autonomy])}{#if !other.enabled}{" · "}{t("agents.wizard.tools.paused")}{/if}
                </span>
              </label>
            {:else}
              <p class="hint">{t("agents.wizard.tools.noOthers")}</p>
            {/each}
            {#if stronger.length > 0}
              <label class="toggle">
                <input type="checkbox" bind:checked={spec.collaboration.allowEscalation} />
                <span>
                  <span class="toggle-title">{t("agents.wizard.tools.escalation")}</span>
                  <span class="hint">
                    {t("agents.wizard.tools.escalationHint", { names: stronger.map((agent) => agent.name).join(", ") })}
                  </span>
                </span>
              </label>
            {/if}
            <label class="toggle">
              <input type="checkbox" bind:checked={spec.collaboration.board} />
              <span>
                <span class="toggle-title">{t("agents.wizard.tools.board")}</span>
                <span class="hint">{t("agents.wizard.tools.boardHint")}</span>
              </span>
            </label>
          {:else if step === "triggers"}
            {@const schedule = spec.triggers.schedule}
            <h2>{t("agents.wizard.triggers.title")}</h2>
            <p class="explain">{t("agents.wizard.triggers.explain")}</p>
            <div class="cards three">
              <button class="card" class:selected={!schedule} onclick={() => setSchedule("none")}>
                <span class="card-title">{t("agents.wizard.triggers.manual")}</span>
                <span class="card-text">{t("agents.wizard.triggers.manualText")}</span>
              </button>
              <button class="card" class:selected={schedule?.kind === "interval"} onclick={() => setSchedule("interval")}>
                <span class="card-title">{t("agents.wizard.triggers.interval")}</span>
                <span class="card-text">{t("agents.wizard.triggers.intervalText")}</span>
              </button>
              <button class="card" class:selected={schedule?.kind === "daily"} onclick={() => setSchedule("daily")}>
                <span class="card-title">{t("agents.wizard.triggers.daily")}</span>
                <span class="card-text">{t("agents.wizard.triggers.dailyText")}</span>
              </button>
            </div>
            {#if schedule?.kind === "interval"}
              <div class="row">
                <span class="label inline">{t("agents.wizard.triggers.every")}</span>
                <input class="field narrow" type="number" min="1" bind:value={schedule.minutes} />
                <span class="hint">{t("agents.wizard.engine.minutes")}</span>
              </div>
            {:else if schedule?.kind === "daily"}
              <div class="row">
                <span class="label inline">{t("agents.wizard.triggers.at")}</span>
                <input class="field narrow" type="time" bind:value={schedule.at} />
              </div>
              <div class="chips">
                {#each WEEKDAY_KEYS as key, day (key)}
                  <button class="chip" class:selected={schedule.weekdays.includes(day)} onclick={() => toggleDay(day)}>{t(key)}</button>
                {/each}
              </div>
              <p class="hint">{t("agents.wizard.triggers.daysHint")}</p>
            {/if}

            <label class="toggle">
              <input
                type="checkbox"
                checked={spec.triggers.watch !== null}
                onchange={(e) => (spec.triggers.watch = e.currentTarget.checked ? { path: spec.workDir || hereFolder || "" } : null)}
              />
              <span>
                <span class="toggle-title">{t("agents.wizard.triggers.watch")}</span>
                <span class="hint">{t("agents.wizard.triggers.watchHint")}</span>
              </span>
            </label>
            {#if spec.triggers.watch}
              <FolderField placeholder={t("agents.wizard.triggers.watchPlaceholder")} bind:value={spec.triggers.watch.path} />
            {/if}

            <label class="toggle">
              <input
                type="checkbox"
                checked={spec.triggers.git !== null}
                onchange={(e) =>
                  (spec.triggers.git = e.currentTarget.checked
                    ? { repoPath: spec.workDir || hereFolder || "", event: "commit" }
                    : null)}
              />
              <span>
                <span class="toggle-title">{t("agents.wizard.triggers.git")}</span>
                <span class="hint">{t("agents.wizard.triggers.gitHint")}</span>
              </span>
            </label>
            {#if spec.triggers.git}
              <div class="row">
                <FolderField placeholder={t("agents.wizard.triggers.gitPlaceholder")} bind:value={spec.triggers.git.repoPath} />
                <select class="field narrow wide" bind:value={spec.triggers.git.event} aria-label={t("agents.wizard.triggers.gitEvent")}>
                  <option value="commit">{t("agents.wizard.triggers.gitCommit")}</option>
                  <option value="push">{t("agents.wizard.triggers.gitPush")}</option>
                </select>
              </div>
            {/if}

            <label class="toggle">
              <input
                type="checkbox"
                checked={spec.triggers.webhook !== null}
                onchange={(e) => (spec.triggers.webhook = e.currentTarget.checked ? { token: newToken() } : null)}
              />
              <span>
                <span class="toggle-title">{t("agents.wizard.triggers.webhook")}</span>
                <span class="hint">{t("agents.wizard.triggers.webhookHint")}</span>
              </span>
            </label>
            {#if spec.triggers.webhook}
              <div class="webhook">
                {#if webhookInfo && !webhookInfo.listening}
                  <p class="error">{t("agents.wizard.triggers.webhookDown", { port: String(webhookInfo.port) })}</p>
                {/if}
                <div class="command">
                  <code>{curlExample}</code>
                  <button class="button small" onclick={() => void copyCurl()}>
                    {copiedCurl ? t("agents.github.copied") : t("agents.github.copy")}
                  </button>
                </div>
                <p class="hint">
                  {t("agents.wizard.triggers.webhookBody")}
                  <button class="link" onclick={() => spec.triggers.webhook && (spec.triggers.webhook.token = newToken())}>
                    {t("agents.wizard.triggers.webhookNewToken")}
                  </button>
                </p>
              </div>
            {/if}
            <label class="toggle">
              <input
                type="checkbox"
                checked={spec.triggers.after !== null}
                disabled={others.length === 0 && !spec.triggers.after}
                onchange={(e) =>
                  (spec.triggers.after = e.currentTarget.checked
                    ? { agentId: others.length === 1 ? others[0].id : "", on: "success" }
                    : null)}
              />
              <span>
                <span class="toggle-title">{t("agents.wizard.triggers.after")}</span>
                <span class="hint">
                  {others.length === 0 ? t("agents.wizard.triggers.afterNone") : t("agents.wizard.triggers.afterHint")}
                </span>
              </span>
            </label>
            {#if spec.triggers.after}
              <div class="row">
                <select class="field" bind:value={spec.triggers.after.agentId} aria-label={t("agents.wizard.triggers.afterAgent")}>
                  <option value="" disabled>{t("agents.wizard.triggers.afterPick")}</option>
                  {#each others as other (other.id)}
                    <option value={other.id}>{other.icon || "🤖"} {other.name}</option>
                  {/each}
                </select>
                <select class="field narrow wide" bind:value={spec.triggers.after.on} aria-label={t("agents.wizard.triggers.afterOn")}>
                  <option value="success">{t("agents.wizard.triggers.afterSuccess")}</option>
                  <option value="failure">{t("agents.wizard.triggers.afterFailure")}</option>
                  <option value="any">{t("agents.wizard.triggers.afterAny")}</option>
                </select>
              </div>
            {/if}
            {#if outsideRisk}
              <p class="warning">⚠ {t("agents.wizard.untrustedWarning")}</p>
            {/if}
            <p class="hint note">{t("agents.wizard.triggers.onlyOpen")}</p>
          {:else if step === "test"}
            <h2>{t("agents.wizard.test.title")}</h2>
            <p class="explain">{t("agents.wizard.test.explain")}</p>
            <div class="row">
              {#if testRunning}
                <button class="button" onclick={() => void agentsStore.stop(spec.id)}>{t("agents.actions.stop")}</button>
                <span class="hint working">{testRun?.activity || t("agents.run.working")}</span>
              {:else}
                <button class="button primary" onclick={() => void test()}>
                  {testRunId ? t("agents.wizard.test.again") : t("agents.wizard.test.run")}
                </button>
              {/if}
            </div>
            {#if testError}<p class="error">{testError}</p>{/if}
            <div class="test-log">
              {#each withoutEchoes(testEvents) as event, index (index)}
                {#if event.kind === "tool"}
                  <div class="test-step mono"><b>{event.tool}</b> {event.text}</div>
                {:else if event.kind === "text" || event.kind === "result"}
                  <div class="test-step" class:result={event.kind === "result"}>{@html renderMarkdownLite(event.text)}</div>
                {/if}
              {:else}
                <p class="hint">{t("agents.wizard.test.empty")}</p>
              {/each}
              {#if testRun && testRun.id === testRunId && testRun.status !== "running" && testRun.status !== "success"}
                <p class="error">{testRun.summary}</p>
              {/if}
            </div>
          {:else if step === "deploy"}
            <h2>{t("agents.wizard.deploy.title")}</h2>
            <p class="explain">{t("agents.wizard.deploy.explain")}</p>
            <div class="cards three">
              <div class="card selected static">
                <span class="card-title">💻 {t("agents.wizard.deploy.local")}</span>
                <span class="card-text">{t("agents.wizard.deploy.localText")}</span>
              </div>
              <button
                class="card"
                class:selected={showGithub}
                disabled={spec.engine !== "claude"}
                onclick={() => (showGithub = !showGithub)}
              >
                <span class="card-title">GitHub Actions</span>
                <span class="card-text">
                  {spec.engine === "claude" ? t("agents.wizard.deploy.githubText") : t("agents.wizard.deploy.githubClaudeOnly")}
                </span>
                {#if spec.deploy.github}<span class="badge">✓ {t("agents.wizard.deploy.githubDone")}</span>{/if}
              </button>
              <div class="card static disabled">
                <span class="card-title">Vercel</span>
                <span class="card-text">{t("agents.wizard.deploy.vercelText")}</span>
                <span class="badge">{t("agents.wizard.deploy.soon")}</span>
              </div>
            </div>
            {#if showGithub && spec.engine === "claude"}
              <GithubDeployPanel {spec} />
            {/if}
            <label class="toggle">
              <input type="checkbox" bind:checked={spec.enabled} />
              <span>
                <span class="toggle-title">{t("agents.wizard.deploy.enabled")}</span>
                <span class="hint">{t("agents.wizard.deploy.enabledHint")}</span>
              </span>
            </label>
          {/if}

          {#if showProblems && problems.length > 0}
            <ul class="problems">
              {#each problems as problem (problem)}<li>{t(problem)}</li>{/each}
            </ul>
          {/if}
        </div>
      {/key}
    </div>

    <footer class="wizard-footer">
      {#if saveError}<span class="error">{saveError}</span>{/if}
      <span class="footer-hint">{t("agents.wizard.keysHint")}</span>
      {#if stepIndex > 0}
        <button class="button" onclick={() => go(stepIndex - 1)}>{t("agents.wizard.back")}</button>
      {/if}
      {#if !isNew || stepIndex === WIZARD_STEPS.length - 1}
        <button class="button" class:primary={stepIndex === WIZARD_STEPS.length - 1} disabled={saving} onclick={() => void save()}>
          {isNew ? t("agents.wizard.create") : t("agents.wizard.save")}
        </button>
      {/if}
      {#if stepIndex < WIZARD_STEPS.length - 1}
        <button class="button primary" onclick={next}>{t("agents.wizard.next")}</button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .wizard-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.55);
  }

  .wizard-anchor {
    position: absolute;
    inset: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    pointer-events: none;
  }

  .wizard {
    pointer-events: auto;
    display: flex;
    flex-direction: column;
    width: min(860px, 94%);
    height: min(640px, 88%);
    margin-top: 5vh;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 10px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
    font-size: 12.5px;
    outline: none;
    overflow: hidden;
  }

  .wizard-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px 10px 18px;
  }

  .wizard-icon {
    font-size: 22px;
  }

  .wizard-heading {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .wizard-title {
    font-weight: 600;
    font-size: 14px;
  }

  .wizard-sub {
    font-size: 11.5px;
    opacity: 0.6;
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

  .stepper {
    display: flex;
    gap: 2px;
    padding: 0 14px 10px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .stepper-item {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 6px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    opacity: 0.5;
    cursor: pointer;
  }

  .stepper-item:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .stepper-item.current,
  .stepper-item.done {
    opacity: 1;
  }

  .stepper-number {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    border-radius: 50%;
    border: 1px solid currentColor;
    font-size: 10px;
  }

  .stepper-item.current .stepper-number {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .stepper-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .step-viewport {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .step-body {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    padding: 16px 22px 20px;
    scrollbar-width: thin;
  }

  h2 {
    margin: 0 0 4px;
    font-size: 16px;
    font-weight: 600;
  }

  .explain {
    margin: 0 0 14px;
    line-height: 1.55;
    opacity: 0.75;
  }

  .label {
    display: block;
    margin: 14px 0 5px;
    font-weight: 600;
  }

  .label.inline {
    margin: 0;
  }

  .hint {
    margin: 0 0 6px;
    font-size: 11.5px;
    line-height: 1.5;
    opacity: 0.6;
  }

  .hint.note {
    margin-top: 16px;
  }

  .hint.working {
    margin: 0;
    animation: pulse 1.4s ease-in-out infinite;
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

  .field.big {
    font-size: 13.5px;
    line-height: 1.5;
  }

  .field.narrow {
    width: 90px;
  }

  .mono,
  .field.mono {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 10px;
  }

  .row .field:not(.narrow) {
    flex: 1;
  }

  .icons,
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
  }

  .icon-choice {
    width: 36px;
    height: 36px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: transparent;
    font-size: 18px;
    cursor: pointer;
  }

  .icon-choice.selected,
  .chip.selected,
  .card.selected {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.2));
  }

  .chip {
    padding: 4px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .cards {
    display: grid;
    gap: 8px;
  }

  .cards.two {
    grid-template-columns: 1fr 1fr;
  }

  .cards.three {
    grid-template-columns: 1fr 1fr 1fr;
  }

  .card {
    position: relative;
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

  .card:hover:not(:disabled):not(.static) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .card:disabled,
  .card.disabled {
    opacity: 0.5;
    cursor: default;
  }

  .card.static {
    cursor: default;
  }

  .card-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }

  .card-text {
    font-size: 11.5px;
    line-height: 1.45;
    opacity: 0.7;
  }

  .badge {
    align-self: flex-start;
    margin-top: 2px;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.1));
    font-size: 10.5px;
  }

  .advanced {
    margin-top: 16px;
  }

  .advanced summary {
    cursor: pointer;
    opacity: 0.7;
  }

  .capabilities {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 12px;
  }

  .capability {
    opacity: 0.5;
  }

  .capability.on {
    opacity: 1;
  }

  .toggle {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    margin-top: 14px;
    cursor: pointer;
  }

  .toggle input {
    margin-top: 2px;
  }

  .toggle > span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .toggle-title {
    font-weight: 600;
  }

  .templates {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .template {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 9px 11px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .template:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .template-icon {
    font-size: 18px;
  }

  .template-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ai-note {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 14px;
    padding: 10px 12px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.12));
    line-height: 1.5;
  }

  .ai-note-title {
    font-weight: 600;
  }

  .field.wide {
    width: 170px;
  }

  .webhook {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 6px;
  }

  .command {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .command code {
    flex: 1;
    min-width: 0;
    padding: 6px 8px;
    border-radius: 5px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.3));
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    overflow-wrap: anywhere;
  }

  .button.small {
    flex-shrink: 0;
    padding: 3px 10px;
    font-size: 11.5px;
  }

  .warning {
    margin: 12px 0 0;
    padding: 8px 10px;
    border-left: 3px solid var(--warning-fg, #d7ba7d);
    border-radius: 5px;
    background: rgba(215, 186, 125, 0.1);
    font-size: 11.5px;
    line-height: 1.5;
  }

  .missing-secret {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: -2px 0 8px 12px;
  }

  .integration {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    margin-bottom: 6px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
  }

  .contact {
    justify-content: flex-start;
    gap: 9px;
    cursor: pointer;
  }

  .contact-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .contact-level {
    margin: 0;
    white-space: nowrap;
  }

  .link {
    border: none;
    background: transparent;
    color: var(--error-fg, #f48771);
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .button {
    padding: 6px 14px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .button.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .test-log {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-top: 12px;
    padding: 10px 12px;
    min-height: 120px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
  }

  .test-step {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: 1.45;
  }

  .test-step.mono {
    font-size: 11.5px;
    opacity: 0.8;
  }

  .test-step.result {
    padding-left: 8px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
  }

  .problems {
    margin: 14px 0 0;
    padding: 8px 12px 8px 26px;
    border-radius: 6px;
    background: rgba(244, 135, 113, 0.12);
    color: var(--error-fg, #f48771);
  }

  .error {
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }

  .wizard-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .footer-hint {
    flex: 1;
    font-size: 11px;
    opacity: 0.45;
  }

  .wizard-footer .error {
    flex: 1;
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
