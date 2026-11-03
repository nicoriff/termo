<script lang="ts">
  import { flip } from "svelte/animate";
  import { crossfade, fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { KANBAN_COLUMNS, type KanbanColumn, type KanbanTask } from "$lib/core/kanban-client";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { ago } from "$lib/agents/agent-spec";
  import { beforeAt, blockers, isRunning, needsCancel, queuePlaces, settingsOf, tagHue, tasksIn } from "./kanban-model";
  import { kanbanStore } from "./kanban-store.svelte";

  let { boardId }: { boardId: string } = $props();

  const board = $derived(kanbanStore.get(boardId));
  const places = $derived(board ? queuePlaces(board) : new Map<string, number>());
  const colors = $derived(themeStore.current.colors);

  /** A card leaving one column flies to its place in the other. */
  const [send, receive] = crossfade({
    duration: 320,
    easing: cubicOut,
    fallback: (node) => fade(node, { duration: 160 }),
  });

  /** Ticks every 30 s, for the cards' "5 min ago". */
  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(timer);
  });

  let error = $state("");
  let errorTimer: ReturnType<typeof setTimeout> | null = null;

  function showError(err: unknown) {
    error = String(err);
    if (errorTimer) clearTimeout(errorTimer);
    errorTimer = setTimeout(() => (error = ""), 6000);
  }

  async function act(action: () => Promise<unknown>) {
    try {
      await action();
    } catch (err) {
      showError(err);
    }
  }

  function engineOf(task: KanbanTask) {
    if (!board || !task.role) return null;
    return task.role === "work" ? board.agents.work : task.role === "review" ? board.agents.review : board.agents.summary;
  }

  /** What the agent on it is doing now, as its run says. */
  function activity(task: KanbanTask): string {
    if (!isRunning(task)) return "";
    const run = agentsStore.lastRun[task.agentId];
    return run?.id === task.runId ? run.activity : "";
  }

  function asking(task: KanbanTask): boolean {
    return isRunning(task) && agentsStore.questions.some((question) => question.runId === task.runId);
  }

  // ---- Dragging a card (pointer events: native drags go to the OS in the
  // Windows webview, see TileChrome).

  type Drop = { column: KanbanColumn; index: number };
  let pressed: { task: KanbanTask; x: number; y: number; card: HTMLElement } | null = null;
  let dragging = $state<KanbanTask | null>(null);
  let drop = $state<Drop | null>(null);
  let ghost: HTMLElement | null = null;
  let offset = { x: 0, y: 0 };
  let swallowClick = false;
  let confirming = $state<{ task: KanbanTask; drop: Drop } | null>(null);

  function onCardPointerDown(event: PointerEvent, task: KanbanTask) {
    if (event.button !== 0) return;
    pressed = { task, x: event.clientX, y: event.clientY, card: event.currentTarget as HTMLElement };
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp, { once: true });
  }

  function startDrag(event: PointerEvent) {
    if (!pressed) return;
    const box = pressed.card.getBoundingClientRect();
    offset = { x: pressed.x - box.left, y: pressed.y - box.top };
    ghost = pressed.card.cloneNode(true) as HTMLElement;
    ghost.classList.add("kanban-drag-ghost");
    ghost.style.width = `${box.width}px`;
    document.body.appendChild(ghost);
    document.body.classList.add("termo-dragging-card");
    dragging = pressed.task;
    moveGhost(event);
  }

  function moveGhost(event: PointerEvent) {
    if (ghost) ghost.style.transform = `translate(${event.clientX - offset.x}px, ${event.clientY - offset.y}px) rotate(1.5deg)`;
  }

  /** The column under the pointer and where in it the card would land. */
  function dropAt(x: number, y: number): Drop | null {
    const columnEl = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-kanban-column]");
    if (!columnEl || !board) return null;
    const column = columnEl.dataset.kanbanColumn as KanbanColumn;
    const cards = [...columnEl.querySelectorAll<HTMLElement>("[data-task-id]")].filter(
      (card) => card.dataset.taskId !== dragging?.id,
    );
    let index = cards.length;
    for (const [i, card] of cards.entries()) {
      const box = card.getBoundingClientRect();
      // Measured where it rests, not pushed down by the marker above it.
      const top = box.top - (card.classList.contains("insert-before") ? 41 : 0);
      if (y < top + box.height / 2) {
        index = i;
        break;
      }
    }
    return { column, index };
  }

  /** The card the drop marker shows above, or null for the column's end. */
  const markerBefore = $derived(
    board && drop && dragging ? beforeAt(tasksIn(board, drop.column), dragging.id, drop.index) : null,
  );

  function onPointerMove(event: PointerEvent) {
    if (!pressed) return;
    if (!dragging) {
      if (Math.hypot(event.clientX - pressed.x, event.clientY - pressed.y) < 5) return;
      startDrag(event);
    }
    moveGhost(event);
    drop = dropAt(event.clientX, event.clientY);
  }

  function endDrag() {
    window.removeEventListener("pointermove", onPointerMove);
    ghost?.remove();
    ghost = null;
    document.body.classList.remove("termo-dragging-card");
    pressed = null;
    dragging = null;
    drop = null;
  }

  function onPointerUp() {
    const task = dragging;
    const target = drop;
    if (task) swallowClick = true;
    endDrag();
    if (!task || !target || !board) return;
    const column = tasksIn(board, target.column);
    const before = beforeAt(column, task.id, target.index);
    // Dropped where it was: its place among the others is the same.
    if (task.column === target.column && column.findIndex((other) => other.id === task.id) === target.index) return;
    if (needsCancel(task, target.column)) {
      confirming = { task, drop: target };
      return;
    }
    void act(() => kanbanStore.move(board!.id, task.id, target.column, before));
  }

  function confirmMove() {
    if (!confirming || !board) return;
    const { task, drop: target } = confirming;
    confirming = null;
    const before = beforeAt(tasksIn(board, target.column), task.id, target.index);
    void act(() => kanbanStore.move(board!.id, task.id, target.column, before, true));
  }

  function openTask(task: KanbanTask) {
    if (swallowClick) {
      swallowClick = false;
      return;
    }
    kanbanStore.editTask(boardId, task.id);
  }

  function onCardKey(event: KeyboardEvent, task: KanbanTask) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      kanbanStore.editTask(boardId, task.id);
    }
  }

  const modeLabel = $derived(
    board
      ? board.mode === "worktrees"
        ? t("kanban.board.worktrees", { n: String(board.maxParallel) })
        : t("kanban.board.sequential")
      : "",
  );
