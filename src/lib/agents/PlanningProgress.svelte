<script lang="ts">
  import { onDestroy } from "svelte";
  import { fly } from "svelte/transition";
  import type { PlanStep } from "$lib/core/orchestration-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { formatDuration } from "./agent-spec";
  import { latestThought, stepCounts, stepLine, toolSteps } from "./planning-progress";

  /** The orchestrator at work: what it's doing, its latest thought, the
   * trail of what it looked at, and how long it's been. */
  let {
    phase,
    steps,
    startedAt,
  }: { phase: "clarify" | "plan" | "revise"; steps: PlanStep[]; startedAt: number } = $props();

  let now = $state(Date.now());
  const timer = setInterval(() => (now = Date.now()), 1000);
  onDestroy(() => clearInterval(timer));

  const thought = $derived(latestThought(steps));
  const trail = $derived(toolSteps(steps, 6));
  const counts = $derived(stepCounts(steps));
  const elapsed = $derived(formatDuration(Math.max(0, now - startedAt)));

  const TITLES = {
    clarify: "agents.orchestration.thinking.clarify",
    plan: "agents.orchestration.thinking.plan",
    revise: "agents.orchestration.thinking.revise",
  } as const;

  /** Where it is in getting to a plan. */
  const stage = $derived(phase === "clarify" ? 0 : 1);
</script>

<div class="thinking" aria-live="polite">
  {#if phase !== "revise"}
    <ol class="stages">
      <li class:done={stage > 0} class:current={stage === 0}>{t("agents.orchestration.thinking.stageRead")}</li>
      <li class:current={stage === 1}>{t("agents.orchestration.thinking.stagePlan")}</li>
      <li>{t("agents.orchestration.thinking.stageReview")}</li>
    </ol>
  {/if}

  <div class="head">
    <span class="spinner" aria-hidden="true"></span>
    <div class="titles">
      <span class="title">{t(TITLES[phase])}</span>
      <span class="meta">
        {elapsed}
        {#if counts.files + counts.searches > 0}
          · {t("agents.orchestration.thinking.counts", { files: String(counts.files), searches: String(counts.searches) })}
        {/if}
      </span>
    </div>
  </div>

  {#key thought}
    <p class="thought" class:empty={!thought} in:fly={{ y: 6, duration: 180 }}>
      {thought || t("agents.orchestration.thinking.starting")}
    </p>
  {/key}

  {#if trail.length > 0}
    <ul class="trail">
      {#each trail as step, index (index + (step.kind === "tool" ? step.name + step.detail : ""))}
        {@const line = stepLine(step, t)}
        <li class:latest={index === trail.length - 1} style="opacity:{0.35 + (0.65 * (index + 1)) / trail.length}">
          <span class="icon">{line.icon}</span>
          <span class="text">{line.text}</span>
        </li>
      {/each}
    </ul>
  {/if}

  <p class="note">{t("agents.orchestration.thinking.note")}</p>
</div>

<style>
  .thinking {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 4px 0;
  }

  .stages {
    display: flex;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 11px;
  }

  .stages li {
    flex: 1;
    padding: 4px 8px;
    border-radius: 999px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    text-align: center;
    opacity: 0.5;
  }

  .stages li.current {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.18));
    opacity: 1;
  }

  .stages li.done {
    opacity: 0.8;
  }

  .stages li.done::before {
    content: "✓ ";
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .spinner {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    border: 2.5px solid color-mix(in srgb, var(--pane-active-border, #3a6df0) 25%, transparent);
    border-top-color: var(--pane-active-border, #3a6df0);
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }

  .titles {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .title {
    font-size: 13.5px;
    font-weight: 600;
  }

  .meta {
    font-size: 11px;
    opacity: 0.6;
    font-variant-numeric: tabular-nums;
  }

  .thought {
    margin: 0;
    min-height: 2.9em;
    padding: 10px 12px;
    border-left: 3px solid var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
    font-style: italic;
    line-height: 1.45;
  }

  .thought.empty {
    opacity: 0.6;
  }

  .trail {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
  }

  .trail li {
    display: flex;
    gap: 8px;
    min-width: 0;
  }

  .trail li.latest {
    font-weight: 600;
  }

  .trail .icon {
    flex-shrink: 0;
    width: 18px;
    text-align: center;
  }

  .trail .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .note {
    margin: 0;
    font-size: 11px;
    opacity: 0.55;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
