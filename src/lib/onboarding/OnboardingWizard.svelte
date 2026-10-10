<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { TOOL_IDS, type ToolId } from "$lib/ai-mode/pattern-table";
  import { showWhatsNewOnce } from "$lib/app/whats-new";
  import {
    getAgentHooksStatus,
    installAgentHooks,
    uninstallAgentHooks,
    type AgentHooksStatus,
  } from "$lib/core/agent-hooks-client";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import AgentsStep from "./AgentsStep.svelte";
  import AppearanceStep from "./AppearanceStep.svelte";
  import DoneStep from "./DoneStep.svelte";
  import NavigationStep from "./NavigationStep.svelte";
  import NotificationsStep from "./NotificationsStep.svelte";
  import AgentInstallDialog from "$lib/agent-install/AgentInstallDialog.svelte";
  import TeamStep from "./TeamStep.svelte";
  import WelcomeStep from "./WelcomeStep.svelte";

  /**
   * The setup shown the first time Termo runs, and again from "About": how
   * to move around, the theme, the agents' hooks and the notifications.
   * Every step applies what it changes as it goes, except the hooks, which
   * touch each agent's own settings and wait for "continue".
   */
  /** The numbered ones, between the welcome and the closing screen. */
  const SETUP_STEPS = ["navigation", "team", "appearance", "agents", "notifications"] as const;
  type SetupStep = (typeof SETUP_STEPS)[number];
  const STEPS = ["welcome", ...SETUP_STEPS, "done"] as const;
  const STEP_LABELS: Record<SetupStep, MessageKey> = {
    navigation: "onboarding.steps.navigation",
    team: "onboarding.steps.team",
    appearance: "onboarding.steps.appearance",
    agents: "onboarding.steps.agents",
    notifications: "onboarding.steps.notifications",
  };
  const STEP_ABOUT: Record<SetupStep, MessageKey> = {
    navigation: "onboarding.welcome.items.navigation",
    team: "onboarding.welcome.items.team",
    appearance: "onboarding.welcome.items.appearance",
    agents: "onboarding.welcome.items.agents",
    notifications: "onboarding.welcome.items.notifications",
  };
  const PATH = SETUP_STEPS.map((id) => ({ label: STEP_LABELS[id], about: STEP_ABOUT[id] }));

  // No version remembered yet: this is the run the setup opened by itself on.
  const firstRun = appConfigStore.lastSeenVersion === null;

  let index = $state(0);
  const step = $derived(STEPS[index]);
  /** Which numbered step this is, from 1; 0 on the welcome, past the last
   * one on the closing screen. */
  const position = $derived(index);
  const upcoming = $derived(STEPS[index + 1]);
  /** Which way the last move went, so the next screen comes in from there. */
  let direction = $state(1);
  let dialogEl: HTMLDivElement | undefined = $state();

  let hookStatus = $state<AgentHooksStatus | null>(null);
  const noHooks = () => Object.fromEntries(TOOL_IDS.map((tool) => [tool, false])) as Record<ToolId, boolean>;
  let hookChoice = $state<Record<ToolId, boolean>>(noHooks());
  let hooksBusy = $state(false);
  let hooksError = $state<string | null>(null);
  // Until the user flips one, the proposal follows what's found.
  let hooksTouched = false;

  onMount(() => {
    dialogEl?.focus();
    newTabOptions.refresh();
    void getAgentHooksStatus()
      .then((status) => (hookStatus = status))
      .catch(() => (hookStatus = noHooks()));
  });

  // Proposed: the hooks already there, plus the agents found on this machine.
  $effect(() => {
    const status = hookStatus;
    const clis = newTabOptions.agentClis;
    if (!status || hooksTouched) return;
    hookChoice = Object.fromEntries(
      TOOL_IDS.map((tool) => [tool, status[tool] || clis?.[tool] === true]),
    ) as Record<ToolId, boolean>;
  });

  const hooksPending = $derived(hookStatus !== null && TOOL_IDS.some((tool) => hookChoice[tool] !== hookStatus![tool]));
  const hooked = $derived(TOOL_IDS.filter((tool) => hookStatus?.[tool]));

  /** The agent whose CLI is being installed from the hooks step. */
  let installing = $state<ToolId | null>(null);

  async function installClosed(installed: boolean) {
    const tool = installing;
    installing = null;
    if (!installed || !tool) return;
    newTabOptions.refresh();
    // It may have set up the hooks on its way out: that's a choice made.
    const status = await getAgentHooksStatus().catch(() => hookStatus);
    hookStatus = status;
    if (status?.[tool]) hookChoice = { ...hookChoice, [tool]: true };
  }

  function toggleHook(tool: ToolId) {
    hooksTouched = true;
    hookChoice = { ...hookChoice, [tool]: !hookChoice[tool] };
  }

  /** Brings each agent's hooks in line with the choice. False if one failed,
   * so the step stays up with the reason. */
  async function applyHooks(): Promise<boolean> {
    const before = hookStatus;
    if (!before) return true;
    hooksBusy = true;
    hooksError = null;
    try {
      for (const tool of TOOL_IDS) {
        if (hookChoice[tool] === before[tool]) continue;
        await (hookChoice[tool] ? installAgentHooks(tool) : uninstallAgentHooks(tool));
      }
      // Hooks report to Modo IA: without it they'd be installed for nothing.
      if (TOOL_IDS.some((tool) => hookChoice[tool]) && !appConfigStore.aiModeEnabled) {
        appConfigStore.setAiModeEnabled(true);
      }
      return true;
    } catch (err) {
      hooksError = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      hookStatus = await getAgentHooksStatus().catch(() => before);
      hooksBusy = false;
    }
  }

  function close() {
    uiState.closeOnboarding();
    // On the first run this is what remembers the version (see MainWindow),
    // so the setup isn't offered again.
    void showWhatsNewOnce();
    uiState.refocusTerminals();
  }

  async function next() {
    if (hooksBusy) return;
    if (step === "agents" && hooksPending && !(await applyHooks())) return;
    if (index === STEPS.length - 1) {
      close();
      return;
    }
    direction = 1;
    index++;
  }

  function back() {
    if (hooksBusy || index === 0) return;
    direction = -1;
    index--;
  }

  // The button says where it leads, so each press reads as a step forward.
  const nextLabel = $derived.by(() => {
    if (step === "welcome") return t("onboarding.start");
    if (step === "done") return t("onboarding.finish");
    if (step === "agents" && hooksBusy) return t("onboarding.agents.applying");
    if (step === "agents" && hooksPending) return t("onboarding.agents.apply");
    if (upcoming === "done") return t("onboarding.wrapUp");
    return t("onboarding.nextStep", { name: t(STEP_LABELS[upcoming as SetupStep]) });
  });

  function onKeydown(event: KeyboardEvent) {
    // This window owns the keyboard while it's open (uiState.overlayOpen keeps
    // the window-level handlers out).
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      if (!hooksBusy) close();
      return;
    }
    // Enter on a control is that control's; anywhere else it moves on.
    if (event.key === "Enter" && !(event.target as HTMLElement).closest("button, input, a")) {
      event.preventDefault();
      void next();
    }
  }
