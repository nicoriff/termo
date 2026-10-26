<script lang="ts">
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { isWindows } from "$lib/core/platform";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { formatPath, type PathFormat } from "./format";
  import { optionOn, optionValue, type StatusItemRef } from "./layout";

  /** An item as it looks in the bar, with made-up readings: what an item
   * with nothing to show right now looks like while the bar is arranged. */
  let { item }: { item: StatusItemRef } = $props();

  const SAMPLE_PATH = isWindows
    ? { path: "C:\\Users\\you\\projects\\termo\\src", home: "C:\\Users\\you", root: "C:\\Users\\you\\projects\\termo" }
    : { path: "/home/you/projects/termo/src", home: "/home/you", root: "/home/you/projects/termo" };

  const clock = $derived(
    new Date(2026, 9, 24, 14, 32, 7).toLocaleTimeString(i18n.locale, {
      hour: "2-digit",
      minute: "2-digit",
      second: optionOn(item, "seconds") ? "2-digit" : undefined,
    }),
  );
</script>

{#snippet gauge(percent: number)}
  <span class="plan-gauge" style="--fill:{percent}%"><span class="plan-gauge-text">{percent}%</span></span>
{/snippet}

{#if item.id === "ai.mode"}
  <span class="status-item">{t("statusBar.aiMode", { state: t("statusBar.aiOn") })}</span>
{:else if item.id === "agent.engine"}
  <span class="status-item"><ConsoleIcon kind="claude" size={11} />Claude Code</span>
{:else if item.id === "agent.state"}
  <span class="status-item"><span class="status-dot" style="background:#3a6df0"></span>{t("aiMode.status.running")}</span>
{:else if item.id === "agent.model"}
  <span class="status-item">Opus 5.5</span>
{:else if item.id === "agent.plan"}
  <span class="status-item status-plan">
    {t("statusBar.plan")}
    <span class="plan-window">
      <span class="plan-label">{t("statusBar.fiveHour")}</span>{@render gauge(23)}
      {#if optionOn(item, "resets")}<span class="status-dim">↻ 1h 20m</span>{/if}
    </span>
    <span class="plan-window">
      <span class="plan-label">{t("statusBar.sevenDay")}</span>{@render gauge(41)}
      {#if optionOn(item, "resets")}<span class="status-dim">↻ 3d 4h</span>{/if}
    </span>
  </span>
{:else if item.id === "agent.context"}
  <span class="status-item status-context">
    {t("statusBar.context")}
    {#if optionValue(item, "format") === "tokens"}
      <span>15.5k<span class="status-dim"> / 200k</span></span>
    {:else}
      {@render gauge(8)}
    {/if}
  </span>
{:else if item.id === "agent.cost"}
  <span class="status-item">{t("statusBar.cost", { value: "$0.42" })}</span>
{:else if item.id === "agent.spendLimit"}
  <span class="status-item">{t("statusBar.spendLimit", { value: "12%" })}</span>
{:else if item.id === "agent.lines"}
  <span class="status-item"><span class="lines-added">+156</span><span class="lines-removed">−23</span></span>
{:else if item.id === "agent.duration"}
  <span class="status-item">⏱ 12m</span>
{:else if item.id === "agent.version"}
  <span class="status-item">v2.1.0</span>
{:else if item.id === "agents.running"}
  <span class="status-item"><span class="status-dot" style="background:#3a6df0"></span>{t("statusBar.agentsRunning", { n: "2" })}</span>
{:else if item.id === "companion.nudge"}
  <span class="status-item"><span class="status-dot" style="background:#f5a623"></span>{t("companion.nudge.cacheExpiring", { minutes: "3" })}</span>
{:else if item.id === "git.repo"}
  <span class="status-item sample-git">
    <svg class="branch-icon" viewBox="0 0 16 16" aria-hidden="true">
      <path d="M5 3.8v8.4M5 8.2c4.4 0 6-.6 6-3.4" fill="none" stroke="currentColor" stroke-width="1.4" />
      <circle cx="5" cy="3" r="1.6" fill="none" stroke="currentColor" stroke-width="1.4" />
      <circle cx="5" cy="13" r="1.6" fill="none" stroke="currentColor" stroke-width="1.4" />
      <circle cx="11" cy="4" r="1.6" fill="none" stroke="currentColor" stroke-width="1.4" />
    </svg>
    {#if optionOn(item, "branch")}<span class="sample-branch">main</span>{/if}
    {#if optionOn(item, "sync")}<span class="sample-sync">↑1</span>{/if}
    {#if optionOn(item, "changes")}<span class="sample-changes">+2 ~3</span>{/if}
    {#if optionOn(item, "pr")}<span class="sample-pr">#42</span>{/if}
  </span>
{:else if item.id === "git.lastCommit"}
  <span class="status-item">
    <span class="status-dim">a1b2c3d</span>{t("statusBar.sampleCommit")}<span class="status-dim">· {t("statusBar.ago.minutes", { n: "5" })}</span>
  </span>
{:else if item.id === "git.stash"}
  <span class="status-item">{t("statusBar.stash", { n: "1" })}</span>
{:else if item.id === "git.upstream"}
  <span class="status-item">→ origin/main</span>
{:else if item.id === "git.lastFetch"}
  <span class="status-item">⇣ {t("statusBar.ago.minutes", { n: "5" })}</span>
{:else if item.id === "path.cwd"}
  <span class="status-item status-path">
    <span class="path-text">{formatPath(SAMPLE_PATH.path, optionValue(item, "format") as PathFormat, SAMPLE_PATH.home, SAMPLE_PATH.root)}</span>
  </span>
{:else if item.id === "console.process"}
  <span class="status-item">▸ node</span>
{:else if item.id === "console.size"}
  <span class="status-item">120×32</span>
{:else if item.id === "console.prompt"}
  <span class="status-item">Starship</span>
{:else if item.id === "console.memory"}
  <span class="status-item">{t("statusBar.memory", { value: "84 MB" })}</span>
{:else if item.id === "console.cpu"}
  <span class="status-item">{t("statusBar.cpu", { value: "3%" })}</span>
{:else if item.id === "system.appMemory"}
  <span class="status-item">{t("statusBar.appMemory", { value: "412 MB" })}</span>
{:else if item.id === "system.cpu"}
  <span class="status-item">{t("statusBar.systemCpu", { value: "18%" })}</span>
{:else if item.id === "system.memory"}
  <span class="status-item">{t("statusBar.systemMemory", { value: "9.2 GB / 32.0 GB" })}</span>
{:else if item.id === "system.clock"}
  <span class="status-item">{clock}</span>
{:else if item.id === "system.daemon"}
  <span class="status-item" style="color:#e3b341"><span class="status-dot" style="background:#e3b341"></span>{t("statusBar.daemonDown")}</span>
{:else if item.id === "system.remote"}
  <span class="status-item"><span class="status-dot"></span>{t("remote.viewers", { count: "1" })}</span>
{/if}

<style>
  .sample-git {
    gap: 6px;
  }

  .branch-icon {
    width: 11px;
    height: 11px;
  }

  .sample-branch {
    font-weight: 600;
  }

  .sample-sync {
    color: var(--pane-active-border, #3a6df0);
    font-weight: 600;
  }

  .sample-changes {
    color: #d7a72a;
  }

  .sample-pr {
    padding: 0 5px;
    border-radius: 7px;
    background: #3fa04a;
    color: #fff;
    font-size: 10px;
    line-height: 14px;
  }
</style>
