<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { draftProject, type AgentSpec, type GeneratedProject, type Project } from "$lib/core/agents-client";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import { sessionFolder, workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { collectTiles } from "$lib/layout/split-tree";
  import { agentsStore } from "./agents-store.svelte";
  import { AUTONOMY_TITLE_KEYS } from "./agent-spec";
  import { PROJECT_ICONS, estimatedRunsPerDay, newProject, projectWarnings } from "./project-spec";
  import { PROJECT_TEMPLATES, buildFromTemplate, templateDescription, templateName, type ProjectTemplate } from "./project-templates";
  import ProjectGraph from "./ProjectGraph.svelte";
  import FolderField from "./FolderField.svelte";

  const editing = uiState.projectDialog?.project ?? null;

  type Mode = "empty" | "template" | "ai";
  let mode = $state<Mode>("empty");
  /** What's being set up: the project, and (from a template or AI) its team. */
  let project = $state<Project>(editing ? structuredClone($state.snapshot(editing)) : newProject(crypto.randomUUID()));
  let draft = $state<GeneratedProject | null>(null);
  let selected = $state<string | null>(null);
  let description = $state("");
  let generating = $state(false);
  let error = $state("");
  let saving = $state(false);
  let dialogEl: HTMLDivElement | undefined = $state();

  /** The folder the focused console is in, offered as the working folder. */
  const hereFolder = $derived.by(() => {
    const root = workspaceStore.root;
    if (!root) return null;
    const tile = collectTiles(root).find((candidate) => candidate.id === workspaceStore.focusedTileId);
    const active = tile?.sessions.find((session) => session.id === tile.activeSessionId);
    return active ? (sessionFolder(active) ?? null) : null;
  });

  const claudeInstalled = $derived(newTabOptions.agentClis?.claude !== false);
  const projectAgents = $derived(editing ? agentsStore.agentsOf(editing.id) : []);
  const reviewAgents = $derived(draft?.agents ?? []);
  const warnings = $derived(draft ? projectWarnings(project, reviewAgents) : []);
  const selectedAgent = $derived(reviewAgents.find((agent) => agent.id === selected) ?? null);

  onMount(() => {
    newTabOptions.refresh();
    void tick().then(() => dialogEl?.querySelector<HTMLElement>("input, textarea")?.focus());
  });

  function close() {
    uiState.closeProjectDialog();
  }

  function useTemplate(template: ProjectTemplate) {
    const built = buildFromTemplate(template, (key) => t(key), project.workDir || hereFolder || "", () =>
      crypto.randomUUID(),
    );
    draft = { ...built, integrations: [], explanation: "" };
    project = built.project;
    selected = null;
  }

  async function generate() {
    if (!description.trim() || generating) return;
    generating = true;
    error = "";
    try {
      const generated = await draftProject(description.trim(), project.workDir);
      draft = generated;
      project = generated.project;
      selected = null;
    } catch (reason) {
      error = String(reason);
    } finally {
      generating = false;
    }
  }

  function nameOf(agentId: string): string {
    return reviewAgents.find((agent) => agent.id === agentId)?.name ?? "";
  }

  async function save() {
    if (!project.name.trim()) {
      error = t("agents.projects.dialog.nameMissing");
      return;
    }
    saving = true;
    error = "";
    try {
      project.name = project.name.trim();
      if (draft) {
        const agents: AgentSpec[] = draft.agents.map((agent) => ({ ...agent, projectId: project.id }));
        await agentsStore.createProject(project, agents);
      } else {
        await agentsStore.saveProject(project);
      }
      agentsStore.setOpen(true);
      if (!editing) workspaceStore.openProject(project.id, project.name);
      close();
    } catch (reason) {
      error = String(reason);
    } finally {
      saving = false;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void save();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet settings(showTeam: boolean)}
  <label class="label" for="project-name">{t("agents.projects.dialog.name")}</label>
  <input id="project-name" class="field" placeholder={t("agents.projects.dialog.namePlaceholder")} bind:value={project.name} />
  <div class="label">{t("agents.projects.dialog.icon")}</div>
  <div class="icons">
    {#each PROJECT_ICONS as icon (icon)}
      <button class="icon-choice" class:selected={project.icon === icon} onclick={() => (project.icon = icon)}>{icon}</button>
    {/each}
  </div>
  <label class="label" for="project-goal">{t("agents.projects.dialog.goal")}</label>
  <textarea id="project-goal" class="field" rows="2" placeholder={t("agents.projects.dialog.goalPlaceholder")} bind:value={project.goal}
  ></textarea>
  <label class="label" for="project-folder">{t("agents.projects.dialog.workDir")}</label>
  <p class="hint">{t("agents.projects.dialog.workDirHint")}</p>
  <div class="row">
    <FolderField id="project-folder" placeholder={t("agents.projects.dialog.workDirPlaceholder")} bind:value={project.workDir} />
    {#if hereFolder}
      <button class="button small" title={hereFolder} onclick={() => (project.workDir = hereFolder!)}>
        {t("agents.wizard.engine.useHere")}
      </button>
    {/if}
  </div>
  {#if showTeam}
    <label class="toggle">
      <input type="checkbox" bind:checked={project.isolated} />
      <span>
        <span class="toggle-title">{t("agents.projects.dialog.team")}</span>
        <span class="hint">{t("agents.projects.dialog.teamHint")}</span>
      </span>
    </label>
    {#if editing}
      <label class="label" for="project-entry">{t("agents.projects.dialog.entry")}</label>
      <p class="hint">{t("agents.projects.dialog.entryHint")}</p>
      <select id="project-entry" class="field" bind:value={project.entryAgentId}>
        <option value="">{t("agents.projects.dialog.entryNone")}</option>
        {#each projectAgents as agent (agent.id)}
          <option value={agent.id}>{agent.icon || "🤖"} {agent.name}</option>
        {/each}
      </select>
    {/if}
    <div class="row">
      <label class="label inline" for="project-limit">{t("agents.projects.dialog.limit")}</label>
      <input id="project-limit" class="field narrow" type="number" min="0" bind:value={project.limits.maxRunsPerDay} />
    </div>
    <p class="hint">{t("agents.projects.dialog.limitHint")}</p>
  {/if}
{/snippet}

<div class="dialog-backdrop" transition:fade={{ duration: 140 }}></div>
<div class="dialog-anchor">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={editing ? t("agents.projects.dialog.titleEdit") : t("agents.projects.dialog.titleNew")}
    tabindex="-1"
    bind:this={dialogEl}
    transition:fly={{ y: -16, duration: 180, easing: cubicOut }}
    onpointerdown={(e) => e.stopPropagation()}
  >
    <header class="dialog-header">
      <span class="dialog-icon">{project.icon}</span>
      <div class="dialog-heading">
        <span class="dialog-title">
          {editing ? t("agents.projects.dialog.titleEdit") : draft ? t("agents.projects.review.title") : t("agents.projects.dialog.titleNew")}
        </span>
        {#if project.name}<span class="dialog-sub">{project.name}</span>{/if}
      </div>
      <button class="close-button" aria-label={t("agents.wizard.cancel")} onclick={close}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    <div class="dialog-body">
      {#if editing}
        {@render settings(true)}
      {:else if draft}
        <p class="explain">{t("agents.projects.review.explain")}</p>
        {#if draft.explanation}
          <div class="ai-note">
            <span class="ai-note-title">✨ {t("agents.projects.review.aiSays")}</span>
            {draft.explanation}
          </div>
        {/if}
        <div class="row">
          <input class="field" aria-label={t("agents.projects.dialog.name")} bind:value={project.name} />
          <label class="label inline" for="review-limit">{t("agents.projects.dialog.limit")}</label>
          <input id="review-limit" class="field narrow" type="number" min="0" bind:value={project.limits.maxRunsPerDay} />
        </div>
        <ProjectGraph agents={reviewAgents} entryId={project.entryAgentId} {selected} onselect={(id) => (selected = selected === id ? null : id)} />
        {#each draft.agents as agent (agent.id)}
          {#if agent.id === selected}
          <div class="agent-detail">
            <div class="row">
              <input class="field" aria-label={t("agents.wizard.logic.name")} bind:value={agent.name} />
              <select class="field narrow wide" aria-label={t("agents.wizard.engine.autonomy")} bind:value={agent.autonomy}>
                <option value="read">{t(AUTONOMY_TITLE_KEYS.read)}</option>
                <option value="edit">{t(AUTONOMY_TITLE_KEYS.edit)}</option>
                <option value="run">{t(AUTONOMY_TITLE_KEYS.run)}</option>
              </select>
              <label class="entry-choice">
                <input type="radio" name="entry" checked={project.entryAgentId === agent.id} onchange={() => (project.entryAgentId = agent.id)} />
                {t("agents.projects.review.makeEntry")}
              </label>
            </div>
            <label class="label" for="review-task">{t("agents.wizard.logic.task")}</label>
            <textarea id="review-task" class="field" rows="2" bind:value={agent.task}></textarea>
            <label class="label" for="review-logic">{t("agents.wizard.logic.instructions")}</label>
            <textarea id="review-logic" class="field mono" rows="6" bind:value={agent.instructions}></textarea>
            {#if agent.triggers.after}
              <p class="hint">⛓ {t("agents.next.after", { name: nameOf(agent.triggers.after.agentId) })}</p>
            {/if}
            {#if agent.collaboration.contacts.length > 0}
              <p class="hint">🤝 {agent.collaboration.contacts.map(nameOf).join(", ")}</p>
            {/if}
          </div>
          {/if}
        {/each}
        {#if !selectedAgent}
          <p class="hint">{t("agents.projects.review.clickNode")}</p>
        {/if}

        <div class="label">{t("agents.projects.review.warnings")}</div>
        {#each warnings as warning (warning.key + JSON.stringify(warning.params ?? {}))}
          <p class={warning.serious ? "warning" : "note"}>{warning.serious ? "⚠" : "ℹ"} {t(warning.key, warning.params)}</p>
        {:else}
          <p class="hint">{t("agents.projects.review.noWarnings")}</p>
        {/each}
        <p class="hint">{t("agents.projects.review.runsPerDay", { n: String(estimatedRunsPerDay(reviewAgents)) })}</p>
        {#if draft.integrations.length > 0}
          <div class="label">{t("agents.projects.review.integrations")}</div>
          <p class="hint">{t("agents.projects.review.integrationsHint")}</p>
          <ul class="integrations">
            {#each draft.integrations as integration (integration.agentId + integration.query)}
              <li><b>{nameOf(integration.agentId)}</b>: {integration.query} — {integration.why}</li>
            {/each}
          </ul>
        {/if}
      {:else}
        <div class="cards three">
          <button class="card" class:selected={mode === "empty"} onclick={() => (mode = "empty")}>
            <span class="card-title">📁 {t("agents.projects.dialog.empty")}</span>
            <span class="card-text">{t("agents.projects.dialog.emptyText")}</span>
          </button>
          <button class="card" class:selected={mode === "template"} onclick={() => (mode = "template")}>
            <span class="card-title">🧩 {t("agents.projects.dialog.template")}</span>
            <span class="card-text">{t("agents.projects.dialog.templateText")}</span>
          </button>
          <button class="card" class:selected={mode === "ai"} onclick={() => (mode = "ai")}>
            <span class="card-title">✨ {t("agents.projects.dialog.ai")}</span>
            <span class="card-text">{t("agents.projects.dialog.aiText")}</span>
          </button>
        </div>

        {#if mode === "empty"}
          {@render settings(true)}
        {:else if mode === "template"}
          <label class="label" for="template-folder">{t("agents.projects.dialog.workDir")}</label>
          <div class="row">
            <FolderField id="template-folder" placeholder={t("agents.projects.dialog.workDirPlaceholder")} bind:value={project.workDir} />
            {#if hereFolder}
              <button class="button small" title={hereFolder} onclick={() => (project.workDir = hereFolder!)}>
                {t("agents.wizard.engine.useHere")}
              </button>
            {/if}
          </div>
          <div class="templates">
            {#each PROJECT_TEMPLATES as template (template.id)}
              <button class="template" onclick={() => useTemplate(template)}>
                <span class="template-icon">{template.icon}</span>
                <span class="template-text">
                  <span class="card-title">{t(templateName(template))}</span>
                  <span class="card-text">{t(templateDescription(template))}</span>
                </span>
              </button>
            {/each}
          </div>
        {:else}
          <label class="label" for="project-ai">{t("agents.projects.dialog.aiDescribe")}</label>
          <textarea
            id="project-ai"
            class="field big"
            rows="5"
            placeholder={t("agents.projects.dialog.aiPlaceholder")}
            bind:value={description}
            disabled={generating}
          ></textarea>
          <label class="label" for="ai-folder">{t("agents.projects.dialog.workDir")}</label>
          <div class="row">
            <FolderField
              id="ai-folder"
              placeholder={t("agents.projects.dialog.workDirPlaceholder")}
              bind:value={project.workDir}
              disabled={generating}
            />
            {#if hereFolder}
              <button class="button small" title={hereFolder} disabled={generating} onclick={() => (project.workDir = hereFolder!)}>
                {t("agents.wizard.engine.useHere")}
              </button>
            {/if}
          </div>
          <div class="row">
            <button class="button primary" disabled={generating || !description.trim() || !claudeInstalled} onclick={() => void generate()}>
              ✨ {t("agents.projects.dialog.generate")}
            </button>
            <span class="hint" class:working={generating}>
              {generating
                ? t("agents.projects.dialog.generating")
                : claudeInstalled
                  ? t("agents.projects.dialog.aiHint")
                  : t("agents.wizard.idea.aiNeedsClaude")}
            </span>
          </div>
        {/if}
      {/if}
    </div>

    <footer class="dialog-footer">
      {#if error}<span class="error">{error}</span>{:else}<span class="footer-hint">{t("agents.wizard.keysHint")}</span>{/if}
      {#if draft && !editing}
        <button class="button" onclick={() => ((draft = null), (project = newProject(crypto.randomUUID())))}>
          {t("agents.wizard.back")}
        </button>
      {/if}
      <button class="button" onclick={close}>{t("agents.wizard.cancel")}</button>
      {#if editing || draft || mode === "empty"}
        <button class="button primary" disabled={saving} onclick={() => void save()}>
          {editing ? t("agents.wizard.save") : t("agents.projects.dialog.create")}
        </button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .dialog-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.55);
  }

  .dialog-anchor {
    position: absolute;
    inset: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    pointer-events: none;
  }

  .dialog {
    pointer-events: auto;
    display: flex;
    flex-direction: column;
    width: min(860px, 94%);
    max-height: 88%;
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

  .dialog-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px 10px 18px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .dialog-icon {
    font-size: 22px;
  }

  .dialog-heading {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .dialog-title {
    font-weight: 600;
    font-size: 14px;
  }

  .dialog-sub {
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

  .dialog-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 22px 20px;
    scrollbar-width: thin;
  }

  .explain {
    margin: 0 0 12px;
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
    white-space: nowrap;
  }

  .hint {
    margin: 0 0 6px;
    font-size: 11.5px;
    line-height: 1.5;
    opacity: 0.6;
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

  .field.wide {
    width: 150px;
  }

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

  .row .field:not(.narrow):not(.wide) {
    flex: 1;
  }

  .icons {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .icon-choice {
    width: 34px;
    height: 34px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: transparent;
    font-size: 17px;
    cursor: pointer;
  }

  .icon-choice.selected,
  .card.selected {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.2));
  }

  .cards {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 8px;
    margin-bottom: 6px;
  }

  .card,
  .template {
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

  .card:hover,
  .template:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .card-title {
    font-weight: 600;
  }

  .card-text {
    font-size: 11.5px;
    line-height: 1.45;
    opacity: 0.7;
  }

  .templates {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-top: 14px;
  }

  .template {
    flex-direction: row;
    align-items: flex-start;
    gap: 10px;
  }

  .template-icon {
    font-size: 18px;
  }

  .template-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
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

  .ai-note {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 12px;
    padding: 10px 12px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.12));
    line-height: 1.5;
  }

  .ai-note-title {
    font-weight: 600;
  }

  .agent-detail {
    margin-top: 10px;
    padding: 4px 12px 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
  }

  .entry-choice {
    display: flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
    font-size: 11.5px;
    cursor: pointer;
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

  .integrations {
    margin: 4px 0 0;
    padding-left: 18px;
    line-height: 1.6;
    font-size: 11.5px;
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

  .button.small {
    flex-shrink: 0;
    padding: 5px 10px;
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

  .error {
    flex: 1;
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }

  .dialog-footer {
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

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
