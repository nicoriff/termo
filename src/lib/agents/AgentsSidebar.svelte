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
  import { agentsStore } from "./agents-store.svelte";
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

  async function remove(agent: AgentSpec) {
    if (confirmDelete !== agent.id) {
      confirmDelete = agent.id;
      return;
    }
    confirmDelete = null;
    menuFor = null;
    await agentsStore.remove(agent.id);
  }

  function toggleMenu(event: MouseEvent, id: string) {
    event.stopPropagation();
    confirmDelete = null;
    menuFor = menuFor === id ? null : id;
  }

  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleAgents"), isMac));
</script>

<svelte:window onpointerdown={() => (menuFor = null)} />

<aside class="agents" transition:slide={{ axis: "x", duration: 180, easing: cubicOut }} aria-label={t("agents.title")}>
  <div class="agents-inner">
    <header class="agents-header">
      <span class="agents-title">{t("agents.title")}</span>
      {#if agentsStore.agents.length > 0}
        <span class="agents-count">{agentsStore.agents.length}</span>
      {/if}
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
    flex-shrink: 0;
    width: 264px;
    overflow: hidden;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .agents-inner {
    width: 264px;
    height: 100%;
    display: flex;
    flex-direction: column;
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
