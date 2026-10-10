<script lang="ts">
  import { onMount } from "svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import type { AgentHooksStatus } from "$lib/core/agent-hooks-client";
  import { TOOL_IDS, type ToolId } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  let {
    status,
    choice,
    busy,
    error,
    ontoggle,
  }: {
    /** What's installed right now; null until the first check answers. */
    status: AgentHooksStatus | null;
    /** What the user wants once this step is applied. */
    choice: Record<ToolId, boolean>;
    busy: boolean;
    error: string | null;
    ontoggle: (tool: ToolId) => void;
  } = $props();

  const SIGNAL_MS = 1800;
  /** Each one's own color, as in ConsoleIcon. */
  const COLORS: Record<ToolId, string> = {
    claude: "#d97757",
    codex: "#10a37f",
    gemini: "#4796e3",
    copilot: "#8957e5",
    // Black and white, like its mark: whatever the text is.
    grok: "currentColor",
  };
  /** Where each agent sits around Termo, in percent of the diagram: the five
   * points of a pentagon, its tip up. */
  const SPOTS: Record<ToolId, { x: number; y: number }> = {
    claude: { x: 14, y: 38 },
    codex: { x: 86, y: 38 },
    gemini: { x: 27, y: 86 },
    copilot: { x: 73, y: 86 },
    grok: { x: 50, y: 11 },
  };
  /** What a hook reports, in the order a turn goes through them. */
  const REPORTS = ["running", "awaiting", "finished"] as const;

  const connected = $derived(TOOL_IDS.filter((tool) => choice[tool]));

  // One connected agent at a time sends a signal down its line, and Termo
  // shows what it said.
  let beat = $state(0);
  onMount(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const timer = setInterval(() => beat++, SIGNAL_MS);
    return () => clearInterval(timer);
  });
  const speaker = $derived(connected.length > 0 ? connected[beat % connected.length] : null);
  const report = $derived(REPORTS[beat % REPORTS.length]);
  const reportColor = $derived(
    { running: themeStore.current.colors.blue, awaiting: themeStore.current.colors.yellow, finished: themeStore.current.colors.green }[
      report
    ],
  );

  function stateKey(tool: ToolId): MessageKey {
    const installed = status?.[tool] ?? false;
    if (choice[tool]) return installed ? "onboarding.agents.hooksOn" : "onboarding.agents.willInstall";
    if (installed) return "onboarding.agents.willRemove";
    return newTabOptions.agentClis?.[tool] === false ? "onboarding.agents.cliMissing" : "onboarding.agents.off";
  }
</script>

