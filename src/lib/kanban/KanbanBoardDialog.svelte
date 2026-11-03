<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import type { MessageKey } from "$lib/i18n/translate";
  import { agentModels, type AgentAutonomy, type AgentEngine, type AskLevel, type ModelOption } from "$lib/core/agents-client";
  import { kanbanBranches, type KanbanColumnAgent, type KanbanRole, type KanbanSettings } from "$lib/core/kanban-client";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import FolderField from "$lib/agents/FolderField.svelte";
  import RichSelect from "$lib/agents/RichSelect.svelte";
  import { DEFAULT_HARNESS, newBoardSettings, newColumnAgent } from "./kanban-model";
  import { kanbanStore, type KanbanDialog } from "./kanban-store.svelte";

  // Read once: KanbanDialogs mounts a fresh one for each dialog opened.
  const { settings: given, tab: firstTab } = kanbanStore.dialog as Extract<KanbanDialog, { kind: "board" }>;

  const isNew = !given?.id;
  let settings = $state<KanbanSettings>(given ? structuredClone($state.snapshot(given)) : newBoardSettings((role) => t(DEFAULT_HARNESS[role])));
  let tab = $state(firstTab);
  let busy = $state(false);
  let error = $state("");
  let confirmingDelete = $state(false);

  const ICONS = ["▦", "🚀", "🛠️", "🧪", "📦", "🌐", "📱", "🐛", "✨", "📚"];

  const ENGINES: { id: AgentEngine; hint: MessageKey }[] = [
    { id: "claude", hint: "agents.wizard.engine.claudeHint" },
    { id: "codex", hint: "agents.wizard.engine.codexHint" },
    { id: "gemini", hint: "agents.wizard.engine.geminiHint" },
    { id: "copilot", hint: "agents.wizard.engine.copilotHint" },
    { id: "grok", hint: "agents.wizard.engine.grokHint" },
  ];

  const AUTONOMY: { id: AgentAutonomy; title: MessageKey; text: MessageKey }[] = [
    { id: "read", title: "agents.wizard.autonomy.readTitle", text: "agents.wizard.autonomy.readText" },
    { id: "edit", title: "agents.wizard.autonomy.editTitle", text: "agents.wizard.autonomy.editText" },
    { id: "run", title: "agents.wizard.autonomy.runTitle", text: "agents.wizard.autonomy.runText" },
  ];
  const AUTONOMY_ICONS: Record<AgentAutonomy, string> = { read: "👀", edit: "✏️", run: "⚡" };
  const ASK_LEVELS: AskLevel[] = ["never", "essential", "unsure"];

  const installed = (engine: AgentEngine) => newTabOptions.agentClis?.[engine] !== false;
  const engineOptions = $derived(
    ENGINES.map((engine) => ({
      value: engine.id,
      title: TOOL_DISPLAY_NAMES[engine.id],
      text: installed(engine.id) ? t(engine.hint) : t("agents.wizard.engine.notInstalled"),
      disabled: !installed(engine.id),
    })),
  );
  const autonomyOptions = $derived(AUTONOMY.map((level) => ({ value: level.id, title: t(level.title), text: t(level.text) })));

  /** The models each engine's CLI offers (loaded once; missing until then). */
  let models = $state<Partial<Record<AgentEngine, ModelOption[]>>>({});
  for (const { id: engine } of ENGINES) {
    void agentModels(engine)
      .then((list) => (models = { ...models, [engine]: list }))
      .catch(() => (models = { ...models, [engine]: [] }));
  }

  /** The folder's branches; none when it isn't a repository (no worktrees then). */
  let branches = $state<string[] | null>(null);
  let branchesFor = "";
  $effect(() => {
    const folder = settings.workDir.trim();
    if (folder === branchesFor) return;
    branchesFor = folder;
    if (!folder) {
      branches = null;
      return;
    }
    const timer = setTimeout(() => {
      void kanbanBranches(folder)
        .then((list) => {
          if (branchesFor === folder) branches = list;
        })
        .catch(() => (branches = []));
    }, 250);
    return () => clearTimeout(timer);
  });
  const isRepo = $derived(branches !== null && branches.length > 0);

  function chooseEngine(agent: KanbanColumnAgent, engine: AgentEngine) {
    agent.engine = engine;
    const list = models[engine];
    if (agent.model && list && !list.some((model) => model.id === agent.model)) agent.model = "";
  }

  function modelLabel(model: ModelOption): string {
    return model.note ? `${model.name} · ${model.note}` : model.name;
  }

  function close() {
    kanbanStore.closeDialog();
  }

  async function save() {
    busy = true;
    error = "";
    try {
      if (settings.mode === "worktrees" && !isRepo) settings.mode = "sequential";
      const board = await kanbanStore.save(settings);
      close();
      if (isNew) kanbanStore.openBoard(board);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!settings.id) return;
    busy = true;
    try {
      await kanbanStore.remove(settings.id);
      close();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  }

  const ROLES: { role: KanbanRole; title: MessageKey; hint: MessageKey; column: MessageKey }[] = [
    { role: "work", title: "kanban.agents.work", hint: "kanban.agents.workHint", column: "kanban.columns.inProgress" },
    { role: "review", title: "kanban.agents.review", hint: "kanban.agents.reviewHint", column: "kanban.columns.review" },
    { role: "summary", title: "kanban.agents.summary", hint: "kanban.agents.summaryHint", column: "kanban.columns.done" },
  ];

  /** The role whose settings the Agents tab shows. */
  let role = $state<KanbanRole>("work");
  const current = $derived(ROLES.find((candidate) => candidate.role === role) ?? ROLES[0]);

  function agentOf(of: KanbanRole): KanbanColumnAgent | null {
    return of === "work" ? settings.agents.work : of === "review" ? settings.agents.review : settings.agents.summary;
  }

  function modelName(agent: KanbanColumnAgent): string {
    if (!agent.model) return t("kanban.agents.defaultModel");
    return models[agent.engine]?.find((model) => model.id === agent.model)?.name ?? agent.model;
  }

  function autonomyTitle(level: AgentAutonomy): string {
    return t(AUTONOMY.find((candidate) => candidate.id === level)?.title ?? "agents.wizard.autonomy.readTitle");
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet agentForm(role: KanbanRole, agent: KanbanColumnAgent)}
  {@const list = models[agent.engine] ?? []}
  <div class="group">
  <div class="group-title">{t("kanban.agents.groupEngine")}</div>
  <div class="agent-grid">
    <div>
      <div class="label">{t("kanban.agents.engine")}</div>
      <RichSelect label={t("kanban.agents.engine")} options={engineOptions} value={agent.engine} onpick={(engine) => chooseEngine(agent, engine)}>
        {#snippet icon(engine)}<ConsoleIcon kind={engine} size={16} />{/snippet}
      </RichSelect>
    </div>
    <div>
      <label class="label" for="kanban-model-{role}">{t("kanban.agents.model")}</label>
      <select id="kanban-model-{role}" class="field" bind:value={agent.model}>
        <option value="">{t("agents.wizard.engine.modelDefault")}</option>
        {#if agent.model && !list.some((model) => model.id === agent.model)}
          <option value={agent.model}>{t("agents.wizard.engine.modelUnlisted", { model: agent.model })}</option>
        {/if}
        {#each list.filter((model) => model.main) as model (model.id)}
          <option value={model.id} title={model.description}>{modelLabel(model)}</option>
        {/each}
        {#if list.some((model) => !model.main)}
          <optgroup label={t("agents.wizard.engine.modelOthers")}>
            {#each list.filter((model) => !model.main) as model (model.id)}
              <option value={model.id} title={model.description}>{modelLabel(model)}</option>
            {/each}
          </optgroup>
        {/if}
      </select>
    </div>
  </div>
  </div>
  <div class="group">
    <div class="group-title">{t("kanban.agents.groupAccess")}</div>
    <div class="label">{t("kanban.agents.autonomy")}</div>
    <RichSelect label={t("kanban.agents.autonomy")} options={autonomyOptions} value={agent.autonomy} onpick={(level) => (agent.autonomy = level)}>
      {#snippet icon(level)}{AUTONOMY_ICONS[level]}{/snippet}
    </RichSelect>
    <label class="check">
      <input type="checkbox" bind:checked={agent.web} />
      {t("kanban.agents.web")}
    </label>
  </div>
  <div class="group">
    <div class="group-title">{t("kanban.agents.groupInstructions")}</div>
    <label class="label" for="kanban-context-{role}">{t("kanban.agents.context")}</label>
    <p class="hint">{t("kanban.agents.contextHint")}</p>
    <textarea id="kanban-context-{role}" class="field" rows="3" bind:value={agent.context} placeholder={t("kanban.agents.contextPlaceholder")}></textarea>
    <label class="label" for="kanban-harness-{role}">{t("kanban.agents.harness")}</label>
    <p class="hint">{t("kanban.agents.harnessHint")}</p>
    <textarea id="kanban-harness-{role}" class="field mono" rows="6" bind:value={agent.harness}></textarea>
  </div>
{/snippet}

<div class="backdrop" transition:fade={{ duration: 140 }}></div>
<div class="anchor">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-label={isNew ? t("kanban.dialog.newTitle") : t("kanban.dialog.editTitle")}
    transition:fly={{ y: -16, duration: 180, easing: cubicOut }}
    onpointerdown={(event) => event.stopPropagation()}
  >
    <header class="dialog-header">
      <span class="dialog-icon">{settings.icon || "▦"}</span>
      <div class="heading">
        <span class="dialog-title">{isNew ? t("kanban.dialog.newTitle") : t("kanban.dialog.editTitle")}</span>
        {#if settings.name}<span class="sub">{settings.name}</span>{/if}
      </div>
      <button class="close-button" aria-label={t("kanban.dialog.cancel")} onclick={close}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    <nav class="tabs">
      {#each ["general", "agents"] as const as candidate (candidate)}
        <button class="tab" class:current={tab === candidate} onclick={() => (tab = candidate)}>
          {t(`kanban.dialog.tabs.${candidate}`)}
        </button>
      {/each}
    </nav>

    <div class="body">
      {#if tab === "general"}
        <label class="label" for="kanban-name">{t("kanban.dialog.name")}</label>
        <input id="kanban-name" class="field" bind:value={settings.name} placeholder={t("kanban.dialog.namePlaceholder")} />

        <div class="label">{t("kanban.dialog.icon")}</div>
        <div class="chips">
          {#each ICONS as icon (icon)}
            <button class="icon-choice" class:selected={settings.icon === icon} onclick={() => (settings.icon = icon)}>{icon}</button>
          {/each}
        </div>

        <label class="label" for="kanban-folder">{t("kanban.dialog.folder")}</label>
        <p class="hint">{t("kanban.dialog.folderHint")}</p>
        <FolderField id="kanban-folder" placeholder={t("kanban.dialog.folderPlaceholder")} bind:value={settings.workDir} />

        <div class="label">{t("kanban.dialog.mode")}</div>
        <div class="modes">
          <button class="mode" class:selected={settings.mode === "sequential"} onclick={() => (settings.mode = "sequential")}>
            <strong>{t("kanban.dialog.modeSequential")}</strong>
            <span>{t("kanban.dialog.modeSequentialHint")}</span>
          </button>
          <button
            class="mode"
            class:selected={settings.mode === "worktrees"}
            disabled={!isRepo}
            onclick={() => (settings.mode = "worktrees")}
          >
            <strong>{t("kanban.dialog.modeWorktrees")}</strong>
            <span>{t("kanban.dialog.modeWorktreesHint")}</span>
          </button>
        </div>
        {#if branches !== null && !isRepo}
          <p class="hint">{t("kanban.dialog.notARepo")}</p>
        {/if}

        {#if settings.mode === "worktrees" && isRepo}
          <div class="row">
            <label class="label inline" for="kanban-parallel">{t("kanban.dialog.parallel")}</label>
            <input id="kanban-parallel" class="field narrow" type="number" min="1" max="6" bind:value={settings.maxParallel} />
          </div>
          <label class="label" for="kanban-base">{t("kanban.dialog.baseBranch")}</label>
          <p class="hint">{t("kanban.dialog.baseBranchHint")}</p>
          <select id="kanban-base" class="field" bind:value={settings.baseBranch}>
            <option value="">{t("kanban.dialog.baseBranchCurrent")}</option>
            {#each branches ?? [] as branch (branch)}
              <option value={branch}>{branch}</option>
            {/each}
          </select>
        {/if}

        <div class="row">
          <label class="label inline" for="kanban-rounds">{t("kanban.dialog.rounds")}</label>
          <input id="kanban-rounds" class="field narrow" type="number" min="1" max="10" bind:value={settings.maxReviewRounds} />
        </div>
        <p class="hint">{t("kanban.dialog.roundsHint")}</p>

        <div class="label">{t("kanban.dialog.askLevel")}</div>
        <div class="chips">
          {#each ASK_LEVELS as level (level)}
            <button class="chip" class:selected={settings.askLevel === level} onclick={() => (settings.askLevel = level)}>
              {t(`kanban.dialog.ask.${level}`)}
            </button>
          {/each}
        </div>
      {:else}
        <p class="hint explain">{t("kanban.agents.explain")}</p>
        <div class="roles" role="tablist" aria-label={t("kanban.dialog.tabs.agents")}>
          {#each ROLES as candidate (candidate.role)}
            {@const agent = agentOf(candidate.role)}
            <button
              class="role-card {candidate.role}"
              class:current={role === candidate.role}
              class:empty={!agent}
              role="tab"
              aria-selected={role === candidate.role}
              onclick={() => (role = candidate.role)}
            >
              <span class="role-top">
                <span class="role-dot"></span>
                <span class="role-name">{t(candidate.title)}</span>
              </span>
              <span class="role-column">{t("kanban.agents.column", { column: t(candidate.column) })}</span>
              {#if agent}
                <span class="role-line"><ConsoleIcon kind={agent.engine} size={13} />{TOOL_DISPLAY_NAMES[agent.engine]} · {modelName(agent)}</span>
                <span class="role-line">{AUTONOMY_ICONS[agent.autonomy]} {autonomyTitle(agent.autonomy)}</span>
              {:else}
                <span class="role-line muted">{t("kanban.agents.none")}</span>
              {/if}
            </button>
          {/each}
        </div>

        {@const agent = agentOf(role)}
        <section class="role-panel {role}" role="tabpanel">
          <header class="role-head">
            <span class="role-dot"></span>
            <span class="role-title">{t(current.title)}</span>
            {#if role === "summary"}<span class="optional">{t("kanban.agents.optional")}</span>{/if}
            <span class="spacer"></span>
            {#if role === "summary" && agent}
              <button class="link" onclick={() => (settings.agents.summary = null)}>{t("kanban.agents.removeSummary")}</button>
            {/if}
          </header>
          <p class="hint role-hint">{t(current.hint)}</p>
          {#if agent}
            {@render agentForm(role, agent)}
          {:else}
            <button
              class="button"
              onclick={() => (settings.agents.summary = { ...newColumnAgent("claude", t(DEFAULT_HARNESS.summary)), autonomy: "read" })}
            >
              + {t("kanban.agents.addSummary")}
            </button>
          {/if}
        </section>
      {/if}
    </div>

    {#if error}
      <div class="error">{error}</div>
    {/if}

    <footer class="footer">
      {#if !isNew}
        {#if confirmingDelete}
          <span class="confirm-text">{t("kanban.dialog.deleteConfirm", { name: settings.name })}</span>
          <button class="button" onclick={() => (confirmingDelete = false)}>{t("kanban.dialog.cancel")}</button>
          <button class="button danger" disabled={busy} onclick={remove}>{t("kanban.dialog.delete")}</button>
        {:else}
          <button class="button subtle" onclick={() => (confirmingDelete = true)}>{t("kanban.dialog.delete")}</button>
        {/if}
      {/if}
      {#if !confirmingDelete}
        <span class="spacer"></span>
        <button class="button" onclick={close}>{t("kanban.dialog.cancel")}</button>
        <button class="button primary" disabled={busy || !settings.name.trim() || !settings.workDir.trim()} onclick={save}>
          {isNew ? t("kanban.dialog.create") : t("kanban.dialog.save")}
        </button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.55);
  }

  .anchor {
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
    width: min(720px, 94%);
    height: min(680px, 88%);
    margin-top: 5vh;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 10px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
    font-size: 12.5px;
    overflow: hidden;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px 8px 18px;
  }

  .dialog-icon {
    font-size: 22px;
  }

  .heading {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .dialog-title {
    font-weight: 600;
    font-size: 14px;
  }

  .sub {
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

  .tabs {
    display: flex;
    gap: 2px;
    padding: 0 14px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .tab {
    padding: 6px 12px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: inherit;
    font: inherit;
    opacity: 0.7;
    cursor: pointer;
  }

  .tab.current {
    opacity: 1;
    border-bottom-color: var(--pane-active-border, #3a6df0);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 18px 16px;
    scrollbar-width: thin;
  }

  .label {
    display: block;
    margin: 12px 0 4px;
    font-weight: 600;
  }

  .label.inline {
    margin: 0;
  }

  .hint {
    margin: 0 0 6px;
    font-size: 11.5px;
    opacity: 0.6;
    line-height: 1.4;
  }

  .hint.explain {
    margin-top: 8px;
    font-size: 12px;
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
    line-height: 1.45;
    resize: vertical;
    outline: none;
  }

  .field:focus {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .field.narrow {
    width: 80px;
  }

  .field.mono {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
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

  .icon-choice {
    width: 32px;
    height: 32px;
    padding: 0;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font-size: 16px;
    cursor: pointer;
  }

  .chip.selected,
  .icon-choice.selected,
  .mode.selected {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.2));
  }

  .modes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .mode {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 9px 11px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .mode span {
    font-size: 11.5px;
    opacity: 0.7;
    line-height: 1.4;
  }

  .mode:disabled {
    opacity: 0.45;
    cursor: default;
  }

  /* Each role keeps its color on its card and on its panel. */
  .work {
    --role: #4c8dff;
  }

  .review {
    --role: #b57cff;
  }

  .summary {
    --role: #3fb68b;
  }

  .roles {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    margin-top: 10px;
  }

  .role-card {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    padding: 9px 11px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-top: 3px solid color-mix(in srgb, var(--role) 55%, transparent);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .role-card:hover:not(.current) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .role-card.current {
    border-color: var(--role);
    border-top-color: var(--role);
    background: color-mix(in srgb, var(--role) 14%, transparent);
  }

  .role-card.empty {
    border-style: dashed;
    border-top-style: solid;
  }

  .role-top {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .role-dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--role);
  }

  .role-name {
    font-weight: 600;
    font-size: 13px;
  }

  .role-column {
    font-size: 11px;
    opacity: 0.55;
  }

  .role-line {
    display: flex;
    align-items: center;
    gap: 5px;
    overflow: hidden;
    font-size: 11.5px;
    white-space: nowrap;
    text-overflow: ellipsis;
    opacity: 0.85;
  }

  .role-line.muted {
    opacity: 0.5;
  }

  .role-panel {
    margin-top: 16px;
  }

  .role-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .role-title {
    font-weight: 600;
    font-size: 14.5px;
    color: color-mix(in srgb, var(--role) 70%, currentColor);
  }

  .optional {
    padding: 1px 7px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    font-size: 10.5px;
    opacity: 0.7;
  }

  .role-hint {
    margin-top: 4px;
  }

  .group {
    margin-top: 14px;
    padding-top: 10px;
    border-top: 1px solid var(--tabbar-border, #333);
  }

  .group-title {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    opacity: 0.55;
  }

  .group-title + .label,
  .group-title + .agent-grid {
    margin-top: 8px;
  }

  .group-title + .agent-grid .label {
    margin-top: 0;
  }

  .agent-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 10px;
  }

  .link {
    border: none;
    background: transparent;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    cursor: pointer;
  }

  .error {
    margin: 0 18px 8px;
    padding: 6px 10px;
    border-radius: 6px;
    background: color-mix(in srgb, #f14c4c 18%, transparent);
  }

  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .spacer {
    flex: 1;
  }

  .confirm-text {
    flex: 1;
    line-height: 1.4;
  }

  .button {
    padding: 6px 14px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
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
    border-color: #f14c4c;
    background: #f14c4c;
    color: #fff;
  }

  .button.subtle {
    border-color: transparent;
    opacity: 0.7;
  }
</style>
