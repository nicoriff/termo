<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { formatKeybindingLabel, type ShortcutAction } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import { orchestrationStore } from "$lib/agents/orchestration-store.svelte";
  import { explorerStore } from "$lib/explorer/explorer-store.svelte";
  import { gitStore } from "$lib/git/git-store.svelte";
  import { harnessStore } from "$lib/harness/harness-store.svelte";

  /** What's working right now, shown on each button so it's noticed with the
   * panel closed: the agents' runs, and the orchestrations running. */
  const running = $derived(Object.values(agentsStore.lastRun).filter((run) => run.status === "running").length);
  const orchestrating = $derived(orchestrationStore.all.filter((item) => item.status === "running").length);
  /** Questions from agents waiting on the user: they win the badge. */
  const asked = $derived(orchestrationStore.waitingQuestions);
  const keys = (action: ShortcutAction) => formatKeybindingLabel(appConfigStore.getEffectiveShortcut(action), isMac);
  const localHint = $derived(`${t("agents.sections.local")} (${keys("toggleAgents")})`);
  const orchestrationHint = $derived(`${t("agents.sections.orchestration")} (${keys("toggleOrchestration")})`);
  const explorerHint = $derived(`${t("explorer.toggle")} (${keys("toggleExplorer")})`);
  const gitHint = $derived(`${t("git.title")} (${keys("toggleGit")})`);
  const harnessHint = $derived(`${t("harness.title")} (${keys("toggleHarness")})`);
  /** Files with anything to commit, in the repository of the tab in focus. */
  const changed = $derived(gitStore.counts.total);
</script>

<!-- The window's vertical tab strip: one button per side panel, each opening
     and closing its own. The agents panel has one per section. -->
<nav class="activity-bar" aria-label={t("activityBar.label")}>
  <button
    class="activity-item"
    class:active={explorerStore.open}
    title={explorerHint}
    aria-label={explorerHint}
    aria-pressed={explorerStore.open}
    onclick={() => explorerStore.toggle()}
  >
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M3.5 18.5v-13h6l2 2.5h9v10.5z"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linejoin="round"
      />
      <path d="M7.5 12h9M7.5 15h6" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
    </svg>
  </button>
  {#if appConfigStore.gitEnabled && appConfigStore.gitInActivityBar}
    <button
      class="activity-item"
      class:active={gitStore.open}
      title={gitHint}
      aria-label={gitHint}
      aria-pressed={gitStore.open}
      onclick={() => gitStore.toggle()}
    >
      <!-- A branch leaving the trunk. -->
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M7.5 7.2v9.6M7.5 12.6c6.2 0 9-1.2 9-5.2" fill="none" stroke="currentColor" stroke-width="1.6" />
        <circle cx="7.5" cy="5.2" r="2.1" fill="none" stroke="currentColor" stroke-width="1.6" />
        <circle cx="7.5" cy="18.8" r="2.1" fill="none" stroke="currentColor" stroke-width="1.6" />
        <circle cx="16.5" cy="5.6" r="2.1" fill="none" stroke="currentColor" stroke-width="1.6" />
      </svg>
      {#if gitStore.running}
        <span class="activity-badge">…</span>
      {:else if changed > 0}
        <span class="activity-badge" title={t("git.changes.badge", { n: String(changed) })}>{changed > 99 ? "99+" : changed}</span>
      {/if}
    </button>
  {/if}
  <button
    class="activity-item"
    class:active={harnessStore.open}
    title={harnessHint}
    aria-label={harnessHint}
    aria-pressed={harnessStore.open}
    onclick={() => harnessStore.toggle()}
  >
    <!-- A shield with a tick: the rules the agents work under. -->
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path
        d="M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6z"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linejoin="round"
      />
      <path d="M8.6 12.2l2.4 2.4 4.4-4.6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    {#if harnessStore.status === "pending" || harnessStore.status === "draft"}
      <span class="activity-badge pending" title={t("harness.badge")}>!</span>
    {/if}
  </button>
  {#if appConfigStore.aiModeEnabled && appConfigStore.agentsInActivityBar}
    <button
      class="activity-item"
      class:active={agentsStore.open && agentsStore.section === "local"}
      title={localHint}
      aria-label={localHint}
      aria-pressed={agentsStore.open && agentsStore.section === "local"}
      onclick={() => agentsStore.toggleSection("local")}
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="4" y="7.5" width="16" height="12" rx="3.2" fill="none" stroke="currentColor" stroke-width="1.6" />
        <path d="M12 7.5V4.2" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        <circle cx="12" cy="3.6" r="1.2" fill="currentColor" />
        <circle cx="9" cy="13" r="1.4" fill="currentColor" />
        <circle cx="15" cy="13" r="1.4" fill="currentColor" />
        <path d="M9.5 16.6h5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
      </svg>
      {#if running > 0}
        <span class="activity-badge">{running}</span>
      {/if}
    </button>
    <button
      class="activity-item"
      class:active={agentsStore.open && agentsStore.section === "orchestration"}
      title={orchestrationHint}
      aria-label={orchestrationHint}
      aria-pressed={agentsStore.open && agentsStore.section === "orchestration"}
      onclick={() => agentsStore.toggleSection("orchestration")}
    >
      <!-- An org chart: the orchestrator over its team. -->
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="8.5" y="2.8" width="7" height="5.4" rx="1.4" fill="none" stroke="currentColor" stroke-width="1.6" />
        <rect x="2.6" y="15.8" width="6.2" height="5.4" rx="1.4" fill="none" stroke="currentColor" stroke-width="1.6" />
        <rect x="15.2" y="15.8" width="6.2" height="5.4" rx="1.4" fill="none" stroke="currentColor" stroke-width="1.6" />
        <path d="M12 8.2v3.8M5.7 15.8v-3.8h12.6v3.8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
      </svg>
      {#if asked > 0}
        <span class="activity-badge asking" title={t("agents.orchestration.questions.badge", { n: String(asked) })}>?</span>
      {:else if orchestrating > 0}
        <span class="activity-badge">{orchestrating}</span>
      {/if}
    </button>
  {/if}
</nav>

<style>
  .activity-bar {
    flex-shrink: 0;
    width: 40px;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    padding-top: 4px;
    background: var(--tabbar-bg, #1e1e1e);
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .activity-item {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    height: 40px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    opacity: 0.55;
    cursor: pointer;
  }

  .activity-item svg {
    width: 21px;
    height: 21px;
  }

  .activity-item:hover {
    opacity: 0.9;
  }

  /* The open panel's button: full strength, with the accent on its edge. */
  .activity-item.active {
    opacity: 1;
  }

  .activity-item.active::before {
    content: "";
    position: absolute;
    top: 6px;
    bottom: 6px;
    left: 0;
    width: 2px;
    border-radius: 0 2px 2px 0;
    background: var(--pane-active-border, #3a6df0);
  }

  .activity-item:focus-visible {
    outline: 1px solid var(--pane-active-border, #3a6df0);
    outline-offset: -3px;
  }

  .activity-badge.pending {
    background: #d7a72a;
    color: #1e1e1e;
  }

  .activity-badge.asking {
    background: #d7a72a;
    color: #1e1e1e;
  }

  .activity-badge {
    position: absolute;
    right: 5px;
    bottom: 6px;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    box-sizing: border-box;
    border-radius: 7px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 9px;
    font-weight: 700;
    line-height: 14px;
    text-align: center;
  }
</style>
