<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import {
    agentsBoard,
    removeBoardEntry,
    type AgentSpec,
    type BoardEntry,
    type ImportReport,
    type Project,
  } from "$lib/core/agents-client";
  import { groupByProject, projectTone } from "./project-spec";
  import { newAgentSpec } from "./agent-spec";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { SIDEBAR_WIDTH, agentsStore } from "./agents-store.svelte";
  import { agentState, ago, nextRunLabel, type AgentTone } from "./agent-spec";
  import type { MessageKey } from "$lib/i18n/translate";
  import OrchestrationList from "./OrchestrationList.svelte";

  const timeOf = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const weekdayOf = (ms: number) => new Date(ms).toLocaleDateString([], { weekday: "short" });

  /** The board the agents share, while it's open. */
  let boardOpen = $state(false);
  let board = $state<Record<string, BoardEntry>>({});
  const usesBoard = $derived(agentsStore.localAgents.some((agent) => agent.collaboration.board));
  // Any run ending may have written on it.
  const runsSignature = $derived(
    Object.values(agentsStore.lastRun)
      .map((run) => `${run.id}:${run.status}`)
      .join(),
  );
  $effect(() => {
    void runsSignature;
    if (!boardOpen) return;
    void agentsBoard()
      .then((entries) => (board = entries))
      .catch(() => {});
  });

  async function removeNote(key: string) {
    board = await removeBoardEntry(key).catch(() => board);
  }

  const nameOf = (id: string) => agentsStore.localAgents.find((agent) => agent.id === id)?.name ?? "";

  // Projects: each a group of agents; agents in none go last.
  const groups = $derived(groupByProject(agentsStore.localAgents, agentsStore.localProjects));
  const loose = $derived(groups.get("") ?? []);
  let projectMenuFor = $state<string | null>(null);
  let deletingProject = $state<string | null>(null);
  let projectErrors = $state<Record<string, string>>({});
  /** The agent whose "Move to project" list is open. */
  let movingFor = $state<string | null>(null);

  function toggleProjectMenu(event: MouseEvent, id: string) {
    event.stopPropagation();
    menuFor = null;
    deletingProject = null;
    projectMenuFor = projectMenuFor === id ? null : id;
  }

  function newAgentIn(project: Project) {
    const spec = newAgentSpec(crypto.randomUUID());
    spec.projectId = project.id;
    spec.workDir = project.workDir;
    uiState.openAgentWizard(spec, true);
  }

  async function runProject(id: string, rehearsal: boolean) {
    projectMenuFor = null;
    const { [id]: _, ...rest } = projectErrors;
    projectErrors = rest;
    try {
      const started = await agentsStore.runProject(id, rehearsal);
      const agent = agentsStore.localAgents.find((candidate) => candidate.id === started.agentId);
      workspaceStore.openAgentRun(started.agentId, started.id, agent?.name ?? "");
    } catch (reason) {
      projectErrors = { ...projectErrors, [id]: String(reason) };
    }
  }

  async function removeProject(id: string, deleteAgents: boolean) {
    projectMenuFor = null;
    deletingProject = null;
    try {
      await agentsStore.removeProject(id, deleteAgents);
    } catch (reason) {
      notice = { error: true, lines: [String(reason)] };
    }
  }

  async function exportProject(id: string, name: string) {
    projectMenuFor = null;
    const slug = name.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "proyecto";
    const path = await save({
      title: t("agents.projects.exportTitle"),
      defaultPath: `${slug}.termo-project.json`,
      filters: [{ name: t("agents.projects.fileKind"), extensions: ["json"] }],
    }).catch(() => null);
    if (!path) return;
    try {
      await agentsStore.exportProject(id, path);
      notice = { error: false, lines: [t("agents.portable.exported", { path })] };
    } catch (reason) {
      notice = { error: true, lines: [String(reason)] };
    }
  }

  async function move(agentId: string, projectId: string) {
    menuFor = null;
    movingFor = null;
    try {
      await agentsStore.moveAgent(agentId, projectId);
    } catch (reason) {
      errors = { ...errors, [agentId]: String(reason) };
    }
  }

  // Dragging an agent onto a project (or "No project"). With pointer events
  // rather than HTML5 drag and drop, like the tabs (see TileChrome): on
  // Windows the webview hands native drags to the OS, and dragover/drop
  // never reach the page.
  let dragging = $state<string | null>(null);
  /** The group under the pointer while dragging: a project id, "" for none. */
  let dropOn = $state<string | null>(null);
  /** A drag ends in a click on the row it started from; that click isn't one. */
  let swallowClick = false;

  /** The group at a point: whatever carries `data-drop-project`. */
  function dropTargetAt(x: number, y: number): string | null {
    const target = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop-project]");
    return target ? (target.dataset.dropProject ?? null) : null;
  }

  function onRowPointerDown(event: PointerEvent, agent: AgentSpec) {
    // Its buttons (run, menu) are buttons, not handles.
    if (event.button !== 0 || (event.target as HTMLElement).closest("button")) return;
    const startX = event.clientX;
    const startY = event.clientY;
    let ghost: HTMLElement | null = null;

    const follow = (e: PointerEvent) => {
      if (!dragging) {
        if (Math.hypot(e.clientX - startX, e.clientY - startY) <= 4) return;
        dragging = agent.id;
        menuFor = null;
        document.body.classList.add("termo-dragging-agent");
        ghost = document.createElement("div");
        ghost.textContent = `${agent.icon || "🤖"} ${agent.name}`;
        Object.assign(ghost.style, {
          position: "fixed",
          zIndex: "1000",
          pointerEvents: "none",
          padding: "4px 10px",
          borderRadius: "6px",
          border: "1px solid var(--pane-active-border, #3a6df0)",
          background: "var(--tabbar-active-bg-solid, #252526)",
          color: "var(--titlebar-fg, #d4d4d4)",
          font: "12px system-ui, sans-serif",
          boxShadow: "0 6px 18px rgba(0, 0, 0, 0.4)",
          whiteSpace: "nowrap",
        });
        document.body.appendChild(ghost);
      }
      if (ghost) {
        ghost.style.left = `${e.clientX + 12}px`;
        ghost.style.top = `${e.clientY + 8}px`;
      }
      dropOn = dropTargetAt(e.clientX, e.clientY);
    };

    const end = (e: PointerEvent) => {
      window.removeEventListener("pointermove", follow);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      if (!dragging) return;
      const target = e.type === "pointerup" ? dropTargetAt(e.clientX, e.clientY) : null;
      dragging = null;
      dropOn = null;
      ghost?.remove();
      document.body.classList.remove("termo-dragging-agent");
      swallowClick = true;
      setTimeout(() => (swallowClick = false), 0);
      if (target !== null && target !== agent.projectId) void move(agent.id, target);
    };

    window.addEventListener("pointermove", follow);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  }

  let menuFor = $state<string | null>(null);
  let confirmDelete = $state<string | null>(null);
  let errors = $state<Record<string, string>>({});

  function toneColor(tone: AgentTone): string {
    const c = themeStore.current.colors;
    switch (tone) {
      case "running":
        return c.blue;
      case "success":
        return c.green;
      case "error":
        return c.red;
      case "paused":
        return c.yellow;
      case "idle":
        return c.brightBlack;
    }
  }

  function openAgent(agent: AgentSpec) {
    const last = agentsStore.lastRun[agent.id];
    if (last) workspaceStore.openAgentRun(agent.id, last.id, agent.name);
    else uiState.openAgentWizard(agent, false);
  }

  async function run(agent: AgentSpec) {
    const { [agent.id]: _, ...rest } = errors;
    errors = rest;
    try {
      const started = await agentsStore.run(agent);
      workspaceStore.openAgentRun(agent.id, started.id, agent.name);
    } catch (reason) {
      errors = { ...errors, [agent.id]: String(reason) };
    }
  }

  function duplicate(agent: AgentSpec) {
    menuFor = null;
    uiState.openAgentWizard(
      { ...structuredClone($state.snapshot(agent)), id: crypto.randomUUID(), name: `${agent.name} (2)`, deploy: { github: null }, createdAt: 0 },
      true,
    );
  }

  /** The outcome of the last import or export, until it's dismissed. */
  let notice = $state<{ error: boolean; lines: string[] } | null>(null);

  /** What to look at before turning imported agents on — the same for a
   * project's agents as for loose ones. */
  function reportLines(report: ImportReport): string[] {
    const lines = [t("agents.portable.paused")];
    for (const item of report.review) {
      if (item.runsCommands) lines.push(t("agents.portable.reviewRuns", { agent: item.agent }));
      if (item.escalates) lines.push(t("agents.portable.reviewEscalates", { agent: item.agent }));
      for (const command of item.commands) lines.push(t("agents.portable.reviewCommand", { agent: item.agent, command }));
      for (const url of item.urls) lines.push(t("agents.portable.reviewUrl", { agent: item.agent, url }));
    }
    if (report.movedToOwnFolder.length > 0)
      lines.push(t("agents.portable.ownFolder", { names: report.movedToOwnFolder.join(", ") }));
    if (report.needsSecrets.length > 0)
      lines.push(t("agents.portable.needsSecrets", { names: report.needsSecrets.join(", ") }));
    return lines;
  }

  async function importAgents() {
    const picked = await open({
      multiple: true,
      title: t("agents.portable.importTitle"),
      filters: [{ name: t("agents.portable.fileKind"), extensions: ["json"] }],
    }).catch(() => null);
    const all = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (all.length === 0) return;
    // A whole project comes in as one, with its agents.
    const isProject = (path: string) => path.toLowerCase().endsWith(".termo-project.json");
    const paths = all.filter((path) => !isProject(path));
    try {
      for (const path of all.filter(isProject)) {
        const report = await agentsStore.importProject(path);
        notice = {
          error: false,
          lines: [t("agents.projects.imported", { names: report.imported.join(", ") }), ...reportLines(report)],
        };
      }
      if (paths.length === 0) return;
      const report = await agentsStore.importFrom(paths);
      notice = {
        error: false,
        lines: [t("agents.portable.imported", { names: report.imported.join(", ") }), ...reportLines(report)],
      };
    } catch (reason) {
      notice = { error: true, lines: [String(reason)] };
    }
  }

  async function exportAgent(agent: AgentSpec) {
    menuFor = null;
    const slug = agent.name.toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "agente";
    const path = await save({
      title: t("agents.portable.exportTitle"),
      defaultPath: `${slug}.termo-agent.json`,
      filters: [{ name: t("agents.portable.fileKind"), extensions: ["json"] }],
    }).catch(() => null);
    if (!path) return;
    try {
      await agentsStore.exportTo([agent.id], path);
      const lines = [t("agents.portable.exported", { path })];
      if (agent.secrets.length > 0) lines.push(t("agents.portable.secretsStay"));
      notice = { error: false, lines };
    } catch (reason) {
      notice = { error: true, lines: [String(reason)] };
    }
  }

  async function remove(agent: AgentSpec) {
    if (confirmDelete !== agent.id) {
      confirmDelete = agent.id;
      return;
    }
    confirmDelete = null;
    menuFor = null;
    await agentsStore.remove(agent.id);
  }

  let resizing = $state(false);

  /** Dragging the right edge: the width follows the pointer, and is
   * remembered once it's let go. */
  function startResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startWidth = agentsStore.width;
    resizing = true;
    const move = (e: PointerEvent) => agentsStore.setWidth(startWidth + e.clientX - startX);
    const end = () => {
      resizing = false;
      agentsStore.setWidth(agentsStore.width, true);
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  }

  function onResizeKey(event: KeyboardEvent) {
    const step = event.shiftKey ? 40 : 10;
    if (event.key === "ArrowLeft") agentsStore.setWidth(agentsStore.width - step, true);
    else if (event.key === "ArrowRight") agentsStore.setWidth(agentsStore.width + step, true);
    else return;
    event.preventDefault();
  }

  function toggleMenu(event: MouseEvent, id: string) {
    event.stopPropagation();
    confirmDelete = null;
    menuFor = menuFor === id ? null : id;
  }

  /** The panel's two sections: teams designed for one job, and the agents
   * the user builds and keeps. Which one was open is remembered. */
  type Section = "orchestration" | "local";
  const SECTIONS: { id: Section; label: MessageKey }[] = [
    { id: "orchestration", label: "agents.sections.orchestration" },
    { id: "local", label: "agents.sections.local" },
  ];
  const SECTION_KEY = "termo.agents.section";

  function loadSection(): Section {
    try {
      return localStorage.getItem(SECTION_KEY) === "orchestration" ? "orchestration" : "local";
    } catch {
      return "local";
    }
  }

  let section = $state<Section>(loadSection());

  function setSection(next: Section) {
    section = next;
    try {
      localStorage.setItem(SECTION_KEY, next);
    } catch {
      // Only a convenience.
    }
  }

  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleAgents"), isMac));