<div class="onb-stage">
  <div class="diagram" role="group" aria-label={t("onboarding.agents.diagramLabel")}>
    <svg class="wires" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
      {#each TOOL_IDS as tool (tool)}
        <line
          class="wire"
          class:on={choice[tool]}
          x1={SPOTS[tool].x}
          y1={SPOTS[tool].y}
          x2="50"
          y2="50"
          stroke={choice[tool] ? COLORS[tool] : "currentColor"}
        />
        {#if speaker === tool}
          {#key beat}
            <line
              class="signal"
              pathLength="100"
              x1={SPOTS[tool].x}
              y1={SPOTS[tool].y}
              x2="50"
              y2="50"
              stroke={COLORS[tool]}
            />
          {/key}
        {/if}
      {/each}
    </svg>

    <div class="hub">
      <img src="/app-icon.png" alt="" />
      {#if speaker}
        {#key beat}
          <span class="hub-report" style="--report:{reportColor}">
            <ConsoleIcon kind={speaker} size={11} />
            {t(`aiMode.status.${report}`)}
          </span>
        {/key}
      {/if}
    </div>

    {#each TOOL_IDS as tool (tool)}
      <button
        class="node"
        class:on={choice[tool]}
        style="left:{SPOTS[tool].x}%; top:{SPOTS[tool].y}%; --agent:{COLORS[tool]}"
        aria-pressed={choice[tool]}
        disabled={busy || status === null}
        onclick={() => ontoggle(tool)}
      >
        <span class="node-logo"><ConsoleIcon kind={tool} size={26} /></span>
        <span class="node-name">{TOOL_DISPLAY_NAMES[tool]}</span>
      </button>
    {/each}
  </div>
</div>

<div class="onb-panel">
  <h2>{t("onboarding.agents.title")}</h2>
  <p class="onb-intro">{t("onboarding.agents.intro")}</p>

  <ul class="agents">
    {#each TOOL_IDS as tool (tool)}
      <li>
        <label class="agent-row">
          <ConsoleIcon kind={tool} size={18} />
          <span class="agent-text">
            {TOOL_DISPLAY_NAMES[tool]}
            <small>{t(stateKey(tool))}</small>
          </span>
          <input
            class="onb-check"
            type="checkbox"
            checked={choice[tool]}
            disabled={busy || status === null}
            onchange={() => ontoggle(tool)}
          />
        </label>
      </li>
    {/each}
  </ul>

  {#if error}
    <p class="onb-error" role="alert">{t("onboarding.agents.failed", { error })}</p>
  {/if}
  <p class="onb-hint">{t("onboarding.agents.hint")}</p>
</div>

<style>
  .diagram {
    position: relative;
    width: min(100%, 380px);
    aspect-ratio: 5 / 4;
  }

  .wires {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .wire {
    stroke-width: 1.5;
    stroke-dasharray: 3 5;
    opacity: 0.25;
    vector-effect: non-scaling-stroke;
    transition: opacity 200ms ease;
  }

  .wire.on {
    stroke-dasharray: none;
    opacity: 0.55;
  }

  /* A short bright stretch that runs from the agent to Termo. */
  .signal {
    stroke-width: 3.5;
    stroke-linecap: round;
    stroke-dasharray: 12 100;
    stroke-dashoffset: 12;
    vector-effect: non-scaling-stroke;
    animation: travel 900ms ease-in forwards;
  }

  @keyframes travel {
    to {
      stroke-dashoffset: -100;
    }
  }

  .hub {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 72px;
    padding: 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 16px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
  }

  .hub img {
    width: 52px;
    height: 52px;
  }

  .hub-report {
    position: absolute;
    top: 100%;
    margin-top: 6px;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px;
    border: 1px solid var(--report);
    border-radius: 10px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    font-size: 10px;
    white-space: nowrap;
    /* Lands when the signal does. */
    animation: report 300ms ease-out 700ms backwards;
  }

  @keyframes report {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  .node {
    position: absolute;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 10px;
    cursor: pointer;
  }

  .node:disabled {
    cursor: default;
  }

  .node-logo {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    border: 1.5px dashed var(--tabbar-border, #2a2a2a);
    border-radius: 50%;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    filter: grayscale(1);
    opacity: 0.55;
    transition:
      border-color 200ms ease,
      opacity 200ms ease,
      filter 200ms ease;
  }

  .node.on .node-logo {
    border: 1.5px solid var(--agent);
    filter: none;
    opacity: 1;
  }

  .node:hover:not(:disabled) .node-logo {
    opacity: 1;
  }

  .node:focus-visible {
    outline: none;
  }

  .node:focus-visible .node-logo {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 2px;
  }

  .node-name {
    opacity: 0.8;
    white-space: nowrap;
  }

  .agents {
    list-style: none;
    margin: 14px 0 0;
    padding: 0;
  }

  .agents li + li {
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .agent-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 2px;
    font-size: 12px;
    cursor: pointer;
  }

  .agent-text {
    flex: 1;
    min-width: 0;
  }

  .agent-text small {
    display: block;
    margin-top: 1px;
    font-size: 10px;
    opacity: 0.6;
  }

  @media (prefers-reduced-motion: reduce) {
    .signal {
      display: none;
    }

    .hub-report {
      animation: none;
    }
  }
</style>
