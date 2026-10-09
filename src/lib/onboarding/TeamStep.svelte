<script lang="ts">
  import { onMount } from "svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import type { ToolId } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";

  type Scene = "local" | "orchestration";
  const SCENES: Scene[] = ["local", "orchestration"];
  const BEAT_MS = 1100;
  const SCENE_LENGTH: Record<Scene, number> = { local: 6, orchestration: 8 };
  /** The beat that stands for the whole scene when motion is reduced. */
  const STILL_BEAT: Record<Scene, number> = { local: 3, orchestration: 5 };

  const TITLES: Record<Scene, MessageKey> = {
    local: "onboarding.team.local.title",
    orchestration: "onboarding.team.orchestration.title",
  };
  const BODIES: Record<Scene, MessageKey> = {
    local: "onboarding.team.local.body",
    orchestration: "onboarding.team.orchestration.body",
  };

  /** What can set a local agent off; the second one fires in the scene. */
  const TRIGGERS: MessageKey[] = ["agents.next.watching", "agents.next.onCommit", "agents.next.onPush"];
  const FIRING_TRIGGER = 1;

  /** The team the orchestrator puts together: a different engine each, which
   * is the point. */
  const TASKS: { engine: ToolId; label: MessageKey }[] = [
    { engine: "claude", label: "onboarding.team.tasks.design" },
    { engine: "codex", label: "onboarding.team.tasks.build" },
    { engine: "gemini", label: "onboarding.team.tasks.test" },
  ];
  /** The beat the first task starts on; each next one starts as the one
   * before it finishes. */
  const FIRST_TASK_BEAT = 3;

  let scene = $state<Scene>("local");
  let beat = $state(0);
  let still = $state(false);
  // Once the user picks a scene it loops, instead of moving on to the other.
  let pinned = false;

  onMount(() => {
    still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (still) return;
    const timer = setInterval(() => {
      if (beat + 1 < SCENE_LENGTH[scene]) {
        beat++;
        return;
      }
      if (!pinned) scene = SCENES[(SCENES.indexOf(scene) + 1) % SCENES.length];
      beat = 0;
    }, BEAT_MS);
    return () => clearInterval(timer);
  });

  function pick(next: Scene) {
    pinned = true;
    scene = next;
    beat = 0;
  }

  const now = $derived(still ? STILL_BEAT[scene] : beat);

  // Local agent: waits, its trigger fires, it runs, it's done.
  const localState = $derived<"idle" | "running" | "done">(now < 2 ? "idle" : now < 4 ? "running" : "done");
  const localStateLabel = $derived(
    t(
      localState === "idle"
        ? "agents.state.never"
        : localState === "running"
          ? "agents.state.running"
          : "agents.state.lastOk",
    ),
  );

  function taskStatus(index: number): "pending" | "running" | "done" {
    const start = FIRST_TASK_BEAT + index;
    return now < start ? "pending" : now === start ? "running" : "done";
  }

  const agentsKey = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleAgents"), isMac));
</script>