</script>

<svelte:window
  onpointerdown={() => {
    menuFor = null;
    movingFor = null;
    projectMenuFor = null;
    deletingProject = null;
  }}
/>

<aside
  class="agents"
  class:resizing
  style="--agents-width:{agentsStore.width}px"
  transition:slide={{ axis: "x", duration: 180, easing: cubicOut }}
  aria-label={t("agents.title")}
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="resize-handle"
    role="separator"
    aria-orientation="vertical"
    aria-label={t("agents.resize")}
    aria-valuemin={SIDEBAR_WIDTH.min}
    aria-valuemax={SIDEBAR_WIDTH.max}
    aria-valuenow={agentsStore.width}
    title={t("agents.resize")}
    tabindex="0"
    onpointerdown={startResize}
    ondblclick={() => agentsStore.setWidth(SIDEBAR_WIDTH.initial, true)}
    onkeydown={onResizeKey}
  ></div>
  <div class="agents-inner">
    <header class="agents-header">
      <span class="agents-title">{t("agents.title")}</span>
      {#if section === "local" && agentsStore.localAgents.length > 0}
        <span class="agents-count">{agentsStore.localAgents.length}</span>
      {/if}
      {#if section === "orchestration"}
        <button
          class="icon-button"
          title={t("agents.orchestration.new")}
          aria-label={t("agents.orchestration.new")}
          onclick={() => uiState.openOrchestrationDialog()}
        >
          <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1.5v9M1.5 6h9" stroke="currentColor" stroke-width="1.3" /></svg>
        </button>
      {:else}
      {#if usesBoard || boardOpen}
        <button
          class="icon-button"
          class:active={boardOpen}
          title={boardOpen ? t("agents.board.close") : t("agents.board.open")}
          aria-label={boardOpen ? t("agents.board.close") : t("agents.board.open")}
          aria-pressed={boardOpen}
          onclick={() => (boardOpen = !boardOpen)}
        >
          <svg viewBox="0 0 12 12" aria-hidden="true">
            <rect x="1.5" y="2" width="9" height="8" rx="1" fill="none" stroke="currentColor" stroke-width="1.1" />
            <path d="M3.5 4.5h5M3.5 6.5h3" stroke="currentColor" stroke-width="1.1" />
          </svg>
        </button>
      {/if}
      <button
        class="icon-button"
        title={t("agents.portable.import")}
        aria-label={t("agents.portable.import")}
        onclick={() => void importAgents()}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true">
          <path d="M6 1.5v6M3.5 5L6 7.5 8.5 5M1.5 8.5v2h9v-2" fill="none" stroke="currentColor" stroke-width="1.2" />
        </svg>
      </button>
      <button
        class="icon-button"
        title={t("agents.projects.new")}
        aria-label={t("agents.projects.new")}
        onclick={() => uiState.openProjectDialog(null)}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true">
          <path d="M1.5 3h3l1 1h5v6h-9z M6 5.5v3M4.5 7h3" fill="none" stroke="currentColor" stroke-width="1.1" />
        </svg>
      </button>
      <button
        class="icon-button"
        title={t("agents.new")}
        aria-label={t("agents.new")}
        onclick={() => uiState.openAgentWizard(null, true)}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1.5v9M1.5 6h9" stroke="currentColor" stroke-width="1.3" /></svg>
      </button>
      {/if}
      <button
        class="icon-button"
        title={`${t("agents.close")} (${shortcut})`}
        aria-label={t("agents.close")}
        onclick={() => agentsStore.setOpen(false)}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    <div class="sections" role="tablist" aria-label={t("agents.sections.label")}>
      {#each SECTIONS as item (item.id)}
        <button
          class="section-tab"
          class:current={section === item.id}
          role="tab"
          aria-selected={section === item.id}
          onclick={() => setSection(item.id)}
        >
          {t(item.label)}
        </button>
      {/each}
    </div>

    {#if section === "orchestration"}
      <OrchestrationList />
    {:else}
    {#if notice}
      <div class="notice" class:error={notice.error}>
        <div class="notice-text">
          {#each notice.lines as line (line)}<p>{line}</p>{/each}
        </div>
        <button class="icon-button" aria-label={t("agents.portable.dismiss")} onclick={() => (notice = null)}>
          <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
        </button>
      </div>
    {/if}

    {#if boardOpen}
      <section class="board" transition:slide={{ duration: 140, easing: cubicOut }} aria-label={t("agents.board.title")}>
        <div class="board-title">{t("agents.board.title")}</div>
        {#each Object.entries(board) as [key, entry] (key)}
          {@const age = ago(entry.at, agentsStore.now)}
          <div class="board-note">
            <div class="board-note-head">
              <span class="board-key">{key}</span>
              <button
                class="icon-button"
                title={t("agents.board.remove")}
                aria-label={t("agents.board.remove")}
                onclick={() => void removeNote(key)}
              >
                <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2 2l6 6M8 2L2 8" stroke="currentColor" stroke-width="1.1" /></svg>
              </button>
            </div>
            <div class="board-value">{entry.value}</div>
            <div class="board-by">{t("agents.board.by", { agent: entry.by, age: t(age.key, age.params) })}</div>
          </div>
        {:else}
          <p class="board-empty">{t("agents.board.empty")}</p>
        {/each}
      </section>
    {/if}

    {#snippet agentRow(agent: AgentSpec)}
        {@const last = agentsStore.lastRun[agent.id] ?? null}
        {@const state = agentState(agent, last)}
        {@const running = last?.status === "running"}
        {@const age = last?.endedAt ? ago(last.endedAt, agentsStore.now) : null}
        {@const next = agentsStore.nextRun[agent.id]}
        {@const nextLabel = next && !running ? nextRunLabel(next, agentsStore.now, timeOf, weekdayOf) : null}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="agent-row"
          class:menu-open={menuFor === agent.id}
          class:dragging={dragging === agent.id}
          role="button"
          tabindex="0"
          title={agent.description || agent.task}
          onpointerdown={(e) => onRowPointerDown(e, agent)}
          onclick={() => {
            if (!swallowClick) openAgent(agent);
          }}
          onkeydown={(e) => e.key === "Enter" && openAgent(agent)}
        >
          <span class="agent-icon">{agent.icon || "🤖"}</span>
          <div class="agent-text">
            <div class="agent-name-line">
              <span class="agent-name">{agent.name}</span>
              <ConsoleIcon kind={agent.engine} size={11} />
              {#if agent.deploy.github}
                <button
                  class="deploy-badge"
                  title={t("agents.github.openActions")}
                  onclick={(e) => {
                    e.stopPropagation();
                    if (agent.deploy.github?.repo) void openUrl(`https://github.com/${agent.deploy.github.repo}/actions`);
                  }}>GitHub</button
                >
              {/if}
            </div>
            <div class="agent-state" style="color:{toneColor(state.tone)}">
              <span class="state-dot" class:pulsing={running} style="background:{toneColor(state.tone)}"></span>
              <span class="state-text">
                {t(state.key, state.params)}{#if age && !running}{" · "}{t(age.key, age.params)}{/if}
              </span>
            </div>
            {#if nextLabel}
              <div class="agent-next">⏱ {t(nextLabel.key, nextLabel.params)}</div>
            {:else if agent.triggers.watch && agent.enabled && !running}
              <div class="agent-next">👁 {t("agents.next.watching")}</div>
            {/if}
            {#if agent.triggers.git && agent.enabled && !running}
              <div class="agent-next">
                🔗 {t(agent.triggers.git.event === "push" ? "agents.next.onPush" : "agents.next.onCommit")}
              </div>
            {/if}
            {#if agent.triggers.webhook && agent.enabled && !running}
              <div class="agent-next">🌐 {t("agents.next.webhook")}</div>
            {/if}
            {#if agent.triggers.after && agent.enabled && !running}
              <div class="agent-next">⛓ {t("agents.next.after", { name: nameOf(agent.triggers.after.agentId) })}</div>
            {/if}
            {#if agent.collaboration.contacts.length > 0 && agent.enabled && !running}
              <div class="agent-next">🤝 {t("agents.next.team")}</div>
            {/if}
            {#if errors[agent.id]}
              <div class="agent-error">{errors[agent.id]}</div>
            {/if}
          </div>
          <div class="agent-actions">
            {#if running}
              <button
                class="icon-button"
                title={t("agents.actions.stop")}
                aria-label={t("agents.actions.stop")}
                onclick={(e) => {
                  e.stopPropagation();
                  void agentsStore.stop(agent.id);
                }}
              >
                <svg viewBox="0 0 10 10" aria-hidden="true"><rect x="2" y="2" width="6" height="6" fill="currentColor" /></svg>
              </button>
            {:else}
              <button
                class="icon-button"
                title={t("agents.actions.run")}
                aria-label={t("agents.actions.run")}
                onclick={(e) => {
                  e.stopPropagation();
                  void run(agent);
                }}
              >
                <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2.5 1.5l6 3.5-6 3.5z" fill="currentColor" /></svg>
              </button>
            {/if}
            <button
              class="icon-button"
              title={t("agents.actions.more")}
              aria-label={t("agents.actions.more")}
              onpointerdown={(e) => e.stopPropagation()}
              onclick={(e) => toggleMenu(e, agent.id)}
            >
              <svg viewBox="0 0 12 12" aria-hidden="true">
                <circle cx="2.5" cy="6" r="1" fill="currentColor" /><circle cx="6" cy="6" r="1" fill="currentColor" /><circle
                  cx="9.5"
                  cy="6"
                  r="1"
                  fill="currentColor"
                />
              </svg>
            </button>
          </div>

          {#if menuFor === agent.id}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="agent-menu" role="menu" tabindex="-1" onpointerdown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
              <button role="menuitem" onclick={() => ((menuFor = null), uiState.openAgentWizard(agent, false))}>
                {t("agents.actions.edit")}
              </button>
              {#if last}
                <button role="menuitem" onclick={() => ((menuFor = null), workspaceStore.openAgentRun(agent.id, last.id, agent.name))}>
                  {t("agents.actions.runs")}
                </button>
              {/if}
              <button role="menuitem" onclick={() => duplicate(agent)}>{t("agents.actions.duplicate")}</button>
              <button role="menuitem" aria-expanded={movingFor === agent.id} onclick={() => (movingFor = movingFor === agent.id ? null : agent.id)}>
                {t("agents.projects.moveTo")} ›
              </button>
              {#if movingFor === agent.id}
                {#each agentsStore.localProjects.filter((project) => project.id !== agent.projectId) as project (project.id)}
                  <button role="menuitem" class="submenu-item" onclick={() => void move(agent.id, project.id)}>
                    {project.icon} {project.name}
                  </button>
                {/each}
                {#if agent.projectId}
                  <button role="menuitem" class="submenu-item" onclick={() => void move(agent.id, "")}>
                    {t("agents.projects.moveOut")}
                  </button>
                {/if}
                <button role="menuitem" class="submenu-item" onclick={() => ((menuFor = null), uiState.openProjectDialog(null))}>
                  + {t("agents.projects.new")}
                </button>
              {/if}
              <button role="menuitem" onclick={() => void exportAgent(agent)}>{t("agents.actions.export")}</button>
              <button role="menuitem" onclick={() => ((menuFor = null), void agentsStore.setEnabled(agent.id, !agent.enabled))}>
                {agent.enabled ? t("agents.actions.pause") : t("agents.actions.resume")}
              </button>
              <div class="menu-divider"></div>
              <button role="menuitem" class="danger" onclick={() => void remove(agent)}>
                {confirmDelete === agent.id ? t("agents.actions.confirmDelete") : t("agents.actions.delete")}
              </button>
            </div>
          {/if}
        </div>
    {/snippet}

    <div class="agents-list">
      {#each agentsStore.localProjects as project (project.id)}
        {@const members = groups.get(project.id) ?? []}
        {@const collapsed = agentsStore.collapsed.includes(project.id)}
        {@const tone = projectTone(members, agentsStore.lastRun)}
        <div class="project-group" class:drop-target={dropOn === project.id} data-drop-project={project.id}>
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="project-row"
            class:menu-open={projectMenuFor === project.id}
            role="button"
            tabindex="0"
            title={project.goal || project.name}
            onclick={() => workspaceStore.openProject(project.id, project.name)}
            onkeydown={(e) => e.key === "Enter" && workspaceStore.openProject(project.id, project.name)}
          >
            <button
              class="chevron"
              class:collapsed
              aria-expanded={!collapsed}
              aria-label={t("agents.projects.toggle")}
              title={t("agents.projects.toggle")}
              onclick={(e) => {
                e.stopPropagation();
                agentsStore.toggleCollapsed(project.id);
              }}
            >
              <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M3 2l4 3-4 3" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
            </button>
            <span class="project-icon">{project.icon || "📁"}</span>
            <span class="project-name">{project.name}</span>
            <span class="agents-count">{members.length}</span>
            {#if members.length > 0}
              <span class="state-dot" class:pulsing={tone === "running"} style="background:{toneColor(tone)}"></span>
            {/if}
            <div class="agent-actions">
              <button
                class="icon-button"
                title={t("agents.projects.newAgentHere")}
                aria-label={t("agents.projects.newAgentHere")}
                onclick={(e) => {
                  e.stopPropagation();
                  newAgentIn(project);
                }}
              >
                <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1.5v9M1.5 6h9" stroke="currentColor" stroke-width="1.3" /></svg>
              </button>
              <button
                class="icon-button"
                title={t("agents.projects.more")}
                aria-label={t("agents.projects.more")}
                onpointerdown={(e) => e.stopPropagation()}
                onclick={(e) => toggleProjectMenu(e, project.id)}
              >
                <svg viewBox="0 0 12 12" aria-hidden="true">
                  <circle cx="2.5" cy="6" r="1" fill="currentColor" /><circle cx="6" cy="6" r="1" fill="currentColor" /><circle
                    cx="9.5"
                    cy="6"
                    r="1"
                    fill="currentColor"
                  />
                </svg>
              </button>
            </div>
            {#if projectMenuFor === project.id}
              {@const allPaused = members.length > 0 && members.every((member) => !member.enabled)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="agent-menu" role="menu" tabindex="-1" onpointerdown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
                {#if deletingProject === project.id}
                  <p class="menu-note">{t("agents.projects.deleteAsk", { n: String(members.length) })}</p>
                  <button role="menuitem" onclick={() => void removeProject(project.id, false)}>{t("agents.projects.deleteKeep")}</button>
                  <button role="menuitem" class="danger" onclick={() => void removeProject(project.id, true)}>
                    {t("agents.projects.deleteAll")}
                  </button>
                  <button role="menuitem" onclick={() => (deletingProject = null)}>{t("agents.wizard.cancel")}</button>
                {:else}
                  <button role="menuitem" onclick={() => ((projectMenuFor = null), workspaceStore.openProject(project.id, project.name))}>
                    {t("agents.projects.menu.open")}
                  </button>
                  <button role="menuitem" disabled={!project.entryAgentId} onclick={() => void runProject(project.id, false)}>
                    {t("agents.projects.menu.run")}
                  </button>
                  <button role="menuitem" disabled={!project.entryAgentId} onclick={() => void runProject(project.id, true)}>
                    {t("agents.projects.menu.rehearse")}
                  </button>
                  <button
                    role="menuitem"
                    disabled={members.length === 0}
                    onclick={() => ((projectMenuFor = null), void agentsStore.setProjectEnabled(project.id, allPaused))}
                  >
                    {allPaused ? t("agents.projects.menu.resumeAll") : t("agents.projects.menu.pauseAll")}
                  </button>
                  <button role="menuitem" onclick={() => ((projectMenuFor = null), uiState.openProjectDialog(project))}>
                    {t("agents.projects.menu.settings")}
                  </button>
                  <button role="menuitem" onclick={() => void exportProject(project.id, project.name)}>
                    {t("agents.projects.menu.export")}
                  </button>
                  <div class="menu-divider"></div>
                  <button
                    role="menuitem"
                    class="danger"
                    onclick={() => (members.length > 0 ? (deletingProject = project.id) : void removeProject(project.id, false))}
                  >
                    {t("agents.projects.menu.delete")}
                  </button>
                {/if}
              </div>
            {/if}
          </div>
          {#if projectErrors[project.id]}
            <div class="agent-error project-error">{projectErrors[project.id]}</div>
          {/if}
          {#if !collapsed}
            <div class="project-members" transition:slide={{ duration: 120, easing: cubicOut }}>
              {#each members as agent (agent.id)}
                {@render agentRow(agent)}
              {:else}
                <p class="group-empty">{t("agents.projects.groupEmpty")}</p>
              {/each}
            </div>
          {/if}
        </div>
      {/each}

      {#if agentsStore.localProjects.length > 0 && (loose.length > 0 || dragging)}
        <div class="loose-group" class:drop-target={dropOn === ""} data-drop-project="">
          <div class="loose-title">{t("agents.projects.loose")}</div>
          {#each loose as agent (agent.id)}
            {@render agentRow(agent)}
          {/each}
        </div>
      {:else}
        {#each loose as agent (agent.id)}
          {@render agentRow(agent)}
        {/each}
      {/if}
      {#if agentsStore.localAgents.length === 0 && agentsStore.localProjects.length === 0}
        <div class="agents-empty">
          <div class="empty-icon">🤖</div>
          <p class="empty-title">{t("agents.empty.title")}</p>
          <p class="empty-text">{t("agents.empty.text")}</p>
          <button class="empty-button" onclick={() => uiState.openAgentWizard(null, true)}>{t("agents.empty.create")}</button>
          <button class="empty-link" onclick={() => void importAgents()}>{t("agents.portable.orImport")}</button>
          <button class="empty-link" onclick={() => uiState.openProjectDialog(null)}>{t("agents.projects.orNew")}</button>
        </div>
      {/if}
    </div>
    {/if}
  </div>
</aside>

<style>
  /* Docked at the window's left edge: the consoles make room for it rather
     than being covered. The width animates; the content inside keeps its
     own fixed width so nothing reflows while it slides. */
  .agents {
    position: relative;
    flex-shrink: 0;
    width: var(--agents-width, 264px);
    overflow: hidden;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .board {
    margin: 0 10px 8px;
    padding: 8px 10px;
    max-height: 40%;
    overflow-y: auto;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
  }

  .board-title {
    margin-bottom: 6px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.55;
  }

  .board-note + .board-note {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .board-note-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }

  .board-key {
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .board-value {
    margin-top: 2px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 7.5em;
    overflow: hidden;
  }

  .board-by,
  .board-empty {
    margin: 3px 0 0;
    font-size: 11px;
    opacity: 0.55;
  }

  .agents-inner {
    width: var(--agents-width, 264px);
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  /* A thin strip over the right border, wider than it looks so it's easy to
     grab. */
  .resize-handle {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 25;
    width: 6px;
    cursor: ew-resize;
    outline: none;
  }

  .resize-handle:hover,
  .resize-handle:focus-visible,
  .agents.resizing .resize-handle {
    background: var(--divider-hover-bg, #3a6df0);
    opacity: 0.6;
  }

  /* While dragging, the pointer shouldn't select text or flicker back to an
     arrow over the consoles. */
  .agents.resizing {
    user-select: none;
  }

  :global(body:has(.agents.resizing)) {
    cursor: ew-resize;
  }

  .agents-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 6px 0 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .agents-title {
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.8;
  }

  .agents-count {
    flex: 1;
    font-size: 10.5px;
    opacity: 0.5;
  }

  .icon-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    opacity: 0.7;
    cursor: pointer;
  }

  .icon-button.active {
    opacity: 1;
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.2));
  }

  .agents-header .icon-button:first-of-type {
    margin-left: auto;
  }

  .icon-button svg {
    width: 11px;
    height: 11px;
  }

  .icon-button:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .sections {
    flex-shrink: 0;
    display: flex;
    gap: 2px;
    padding: 0 8px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .section-tab {
    flex: 1;
    padding: 6px 4px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    opacity: 0.6;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .section-tab:hover {
    opacity: 0.9;
  }

  .section-tab.current {
    border-bottom-color: var(--pane-active-border, #3a6df0);
    opacity: 1;
  }

  .agents-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 0;
    scrollbar-width: thin;
  }

  .agent-row {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 8px 8px 8px 12px;
    cursor: pointer;
    outline: none;
  }

  .agent-row:hover,
  .agent-row:focus-visible,
  .agent-row.menu-open,
  .project-row:hover,
  .project-row:focus-visible,
  .project-row.menu-open {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .agent-row.dragging {
    opacity: 0.45;
  }

  .project-group,
  .loose-group {
    border-radius: 6px;
  }

  :global(body.termo-dragging-agent),
  :global(body.termo-dragging-agent *) {
    cursor: grabbing !important;
    user-select: none;
  }

  .project-group.drop-target,
  .loose-group.drop-target {
    outline: 1px dashed var(--pane-active-border, #3a6df0);
    outline-offset: -2px;
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.12));
  }

  .project-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px 6px 4px;
    cursor: pointer;
    outline: none;
    font-weight: 600;
  }

  .project-row:hover .agent-actions,
  .project-row:focus-within .agent-actions,
  .project-row.menu-open .agent-actions {
    opacity: 1;
  }

  .project-row .agents-count {
    flex: 0 0 auto;
    font-weight: normal;
  }

  .chevron {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    padding: 0;
    border: none;
    background: transparent;
    color: inherit;
    opacity: 0.6;
    cursor: pointer;
    transform: rotate(90deg);
    transition: transform 120ms ease;
  }

  .chevron.collapsed {
    transform: rotate(0deg);
  }

  .chevron svg {
    width: 9px;
    height: 9px;
  }

  .project-icon {
    font-size: 14px;
  }

  .project-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .project-members .agent-row {
    padding-left: 24px;
  }

  .project-error {
    padding: 0 10px 4px 28px;
  }

  .group-empty {
    margin: 0;
    padding: 2px 12px 8px 28px;
    font-size: 11px;
    opacity: 0.5;
  }

  .loose-title {
    margin-top: 6px;
    padding: 6px 12px 4px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.55;
  }

  .agent-menu .submenu-item {
    padding-left: 22px;
    font-size: 11.5px;
  }

  .agent-menu button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .menu-note {
    margin: 4px 8px 6px;
    font-size: 11.5px;
    line-height: 1.4;
  }

  .agent-icon {
    font-size: 17px;
    line-height: 20px;
  }

  .agent-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .agent-name-line {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .agent-name {
    font-weight: 600;
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .agent-state {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
  }

  .state-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.9;
  }

  .state-dot {
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    border-radius: 50%;
  }

  .state-dot.pulsing {
    animation: pulse 1.2s ease-in-out infinite;
  }

  .deploy-badge {
    flex-shrink: 0;
    padding: 0 5px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 9.5px;
    opacity: 0.75;
    cursor: pointer;
  }

  .deploy-badge:hover {
    opacity: 1;
  }

  .agent-next {
    font-size: 10.5px;
    opacity: 0.55;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .agent-error {
    font-size: 11px;
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }

  /* Only on hover (or with the menu open), so a list of agents reads as a
     list and not as a toolbar. */
  .agent-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 100ms ease;
  }

  .agent-row:hover .agent-actions,
  .agent-row:focus-within .agent-actions,
  .agent-row.menu-open .agent-actions {
    opacity: 1;
  }

  .agent-menu {
    position: absolute;
    top: 32px;
    right: 8px;
    z-index: 20;
    min-width: 150px;
    padding: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: var(--tabbar-active-bg-solid, #252526);
    box-shadow: 0 8px 22px rgba(0, 0, 0, 0.4);
  }

  .agent-menu button {
    display: block;
    width: 100%;
    padding: 5px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .agent-menu button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .agent-menu .danger {
    color: var(--error-fg, #f48771);
  }

  .menu-divider {
    height: 1px;
    margin: 4px 2px;
    background: var(--tabbar-border, #2a2a2a);
  }

  .notice {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 8px 8px 0;
    padding: 8px 6px 8px 10px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
    font-size: 11.5px;
    line-height: 1.45;
  }

  .notice.error {
    border-left-color: var(--error-fg, #f48771);
  }

  .notice-text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .notice-text p {
    margin: 0 0 3px;
  }

  .empty-link {
    margin-top: 10px;
    border: none;
    background: transparent;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    cursor: pointer;
  }

  .agents-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 28px 18px;
    text-align: center;
  }

  .empty-icon {
    font-size: 28px;
  }

  .empty-title {
    margin: 10px 0 4px;
    font-weight: 600;
    font-size: 13px;
  }

  .empty-text {
    margin: 0 0 14px;
    line-height: 1.5;
    opacity: 0.65;
  }

  .empty-button {
    padding: 6px 14px;
    border: none;
    border-radius: 5px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font: inherit;
    cursor: pointer;
  }

  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
</style>