</script>

{#if installing}
  <AgentInstallDialog tool={installing} onclose={(installed) => void installClosed(installed)} />
{/if}

<div class="onb-backdrop" transition:fade={{ duration: 140 }}>
  <div
    class="onb-dialog"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={t("onboarding.title")}
    bind:this={dialogEl}
    onkeydown={onKeydown}
    transition:fly={{ y: 16, duration: 180 }}
  >
    <header class="onb-header">
      <img src="/app-icon.png" alt="" />
      <h1>{t("onboarding.heading")}</h1>
      {#if step !== "welcome" && step !== "done"}
        <span class="step-count">{t("onboarding.stepCount", { current: position, total: SETUP_STEPS.length })}</span>
      {/if}
    </header>

    <!-- Not on the welcome: there the same steps are laid out as what's ahead. -->
    {#if step !== "welcome"}
      <ol class="stepper" transition:fade={{ duration: 160 }}>
        {#each SETUP_STEPS as id, at (id)}
          {@const number = at + 1}
          {@const done = number < position}
          <li class:done class:current={number === position} aria-current={number === position ? "step" : undefined}>
            <span class="marker" aria-label={done ? t("onboarding.stepDone", { name: t(STEP_LABELS[id]) }) : undefined}>
              {#if done}
                <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 8.3l2.6 2.6L12 5.2" /></svg>
              {:else}
                {number}
              {/if}
            </span>
            <span class="marker-label">{t(STEP_LABELS[id])}</span>
          </li>
        {/each}
      </ol>
    {/if}

    {#key step}
      <div class="onb-body" in:fly={{ x: 28 * direction, duration: 220 }}>
        {#if step === "welcome"}
          <WelcomeStep {firstRun} path={PATH} />
        {:else if step === "navigation"}
          <NavigationStep />
        {:else if step === "team"}
          <TeamStep />
        {:else if step === "appearance"}
          <AppearanceStep />
        {:else if step === "agents"}
          <AgentsStep
            status={hookStatus}
            choice={hookChoice}
            busy={hooksBusy}
            error={hooksError}
            ontoggle={toggleHook}
            oninstall={(tool) => (installing = tool)}
          />
        {:else if step === "notifications"}
          <NotificationsStep tools={hooked} />
        {:else}
          <DoneStep {hooked} />
        {/if}
      </div>
    {/key}

    <div class="onb-footer">
      {#if step !== "done"}
        <button class="onb-button quiet" disabled={hooksBusy} onclick={close}>
          {t(step === "welcome" ? "onboarding.notNow" : "onboarding.skip")}
        </button>
      {/if}
      <span class="spacer"></span>
      {#if index > 0}
        <button class="onb-button" disabled={hooksBusy} onclick={back}>{t("onboarding.back")}</button>
      {/if}
      <button class="onb-button primary" disabled={hooksBusy} onclick={() => void next()}>{nextLabel}</button>
    </div>
  </div>
</div>

<style>
  .onb-backdrop {
    position: absolute;
    inset: 0;
    z-index: 400;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
  }

  .onb-dialog {
    width: min(860px, 94%);
    height: min(620px, 94%);
    display: flex;
    flex-direction: column;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 10px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    color: var(--titlebar-fg, #d4d4d4);
    overflow: hidden;
  }

  .onb-dialog:focus {
    outline: none;
  }

  .onb-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 20px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .onb-header img {
    width: 22px;
    height: 22px;
  }

  .onb-header h1 {
    flex: 1;
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .step-count {
    font-size: 12px;
    opacity: 0.7;
    font-variant-numeric: tabular-nums;
  }

  /* Where you are in the sequence: a numbered stop per step, joined by a
     line that fills in as they're left behind. */
  .stepper {
    flex-shrink: 0;
    display: flex;
    margin: 0;
    padding: 14px 20px 0;
    list-style: none;
    font-size: 11px;
  }

  .stepper li {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
  }

  /* The stretch of line from the stop before to this one. */
  .stepper li + li::before {
    content: "";
    position: absolute;
    top: 11px;
    right: calc(50% + 16px);
    width: calc(100% - 32px);
    height: 2px;
    background: var(--tabbar-border, #2a2a2a);
    transition: background 240ms ease;
  }

  .stepper li.done + li::before {
    background: var(--pane-active-border, #3a6df0);
  }

  .marker {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: 1.5px solid var(--tabbar-border, #2a2a2a);
    border-radius: 50%;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    transition:
      background 240ms ease,
      border-color 240ms ease,
      color 240ms ease;
  }

  .marker svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .marker-label {
    max-width: 100%;
    opacity: 0.55;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .stepper li.done .marker {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .stepper li.current .marker {
    border-color: var(--pane-active-border, #3a6df0);
    box-shadow: 0 0 0 3px var(--tab-focus-bg, rgba(58, 109, 240, 0.35));
  }

  .stepper li.current .marker-label {
    opacity: 1;
    font-weight: 600;
  }

  .onb-body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 11fr) minmax(0, 9fr);
  }

  /* The two halves every step renders: the picture and the choices. */
  .onb-body :global(.onb-stage) {
    min-width: 0;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 16px 0 16px 20px;
    padding: 20px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    /* The console's own color, dotted like a cutting mat. */
    background:
      radial-gradient(circle, var(--tabbar-border, #2a2a2a) 1px, transparent 1.5px) 0 0 / 16px 16px,
      var(--terminal-bg, #1e1e1e);
    overflow: hidden;
  }

  .onb-body :global(.onb-panel) {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 20px 20px 16px;
    overflow-y: auto;
  }

  .onb-body :global(.onb-panel h2) {
    margin: 0;
    font-size: 19px;
    font-weight: 600;
    line-height: 1.25;
    text-wrap: balance;
  }

  .onb-body :global(.onb-intro) {
    margin: 8px 0 0;
    font-size: 12px;
    line-height: 1.55;
    opacity: 0.8;
  }

  /* Pushed to the bottom of the panel, where the pointers to Preferences go. */
  .onb-body :global(.onb-hint) {
    margin: auto 0 0;
    padding-top: 14px;
    font-size: 11px;
    line-height: 1.5;
    opacity: 0.6;
  }

  .onb-body :global(.onb-error) {
    margin: 10px 0 0;
    font-size: 11px;
    line-height: 1.5;
    color: #f14c4c;
    overflow-wrap: anywhere;
  }

  /* The same checkbox as Preferences. */
  .onb-body :global(.onb-check) {
    flex-shrink: 0;
    margin: 0;
    accent-color: var(--pane-active-border, #3a6df0);
    cursor: pointer;
  }

  .onb-body :global(.onb-check:disabled) {
    cursor: default;
  }

  .onb-footer {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .spacer {
    flex: 1;
  }

  .onb-button {
    padding: 7px 16px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .onb-button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .onb-button:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 2px;
  }

  .onb-button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .onb-button.quiet {
    border-color: transparent;
    opacity: 0.7;
  }

  .onb-button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .onb-button.primary:hover:not(:disabled) {
    background: var(--pane-active-border, #3a6df0);
    filter: brightness(1.1);
  }

  /* A narrow window: the picture goes on top, shorter. */
  @media (max-width: 680px) {
    .onb-body {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: 190px minmax(0, 1fr);
    }

    .onb-body :global(.onb-stage) {
      margin: 12px 16px 0;
      padding: 12px;
    }
  }
</style>
