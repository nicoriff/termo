<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";

  import type { KanbanColumn, KanbanLogEntry } from "$lib/core/kanban-client";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { actorKey, addTag, boardTags, isRunning, tagHue } from "./kanban-model";
  import { kanbanStore, type KanbanDialog } from "./kanban-store.svelte";

  // Read once: KanbanDialogs mounts a fresh one for each dialog opened.
  const { boardId, taskId, column } = kanbanStore.dialog as Extract<KanbanDialog, { kind: "task" }>;

  const board = $derived(kanbanStore.get(boardId));
  const task = $derived(taskId ? board?.tasks.find((candidate) => candidate.id === taskId) : undefined);
  const isNew = taskId === null;

  // What the editor holds, from the task as it was when opened: the agents'
  // changes to it (its log, its column) show live, the user's text stays theirs.
  const initial = taskId ? kanbanStore.get(boardId)?.tasks.find((candidate) => candidate.id === taskId) : undefined;
  let title = $state(initial?.title ?? "");
  let spec = $state(initial?.spec ?? "");
  let criteria = $state(initial?.successCriteria ?? "");
  let dependsOn = $state<string[]>(initial?.dependsOn ?? []);
  let tags = $state<string[]>(initial?.tags ?? []);
  let tagInput = $state("");
  /** The board's other tags, for picking one already used. */
  const suggestions = $derived(
    (board ? boardTags(board) : [])
      .filter((tag) => !tags.some((other) => other.toLowerCase() === tag.toLowerCase()))
      .filter((tag) => tag.toLowerCase().includes(tagInput.trim().replace(/^#+/, "").toLowerCase()))
      .slice(0, 8),
  );

  function takeTag() {
    tags = addTag(tags, tagInput);
    tagInput = "";
  }

  function onTagKey(event: KeyboardEvent) {
    if (event.key === "Enter" || event.key === ",") {
      event.preventDefault();
      event.stopPropagation();
      takeTag();
    } else if (event.key === "Backspace" && tagInput === "" && tags.length > 0) {
      tags = tags.slice(0, -1);
    }
  }
  let startIn = $state<KanbanColumn>(column === "todo" ? "todo" : "notReady");
  let tab = $state<"details" | "log" | "summary">("details");
  let note = $state("");
  let busy = $state(false);
  let error = $state("");
  let confirmingDelete = $state(false);
  let titleInput = $state<HTMLInputElement | null>(null);

  const others = $derived(board?.tasks.filter((candidate) => candidate.id !== taskId) ?? []);
  const dirty = $derived(
    isNew ||
      (initial !== undefined &&
        (title !== initial.title ||
          spec !== initial.spec ||
          criteria !== initial.successCriteria ||
          tags.join("\n") !== (initial.tags ?? []).join("\n") ||
          dependsOn.join() !== initial.dependsOn.join())),
  );

  onMount(() => {
    void tick().then(() => titleInput?.focus());
  });

  function close() {
    kanbanStore.closeDialog();
  }

  async function run(action: () => Promise<unknown>, closeAfter = false) {
    busy = true;
    error = "";
    try {
      await action();
      if (closeAfter) close();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function save() {
    if (!title.trim()) {
      error = t("kanban.task.titlePlaceholder");
      return;
    }
    void run(
      () =>
        kanbanStore.upsertTask(boardId, {
          id: taskId ?? "",
          title,
          spec,
          successCriteria: criteria,
          tags: addTag(tags, tagInput),
          dependsOn,
          column: isNew ? startIn : undefined,
        }),
      true,
    );
  }

  function toggleDependency(id: string) {
    dependsOn = dependsOn.includes(id) ? dependsOn.filter((dep) => dep !== id) : [...dependsOn, id];
  }

  function addNote() {
    const text = note.trim();
    if (!text || !taskId) return;
    void run(async () => {
      await kanbanStore.noteTask(boardId, taskId, text);
      note = "";
    });
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && tab === "details") {
      event.preventDefault();
      save();
    }
  }

  function who(entry: KanbanLogEntry): string {
    const name = t(actorKey(entry.actor));
    if (entry.actor.kind !== "agent") return name;
    const engine = entry.actor.model || TOOL_DISPLAY_NAMES[entry.actor.engine];
    return `${name} · ${engine}`;
  }

  function what(entry: KanbanLogEntry): string {
    if (entry.kind === "moved" && entry.from && entry.to) {
      return t("kanban.log.moved", { from: t(`kanban.columns.${entry.from}`), to: t(`kanban.columns.${entry.to}`) });
    }
    return t(`kanban.log.${entry.kind}`);
  }

  const money = (usd: number) => `$${usd.toFixed(2)}`;
  const formatDateTime = (ms: number) => new Date(ms).toLocaleString([], { dateStyle: "short", timeStyle: "short" });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop" transition:fade={{ duration: 140 }}></div>
<div class="anchor">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-label={task ? t("kanban.task.editTitle", { n: String(task.number) }) : t("kanban.task.newTitle")}
    transition:fly={{ y: -16, duration: 180, easing: cubicOut }}
    onpointerdown={(event) => event.stopPropagation()}
  >
    <header class="dialog-header">
      <div class="heading">
        <span class="dialog-title">{task ? t("kanban.task.editTitle", { n: String(task.number) }) : t("kanban.task.newTitle")}</span>
        {#if task}
          <span class="sub">{t(`kanban.columns.${task.column}`)}{board ? ` · ${board.name}` : ""}</span>
        {/if}
      </div>
      <button class="close-button" aria-label={t("kanban.task.close")} onclick={close}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    {#if task}
      <nav class="tabs">
        {#each ["details", "log", "summary"] as const as candidate (candidate)}
          <button class="tab" class:current={tab === candidate} onclick={() => (tab = candidate)}>
            {t(`kanban.task.tabs.${candidate}`)}
            {#if candidate === "log"}<span class="count">{task.log.length}</span>{/if}
          </button>
        {/each}
      </nav>

      {#if task.needsYou || isRunning(task) || (task.branch && !task.merged && task.column === "done")}
        <div class="status">
          {#if task.needsYou}
            <div class="status-row ask">
              <span class="status-text"><strong>{t("kanban.task.needsYou")}:</strong> {task.needsYou}</span>
              <button class="button" disabled={busy} title={t("kanban.task.resolveHint")} onclick={() => run(() => kanbanStore.resolveTask(boardId, task!.id))}>
                {t("kanban.task.resolve")}
              </button>
            </div>
          {/if}
          {#if isRunning(task)}
            <div class="status-row live">
              <span class="status-text">
                <span class="pulse-dot"></span>
                {task.role ? t(`kanban.roles.${task.role}`) : ""}
              </span>
              <button class="button" onclick={() => workspaceStore.openAgentRun(task!.agentId, task!.runId, task!.title)}>
                {t("kanban.task.openRun")}
              </button>
              <button class="button" disabled={busy} onclick={() => run(() => kanbanStore.cancelTask(boardId, task!.id))}>
                {t("kanban.task.cancelRun")}
              </button>
            </div>
          {/if}
          {#if task.branch && !task.merged && task.column === "done" && !isRunning(task)}
            <div class="status-row">
              <span class="status-text mono">⎇ {task.branch}</span>
              <button
                class="button primary"
                disabled={busy}
                title={t("kanban.task.mergeHint", { branch: task.branch })}
                onclick={() => run(() => kanbanStore.mergeTask(boardId, task!.id))}
              >
                {t("kanban.task.merge")}
              </button>
            </div>
          {/if}
        </div>
      {/if}
    {/if}

    <div class="body">
      {#if tab === "details" || !task}
        <label class="label" for="kanban-task-title">{t("kanban.task.title")}</label>
        <input id="kanban-task-title" class="field" bind:this={titleInput} bind:value={title} placeholder={t("kanban.task.titlePlaceholder")} />

        <label class="label" for="kanban-task-spec">{t("kanban.task.spec")}</label>
        <p class="hint">{t("kanban.task.specHint")}</p>
        <textarea id="kanban-task-spec" class="field" rows="7" bind:value={spec} placeholder={t("kanban.task.specPlaceholder")}></textarea>

        <label class="label" for="kanban-task-criteria">{t("kanban.task.criteria")}</label>
        <p class="hint">{t("kanban.task.criteriaHint")}</p>
        <textarea id="kanban-task-criteria" class="field" rows="3" bind:value={criteria} placeholder={t("kanban.task.criteriaPlaceholder")}></textarea>

        <label class="label" for="kanban-task-tags">{t("kanban.task.tags")}</label>
        <p class="hint">{t("kanban.task.tagsHint")}</p>
        <div class="tag-field">
          {#each tags as tag (tag)}
            <span class="tag" style="--hue:{tagHue(tag)}">
              {tag}
              <button
                class="tag-remove"
                aria-label={t("kanban.task.removeTag", { tag })}
                onclick={() => (tags = tags.filter((other) => other !== tag))}
              >×</button>
            </span>
          {/each}
          <input
            id="kanban-task-tags"
            class="tag-input"
            bind:value={tagInput}
            placeholder={tags.length === 0 ? t("kanban.task.tagsPlaceholder") : ""}
            onkeydown={onTagKey}
            onblur={takeTag}
          />
        </div>
        {#if suggestions.length > 0}
          <div class="chips suggestions">
            {#each suggestions as tag (tag)}
              <button class="tag suggestion" style="--hue:{tagHue(tag)}" onclick={() => (tags = addTag(tags, tag))}>+ {tag}</button>
            {/each}
          </div>
        {/if}

        <div class="label">{t("kanban.task.dependsOn")}</div>
        <p class="hint">{t("kanban.task.dependsOnHint")}</p>
        {#if others.length === 0}
          <p class="hint">{t("kanban.task.dependsOnNone")}</p>
        {:else}
          <div class="chips">
            {#each others as other (other.id)}
              <button class="chip" class:selected={dependsOn.includes(other.id)} onclick={() => toggleDependency(other.id)}>
                #{other.number} {other.title}
              </button>
            {/each}
          </div>
        {/if}

        {#if isNew}
          <div class="label">{t("kanban.task.column")}</div>
          <div class="chips">
            {#each ["notReady", "todo"] as const as candidate (candidate)}
              <button class="chip" class:selected={startIn === candidate} onclick={() => (startIn = candidate)}>
                {t(`kanban.columns.${candidate}`)}
              </button>
            {/each}
          </div>
        {/if}
      {:else if tab === "log"}
        {#if task.log.length === 0}
          <p class="hint">{t("kanban.task.logEmpty")}</p>
        {/if}
        <ol class="log">
          {#each task.log as entry, index (index)}
            <li class="entry {entry.actor.kind}">
              <div class="entry-head">
                <span class="entry-who">{who(entry)}</span>
                <span class="entry-what">{what(entry)}</span>
                <span class="entry-when">{formatDateTime(entry.at)}</span>
              </div>
              {#if entry.text}
                <div class="entry-text">{entry.text}</div>
              {/if}
            </li>
          {/each}
        </ol>
        <div class="note-row">
          <input
            class="field"
            bind:value={note}
            placeholder={t("kanban.task.notePlaceholder")}
            onkeydown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                addNote();
              }
            }}
          />
          <button class="button" disabled={busy || !note.trim()} onclick={addNote}>{t("kanban.task.addNote")}</button>
        </div>
      {:else}
        {#if task.outcome}
          <div class="outcome {task.outcome.success}">
            <span class="outcome-chip">{t(`kanban.chip.success.${task.outcome.success}`)}</span>
            <div class="prose">{task.outcome.summary}</div>
            {#if task.outcome.notes}
              <div class="label">{t("kanban.task.outcomeNotes")}</div>
              <div class="prose">{task.outcome.notes}</div>
            {/if}
          </div>
        {:else}
          <p class="hint">{t("kanban.task.noOutcome")}</p>
        {/if}
        {#if task.workSummary}
          <div class="label">{t("kanban.task.workSummary")}</div>
          <div class="prose">{task.workSummary}</div>
        {/if}
        {#if task.reviewNotes}
          <div class="label">{t("kanban.task.reviewNotes")}</div>
          <div class="prose">{task.reviewNotes}</div>
        {/if}
        <div class="facts">
          {#if task.costUsd !== null}<span>{t("kanban.task.cost", { usd: money(task.costUsd) })}</span>{/if}
          {#if task.branch}<span class="mono">{t("kanban.task.branch", { branch: task.branch })}</span>{/if}
        </div>
      {/if}
    </div>

    {#if error}
      <div class="error">{error}</div>
    {/if}

    <footer class="footer">
      {#if task}
        {#if confirmingDelete}
          <span class="confirm-text">{t("kanban.task.deleteConfirm", { n: String(task.number), title: task.title })}</span>
          <button class="button" onclick={() => (confirmingDelete = false)}>{t("kanban.task.cancel")}</button>
          <button class="button danger" disabled={busy} onclick={() => run(() => kanbanStore.deleteTask(boardId, task!.id), true)}>
            {t("kanban.task.delete")}
          </button>
        {:else}
          <button class="button subtle" onclick={() => (confirmingDelete = true)}>{t("kanban.task.delete")}</button>
          <span class="spacer"></span>
          <button class="button" onclick={close}>{dirty ? t("kanban.task.cancel") : t("kanban.task.close")}</button>
          {#if tab === "details"}
            <button class="button primary" disabled={busy || !dirty} onclick={save}>{t("kanban.task.save")}</button>
          {/if}
        {/if}
      {:else}
        <span class="spacer"></span>
        <button class="button" onclick={close}>{t("kanban.task.cancel")}</button>
        <button class="button primary" disabled={busy || !title.trim()} onclick={save}>{t("kanban.task.create")}</button>
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
    width: min(680px, 94%);
    max-height: 88%;
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

  .count {
    margin-left: 4px;
    font-size: 10.5px;
    opacity: 0.6;
  }

  .status {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 18px 0;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: 7px;
    background: rgba(127, 127, 127, 0.12);
  }

  .status-row.ask {
    background: color-mix(in srgb, #d7ba7d 18%, transparent);
  }

  .status-row.live {
    background: color-mix(in srgb, #3a8ef0 16%, transparent);
  }

  .status-text {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    line-height: 1.4;
    overflow-wrap: anywhere;
  }

  .pulse-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #3a8ef0;
    animation: pulse 1.4s ease-in-out infinite;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 18px 14px;
    scrollbar-width: thin;
  }

  .label {
    display: block;
    margin: 12px 0 4px;
    font-weight: 600;
  }

  .hint {
    margin: 0 0 6px;
    font-size: 11.5px;
    opacity: 0.6;
    line-height: 1.4;
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

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    max-width: 100%;
    padding: 4px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }

  .chip.selected {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.2));
  }

  .tag-field {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    padding: 5px 7px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
  }

  .tag-field:focus-within {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .tag {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 8px;
    border: 1px solid hsl(var(--hue) 60% 55% / 0.55);
    border-radius: 999px;
    background: hsl(var(--hue) 60% 50% / 0.2);
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    line-height: 18px;
  }

  .tag-remove {
    padding: 0 0 0 2px;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 13px;
    line-height: 1;
    opacity: 0.6;
    cursor: pointer;
  }

  .tag-remove:hover {
    opacity: 1;
  }

  .tag-input {
    flex: 1;
    min-width: 120px;
    padding: 2px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    outline: none;
  }

  .suggestions {
    margin-top: 6px;
  }

  .tag.suggestion {
    opacity: 0.75;
    cursor: pointer;
  }

  .tag.suggestion:hover {
    opacity: 1;
  }

  .log {
    list-style: none;
    margin: 4px 0 10px;
    padding: 0 0 0 12px;
    border-left: 2px solid var(--tabbar-border, #333);
  }

  .entry {
    position: relative;
    padding: 6px 0 8px;
  }

  .entry::before {
    content: "";
    position: absolute;
    left: -17px;
    top: 10px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--tabbar-border, #555);
  }

  .entry.agent::before {
    background: #3a8ef0;
  }

  .entry.user::before {
    background: #4ec27a;
  }

  .entry.console::before {
    background: #b180d7;
  }

  .entry-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
  }

  .entry-who {
    font-weight: 600;
  }

  .entry-what {
    opacity: 0.85;
  }

  .entry-when {
    margin-left: auto;
    font-size: 11px;
    opacity: 0.5;
  }

  .entry-text,
  .prose {
    margin-top: 4px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    line-height: 1.5;
    opacity: 0.9;
  }

  .note-row {
    display: flex;
    gap: 8px;
  }

  .outcome {
    padding: 10px 12px;
    border-radius: 8px;
    background: rgba(127, 127, 127, 0.1);
  }

  .outcome.full {
    background: color-mix(in srgb, #4ec27a 12%, transparent);
  }

  .outcome.partial {
    background: color-mix(in srgb, #d7ba7d 14%, transparent);
  }

  .outcome.failed {
    background: color-mix(in srgb, #f14c4c 14%, transparent);
  }

  .outcome-chip {
    font-weight: 600;
  }

  .facts {
    display: flex;
    gap: 14px;
    margin-top: 12px;
    font-size: 11.5px;
    opacity: 0.7;
  }

  .mono {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
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

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