<div class="onb-stage">
  <div class="demo" role="img" aria-label={t("onboarding.team.demoLabel", { name: t(TITLES[scene]) })}>
    {#if scene === "local"}
      <div class="local">
        <ul class="triggers">
          {#each TRIGGERS as trigger, index}
            <li class:firing={index === FIRING_TRIGGER && now >= 1 && now < 4}>{t(trigger)}</li>
          {/each}
        </ul>
        <i class="link" class:live={localState === "running"}></i>
        <div class="card agent" class:running={localState === "running"} class:done={localState === "done"}>
          <span class="agent-head">
            <ConsoleIcon kind="codex" size={20} />
            <strong>{t("onboarding.team.sampleAgent")}</strong>
          </span>
          <span class="agent-state"><i class="dot {localState}"></i>{localStateLabel}</span>
          <span class="bar"><i></i></span>
        </div>
      </div>
    {:else}
      <div class="orchestration">
        <div class="card prompt">{t("onboarding.team.samplePrompt")}</div>
        <div class="card orchestrator" class:shown={now >= 1}>
          <ConsoleIcon kind="claude" size={20} />
          <span>
            <strong>{t("agents.orchestration.view.orchestrator")}</strong>
            <small>{t("agents.orchestration.view.orchestratorRole")}</small>
          </span>
        </div>
        <ul class="tasks" class:shown={now >= 2}>
          {#each TASKS as task, index}
            {@const status = taskStatus(index)}
            <li class="card task {status}">
              <ConsoleIcon kind={task.engine} size={16} />
              <span class="task-text">
                {t(task.label)}
                <small>{TOOL_DISPLAY_NAMES[task.engine]}</small>
              </span>
              <span class="task-status"><i class="dot {status}"></i>{t(`agents.orchestration.task.${status}`)}</span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>
</div>

<div class="onb-panel">
  <h2>{t("onboarding.team.title")}</h2>
  <p class="onb-intro">{t("onboarding.team.intro")}</p>
  <ul class="scenes">
    {#each SCENES as id (id)}
      <li>
        <button class="scene" class:active={scene === id} aria-pressed={scene === id} onclick={() => pick(id)}>
          <span class="scene-title">{t(TITLES[id])}</span>
          {#if scene === id}
            <span class="scene-body">{t(BODIES[id])}</span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
  <p class="onb-hint">{t("onboarding.team.hint", { keys: agentsKey })}</p>
</div>

<style>
  .demo {
    width: min(100%, 400px);
    font-size: 11px;
  }

  .card {
    box-sizing: border-box;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
  }

  .dot {
    display: inline-block;
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.35;
  }

  .dot.running {
    opacity: 1;
    background: var(--pane-active-border, #3a6df0);
    animation: pulse 900ms ease-in-out infinite;
  }

  .dot.done {
    opacity: 1;
    background: #22a55b;
  }

  @keyframes pulse {
    50% {
      transform: scale(1.5);
      opacity: 0.5;
    }
  }

  /* Local agent: what sets it off, on the left; the agent, on the right. */
  .local {
    display: flex;
    align-items: center;
  }

  .triggers {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .triggers li {
    padding: 5px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 12px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    white-space: nowrap;
    opacity: 0.6;
    transition:
      border-color 200ms ease,
      opacity 200ms ease;
  }

  .triggers li.firing {
    border-color: var(--pane-active-border, #3a6df0);
    opacity: 1;
  }

  .link {
    flex: 1;
    min-width: 16px;
    height: 2px;
    background: var(--tabbar-border, #2a2a2a);
    transition: background 200ms ease;
  }

  .link.live {
    background: var(--pane-active-border, #3a6df0);
  }

  .agent {
    flex-shrink: 0;
    width: 46%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.3);
    transition: border-color 200ms ease;
  }

  .agent.running {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .agent-head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }

  .agent-state {
    display: flex;
    align-items: center;
    gap: 6px;
    opacity: 0.85;
  }

  .bar {
    height: 3px;
    border-radius: 2px;
    background: var(--tabbar-border, #2a2a2a);
    overflow: hidden;
  }

  .bar i {
    display: block;
    width: 0;
    height: 100%;
    background: var(--pane-active-border, #3a6df0);
  }

  .agent.running .bar i {
    width: 85%;
    transition: width 2000ms linear;
  }

  .agent.done .bar i {
    width: 100%;
    background: #22a55b;
    transition:
      width 200ms ease,
      background 200ms ease;
  }

  /* Orchestration: the request, who plans it, and the team's tasks. */
  .orchestration {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .prompt {
    max-width: 100%;
    padding: 8px 12px;
    border-style: dashed;
    font-size: 12px;
    text-align: center;
  }

  .orchestrator {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 16px;
    padding: 8px 14px;
    font-size: 12px;
    opacity: 0;
    transform: translateY(-6px);
    transition:
      opacity 260ms ease,
      transform 260ms ease;
  }

  .orchestrator.shown,
  .tasks.shown {
    opacity: 1;
    transform: none;
  }

  /* The lines down the tree, from the card above. */
  .orchestrator::before,
  .tasks::before {
    content: "";
    position: absolute;
    left: 50%;
    bottom: 100%;
    width: 2px;
    height: 16px;
    background: var(--tabbar-border, #2a2a2a);
  }

  .orchestrator small,
  .task-text small {
    display: block;
    margin-top: 1px;
    font-size: 10px;
    opacity: 0.6;
  }

  .tasks {
    position: relative;
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 16px 0 0;
    padding: 0;
    list-style: none;
    opacity: 0;
    transform: translateY(-6px);
    transition:
      opacity 260ms ease,
      transform 260ms ease;
  }

  .task {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    transition: border-color 200ms ease;
  }

  .task.running {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .task-text {
    flex: 1;
    min-width: 0;
  }

  .task-status {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    opacity: 0.85;
  }

  .scenes {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .scene {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .scene:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .scene:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 1px;
  }

  .scene.active {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .scene-title {
    font-weight: 600;
  }

  .scene-body {
    line-height: 1.5;
    opacity: 0.8;
  }

  @media (prefers-reduced-motion: reduce) {
    .dot.running {
      animation: none;
    }

    .orchestrator,
    .tasks,
    .agent .bar i {
      transition: none;
    }
  }
</style>
