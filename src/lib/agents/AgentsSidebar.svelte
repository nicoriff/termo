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
  import type { AgentSpec } from "$lib/core/agents-client";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { SIDEBAR_WIDTH, agentsStore } from "./agents-store.svelte";
  import { agentState, ago, nextRunLabel, type AgentTone } from "./agent-spec";

  const timeOf = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const weekdayOf = (ms: number) => new Date(ms).toLocaleDateString([], { weekday: "short" });

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

  async function importAgents() {
    const picked = await open({
      multiple: true,
      title: t("agents.portable.importTitle"),
      filters: [{ name: t("agents.portable.fileKind"), extensions: ["json"] }],
    }).catch(() => null);
    const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
    if (paths.length === 0) return;
    try {
      const report = await agentsStore.importFrom(paths);
      const lines = [
        t("agents.portable.imported", { names: report.imported.join(", ") }),
        t("agents.portable.paused"),
      ];
      for (const item of report.review) {
        if (item.runsCommands) lines.push(t("agents.portable.reviewRuns", { agent: item.agent }));
        for (const command of item.commands) lines.push(t("agents.portable.reviewCommand", { agent: item.agent, command }));
        for (const url of item.urls) lines.push(t("agents.portable.reviewUrl", { agent: item.agent, url }));
      }
      if (report.movedToOwnFolder.length > 0)
        lines.push(t("agents.portable.ownFolder", { names: report.movedToOwnFolder.join(", ") }));
      if (report.needsSecrets.length > 0)
        lines.push(t("agents.portable.needsSecrets", { names: report.needsSecrets.join(", ") }));
      notice = { error: false, lines };
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

  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleAgents"), isMac));
</script>

<svelte:window onpointerdown={() => (menuFor = null)} />

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
      {#if agentsStore.agents.length > 0}
        <span class="agents-count">{agentsStore.agents.length}</span>
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
        title={t("agents.new")}
        aria-label={t("agents.new")}
        onclick={() => uiState.openAgentWizard(null, true)}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1.5v9M1.5 6h9" stroke="currentColor" stroke-width="1.3" /></svg>
      </button>
      <button
        class="icon-button"
        title={`${t("agents.close")} (${shortcut})`}
        aria-label={t("agents.close")}
        onclick={() => agentsStore.setOpen(false)}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

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

    <div class="agents-list">
      {#each agentsStore.agents as agent (agent.id)}
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
          role="button"
          tabindex="0"
          title={agent.description || agent.task}
          onclick={() => openAgent(agent)}
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
      {:else}
        <div class="agents-empty">
          <div class="empty-icon">🤖</div>
          <p class="empty-title">{t("agents.empty.title")}</p>
          <p class="empty-text">{t("agents.empty.text")}</p>
          <button class="empty-button" onclick={() => uiState.openAgentWizard(null, true)}>{t("agents.empty.create")}</button>
          <button class="empty-link" onclick={() => void importAgents()}>{t("agents.portable.orImport")}</button>
        </div>
      {/each}
    </div>
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
  .agent-row.menu-open {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
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