</script>

{#if !board}
  <div class="gone">{t("kanban.board.gone")}</div>
{:else}
  <div
    class="kanban-view"
    style="--live:{colors.blue};--done:{colors.green};--ask:{colors.yellow};--bad:{colors.red};--on:{colors.green}"
  >
    <header class="board-header">
      <span class="board-icon">{board.icon || "▦"}</span>
      <div class="board-heading">
        <span class="board-name">{board.name}</span>
        <span class="board-folder" title={board.workDir}>{board.workDir}</span>
      </div>
      <span class="mode-chip">{modeLabel}</span>
      <button
        class="power"
        class:on={board.enabled}
        role="switch"
        aria-checked={board.enabled}
        title={t("kanban.board.switchHint")}
        onclick={() => act(() => kanbanStore.setEnabled(board!.id, !board!.enabled))}
      >
        <span class="power-track"><span class="power-knob"></span></span>
        {board.enabled ? t("kanban.board.on") : t("kanban.board.off")}
      </button>
      <button class="header-button" onclick={() => kanbanStore.editBoard(settingsOf(board!), "agents")}>
        {t("kanban.board.agents")}
      </button>
      <button
        class="header-button icon"
        title={t("kanban.board.settings")}
        aria-label={t("kanban.board.settings")}
        onclick={() => kanbanStore.editBoard(settingsOf(board!))}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="8" cy="8" r="2.2" fill="none" stroke="currentColor" stroke-width="1.2" />
          <path
            d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4"
            stroke="currentColor"
            stroke-width="1.2"
            stroke-linecap="round"
          />
        </svg>
      </button>
      <button class="header-button primary" onclick={() => kanbanStore.editTask(board!.id, null)}>
        + {t("kanban.board.newTask")}
      </button>
    </header>

    {#if !appConfigStore.aiModeEnabled}
      <div class="notice">{t("kanban.board.aiOff")}</div>
    {:else if !board.enabled}
      <div class="notice">{t("kanban.board.offNotice")}</div>
    {:else if board.onHold}
      <div class="notice bad">{board.onHold}</div>
    {/if}
    {#if error}
      <div class="notice bad" transition:fade={{ duration: 120 }}>{error}</div>
    {/if}

    <div class="columns">
      {#each KANBAN_COLUMNS as column (column)}
        {@const tasks = tasksIn(board, column)}
        <section class="column" class:target={drop?.column === column} data-kanban-column={column} aria-label={t(`kanban.columns.${column}`)}>
          <header class="column-head" title={t(`kanban.columnHints.${column}`)}>
            <span class="column-dot {column}"></span>
            <span class="column-name">{t(`kanban.columns.${column}`)}</span>
            <span class="column-count">{tasks.length}</span>
            {#if column === "notReady" || column === "todo"}
              <button
                class="column-add"
                title={t("kanban.board.addHere")}
                aria-label={t("kanban.board.addHere")}
                onclick={() => kanbanStore.editTask(board!.id, null, column)}
              >
                <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 0v10M0 5h10" stroke="currentColor" stroke-width="1.2" /></svg>
              </button>
            {/if}
          </header>
          <div class="cards">
            {#each tasks as task (task.id)}
              {@const place = places.get(task.id)}
              {@const waits = blockers(board, task)}
              {@const agent = engineOf(task)}
              {@const live = activity(task)}
              {@const updated = ago(task.updatedAt || task.createdAt, now)}
              <div
                class="card"
                class:live={isRunning(task)}
                class:asking={asking(task) || task.needsYou !== ""}
                class:done={task.column === "done"}
                class:dragged={dragging?.id === task.id}
                class:insert-before={dragging !== null && drop?.column === column && markerBefore === task.id}
                class:blocked={waits.length > 0 && (column === "todo" || column === "notReady")}
                data-task-id={task.id}
                role="button"
                tabindex="0"
                animate:flip={{ duration: 260, easing: cubicOut }}
                in:receive={{ key: task.id }}
                out:send={{ key: task.id }}
                onpointerdown={(event) => onCardPointerDown(event, task)}
                onclick={() => openTask(task)}
                onkeydown={(event) => onCardKey(event, task)}
              >
                <div class="card-title">
                  <span class="card-number">#{task.number}</span>
                  <span class="card-text">{task.title}</span>
                  {#if isRunning(task) && agent}
                    <span class="card-agent" title={TOOL_DISPLAY_NAMES[agent.engine]}><ConsoleIcon kind={agent.engine} size={13} /></span>
                  {/if}
                </div>
                {#if task.needsYou && !isRunning(task) && !asking(task)}
                  <div class="card-decide" title={task.needsYou}>
                    <span class="decide-mark" aria-hidden="true">!</span>
                    <span class="decide-text">
                      {t(`kanban.decide.short.${task.stuck ?? "other"}`, { n: String(task.reviewRounds || board.maxReviewRounds) })}
                    </span>
                    <button
                      class="decide-button"
                      onclick={(event) => {
                        event.stopPropagation();
                        openTask(task);
                      }}
                    >
                      {t("kanban.decide.button")} →
                    </button>
                  </div>
                {/if}
                {#if task.spec.trim()}
                  <div class="card-spec">{task.spec.trim()}</div>
                {/if}
                {#if task.tags?.length}
                  <div class="card-tags">
                    {#each task.tags as tag (tag)}
                      <span class="tag" style="--hue:{tagHue(tag)}">{tag}</span>
                    {/each}
                  </div>
                {/if}
                {#if live}
                  <div class="card-activity" title={live}>{live}</div>
                {/if}
                <div class="chips">
                  {#if place !== undefined}
                    <span class="chip next" class:first={place === 1}>{t("kanban.chip.next", { n: String(place) })}</span>
                  {/if}
                  {#if waits.length > 0 && column !== "done"}
                    <span class="chip wait">⧗ {t("kanban.chip.blocked", { list: waits.map((n) => `#${n}`).join(", ") })}</span>
                  {/if}
                  {#if isRunning(task) && agent && task.role}
                    <span class="chip running">
                      <span class="pulse-dot"></span>
                      <ConsoleIcon kind={agent.engine} size={11} />
                      {t("kanban.chip.running", {
                        role: t(`kanban.roles.${task.role}`),
                        engine: agent.model || TOOL_DISPLAY_NAMES[agent.engine],
                      })}
                    </span>
                  {/if}
                  {#if asking(task)}
                    <span class="chip ask">❓ {t("kanban.chip.asking")}</span>
                  {/if}
                  {#if task.reviewRounds > 0 && column !== "done"}
                    <span class="chip">{t("kanban.chip.round", { n: String(task.reviewRounds), max: String(board.maxReviewRounds) })}</span>
                  {/if}
                  {#if task.summaryPending}
                    <span class="chip">{t("kanban.chip.summarizing")}</span>
                  {/if}
                  {#if task.outcome}
                    <span class="chip outcome {task.outcome.success}">{t(`kanban.chip.success.${task.outcome.success}`)}</span>
                  {/if}
                  {#if task.branch}
                    <span class="chip branch" title={task.branch}>
                      ⎇ {task.merged ? t("kanban.chip.merged") : t("kanban.chip.branch")}
                    </span>
                  {/if}
                </div>
                <div class="card-foot">
                  <span
                    class="foot-item"
                    class:missing={!task.successCriteria.trim()}
                    title={task.successCriteria.trim() ? t("kanban.card.criteria") : t("kanban.card.noCriteria")}
                  >
                    <svg viewBox="0 0 12 12" aria-hidden="true">
                      <circle cx="6" cy="6" r="4.6" fill="none" stroke="currentColor" stroke-width="1.1" />
                      {#if task.successCriteria.trim()}<path d="M3.8 6.1l1.5 1.5 3-3.1" fill="none" stroke="currentColor" stroke-width="1.1" />{/if}
                    </svg>
                  </span>
                  {#if task.dependsOn.length > 0}
                    <span class="foot-item" title={task.dependsOn.length === 1 ? t("kanban.card.dependsOnOne") : t("kanban.card.dependsOn", { n: String(task.dependsOn.length) })}>
                      <svg viewBox="0 0 12 12" aria-hidden="true">
                        <path d="M5 7l2-2M4.2 5.4L3 6.6a1.7 1.7 0 002.4 2.4l1.2-1.2M7.8 6.6L9 5.4A1.7 1.7 0 006.6 3L5.4 4.2" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" />
                      </svg>
                      {task.dependsOn.length}
                    </span>
                  {/if}
                  {#if task.log.length > 0}
                    <span class="foot-item" title={task.log.length === 1 ? t("kanban.card.logOne") : t("kanban.card.log", { n: String(task.log.length) })}>
                      <svg viewBox="0 0 12 12" aria-hidden="true">
                        <path d="M2.5 2.5h7v5h-4l-2 2v-2h-1z" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
                      </svg>
                      {task.log.length}
                    </span>
                  {/if}
                  {#if task.costUsd}
                    <span class="foot-item" title={t("kanban.task.cost", { usd: `$${task.costUsd.toFixed(2)}` })}>${task.costUsd.toFixed(2)}</span>
                  {/if}
                  <span class="foot-when" title={new Date(task.updatedAt || task.createdAt).toLocaleString()}>
                    {t(updated.key, updated.params)}
                  </span>
                </div>
              </div>
            {/each}
            {#if dragging && drop?.column === column && markerBefore === null}
              <div class="drop-marker" transition:fade={{ duration: 80 }}></div>
            {/if}
            {#if tasks.length === 0 && !(drop?.column === column)}
              <div class="column-empty">{t("kanban.board.emptyColumn")}</div>
            {/if}
          </div>
        </section>
      {/each}
    </div>
  </div>

  {#if confirming}
    <div class="confirm-backdrop" transition:fade={{ duration: 120 }}></div>
    <div class="confirm-anchor">
      <div class="confirm" role="alertdialog" aria-modal="true" aria-label={t("kanban.confirmMove.title")} transition:fly={{ y: -12, duration: 160 }}>
        <h3>{t("kanban.confirmMove.title")}</h3>
        <p>
          {t("kanban.confirmMove.body", {
            n: String(confirming.task.number),
            title: confirming.task.title,
            column: t(`kanban.columns.${confirming.drop.column}`),
          })}
        </p>
        <div class="confirm-actions">
          <button class="header-button" onclick={() => (confirming = null)}>{t("kanban.confirmMove.keep")}</button>
          <button class="header-button danger" onclick={confirmMove}>{t("kanban.confirmMove.confirm")}</button>
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .gone {
    padding: 24px;
    opacity: 0.6;
  }

  .kanban-view {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--terminal-bg, #1a1a1a);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12.5px;
    overflow: hidden;
  }

  .board-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .board-icon {
    font-size: 20px;
  }

  .board-heading {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .board-name {
    font-weight: 600;
    font-size: 14px;
  }

  .board-folder {
    font-size: 11px;
    opacity: 0.55;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mode-chip {
    padding: 2px 8px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    font-size: 11px;
    opacity: 0.8;
    white-space: nowrap;
  }

  .power {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 10px 4px 6px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .power-track {
    position: relative;
    width: 28px;
    height: 16px;
    border-radius: 8px;
    background: rgba(127, 127, 127, 0.35);
    transition: background 160ms ease;
  }

  .power-knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    transition: transform 160ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .power.on {
    border-color: var(--on, #4ec27a);
  }

  .power.on .power-track {
    background: var(--on, #4ec27a);
  }

  .power.on .power-knob {
    transform: translateX(12px);
  }

  .header-button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
  }

  .header-button.icon {
    padding: 5px 7px;
  }

  .header-button svg {
    width: 14px;
    height: 14px;
  }

  .header-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .header-button.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .header-button.danger {
    border-color: var(--bad, #f14c4c);
    background: var(--bad, #f14c4c);
    color: #fff;
  }

  .notice {
    flex-shrink: 0;
    margin: 8px 14px 0;
    padding: 6px 10px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--ask, #d7ba7d) 14%, transparent);
    font-size: 12px;
  }

  .notice.bad {
    background: color-mix(in srgb, var(--bad, #f14c4c) 18%, transparent);
  }

  .columns {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(5, minmax(220px, 1fr));
    gap: 10px;
    padding: 12px 14px;
    overflow-x: auto;
    scrollbar-width: thin;
  }

  .column {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-radius: 8px;
    background: var(--tabbar-bg, rgba(255, 255, 255, 0.03));
    border: 1px solid transparent;
    transition:
      border-color 120ms ease,
      background 120ms ease;
  }

  .column.target {
    border-color: var(--pane-active-border, #3a6df0);
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 6%, var(--tabbar-bg, #1e1e1e));
  }

  .column-head {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 9px 10px 7px;
  }

  .column-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: rgba(127, 127, 127, 0.6);
  }

  .column-dot.todo {
    background: var(--ask, #d7ba7d);
  }

  .column-dot.inProgress {
    background: var(--live, #3a8ef0);
  }

  .column-dot.review {
    background: #b180d7;
  }

  .column-dot.done {
    background: var(--done, #4ec27a);
  }

  .column-name {
    font-weight: 600;
  }

  .column-count {
    flex: 1;
    font-size: 11px;
    opacity: 0.5;
  }

  .column-add {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    opacity: 0.6;
    cursor: pointer;
  }

  .column-add svg {
    width: 10px;
    height: 10px;
  }

  .column-add:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .cards {
    flex: 1;
    min-height: 40px;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 2px 8px 10px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .column-empty {
    padding: 14px 4px;
    text-align: center;
    font-size: 11.5px;
    opacity: 0.4;
  }

  .card {
    padding: 8px 10px;
    border: 1px solid var(--tabbar-border, #333);
    border-radius: 7px;
    background: var(--tabbar-active-bg-solid, #252526);
    cursor: grab;
    user-select: none;
    outline: none;
  }

  .card:hover {
    border-color: color-mix(in srgb, var(--pane-active-border, #3a6df0) 60%, var(--tabbar-border, #333));
  }

  .card:focus-visible {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .card.blocked {
    opacity: 0.7;
  }

  /* An agent on it now: lit in the running color, breathing — as in the
     orchestration's chart. */
  .card.live {
    border-color: var(--live, #3a8ef0);
    background: color-mix(in srgb, var(--live, #3a8ef0) 12%, var(--tabbar-active-bg-solid, #252526));
    animation: live-glow 1.8s ease-in-out infinite;
  }

  /* Waiting on the user: yellow, the first thing seen. */
  .card.asking {
    --live: var(--ask, #d7ba7d);
    border-color: var(--ask, #d7ba7d);
    background: color-mix(in srgb, var(--ask, #d7ba7d) 12%, var(--tabbar-active-bg-solid, #252526));
  }

  .card.done:not(.live) {
    border-color: color-mix(in srgb, var(--done, #4ec27a) 45%, var(--tabbar-border, #333));
  }

  .card.dragged {
    opacity: 0.3;
    animation: none;
  }

  .card-title {
    display: flex;
    gap: 6px;
    line-height: 1.35;
  }

  .card-number {
    flex-shrink: 0;
    font-size: 11px;
    opacity: 0.5;
    padding-top: 1px;
  }

  .card-text {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .card-text {
    flex: 1;
    font-weight: 600;
  }

  .card-agent {
    flex-shrink: 0;
    display: flex;
    padding-top: 1px;
  }

  .card-decide {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    padding: 5px 6px 5px 7px;
    border-radius: 6px;
    background: color-mix(in srgb, #d7ba7d 20%, transparent);
    font-size: 11.5px;
  }

  .decide-mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: #d7ba7d;
    color: #1e1e1e;
    font-size: 10px;
    font-weight: 700;
  }

  .decide-text {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .decide-button {
    flex: none;
    padding: 2px 8px;
    border: 1px solid #d7ba7d;
    border-radius: 5px;
    background: color-mix(in srgb, #d7ba7d 30%, transparent);
    color: inherit;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .decide-button:hover {
    background: color-mix(in srgb, #d7ba7d 45%, transparent);
  }

  .card-spec {
    margin-top: 4px;
    font-size: 11.5px;
    line-height: 1.4;
    opacity: 0.65;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  .card-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }

  .tag {
    padding: 0 7px;
    border: 1px solid hsl(var(--hue) 60% 55% / 0.5);
    border-radius: 999px;
    background: hsl(var(--hue) 60% 50% / 0.18);
    font-size: 10.5px;
    line-height: 16px;
    white-space: nowrap;
  }

  .card-foot {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-top: 7px;
    padding-top: 6px;
    border-top: 1px solid color-mix(in srgb, var(--tabbar-border, #333) 70%, transparent);
    font-size: 10.5px;
    opacity: 0.6;
  }

  .foot-item {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  .foot-item svg {
    width: 12px;
    height: 12px;
  }

  .foot-item.missing {
    opacity: 0.5;
  }

  .foot-when {
    margin-left: auto;
    white-space: nowrap;
  }

  .card-activity {
    margin-top: 5px;
    font-size: 11px;
    opacity: 0.7;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }

  .chips:empty {
    display: none;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 7px;
    border-radius: 999px;
    background: rgba(127, 127, 127, 0.16);
    font-size: 10.5px;
    line-height: 16px;
    white-space: nowrap;
  }

  .chip.next {
    background: color-mix(in srgb, var(--ask, #d7ba7d) 20%, transparent);
  }

  .chip.next.first {
    background: var(--ask, #d7ba7d);
    color: #1e1e1e;
    font-weight: 600;
  }

  .chip.wait {
    opacity: 0.85;
  }

  .chip.running {
    background: color-mix(in srgb, var(--live, #3a8ef0) 25%, transparent);
  }

  .chip.ask {
    background: var(--ask, #d7ba7d);
    color: #1e1e1e;
  }

  .chip.outcome.full {
    background: color-mix(in srgb, var(--done, #4ec27a) 28%, transparent);
  }

  .chip.outcome.partial {
    background: color-mix(in srgb, var(--ask, #d7ba7d) 28%, transparent);
  }

  .chip.outcome.failed {
    background: color-mix(in srgb, var(--bad, #f14c4c) 28%, transparent);
  }

  .chip.branch {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  }

  .pulse-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--live, #3a8ef0);
    animation: pulse 1.4s ease-in-out infinite;
  }

  /* Where the card being dragged would land: room opens above this one. */
  .card.insert-before {
    position: relative;
    margin-top: 41px;
  }

  .card.insert-before::before {
    content: "";
    position: absolute;
    left: -1px;
    right: -1px;
    top: -42px;
    height: 34px;
    border: 2px dashed var(--pane-active-border, #3a6df0);
    border-radius: 7px;
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 8%, transparent);
    pointer-events: none;
  }

  .card {
    transition:
      margin-top 140ms cubic-bezier(0.22, 1, 0.36, 1),
      border-color 160ms ease,
      background 160ms ease,
      opacity 160ms ease;
  }

  .drop-marker {
    flex-shrink: 0;
    height: 34px;
    border: 2px dashed var(--pane-active-border, #3a6df0);
    border-radius: 7px;
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 8%, transparent);
  }

  :global(.kanban-drag-ghost) {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 1000;
    pointer-events: none;
    opacity: 0.92;
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.45);
    cursor: grabbing;
  }

  :global(body.termo-dragging-card),
  :global(body.termo-dragging-card *) {
    cursor: grabbing !important;
  }

  .confirm-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.5);
  }

  .confirm-anchor {
    position: absolute;
    inset: 0;
    z-index: 400;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    pointer-events: none;
  }

  .confirm {
    pointer-events: auto;
    width: min(420px, 90%);
    margin-top: 12vh;
    padding: 16px 18px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 10px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
  }

  .confirm h3 {
    margin: 0 0 8px;
    font-size: 14px;
  }

  .confirm p {
    margin: 0;
    line-height: 1.5;
    opacity: 0.85;
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
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

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
