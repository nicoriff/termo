<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { agentsStore } from "$lib/agents/agents-store.svelte";

  /** Agents working right now, shown on the button so they're noticed with
   * the panel closed. */
  const running = $derived(Object.values(agentsStore.lastRun).filter((run) => run.status === "running").length);
  const agentsHint = $derived(
    `${t("agents.title")} (${formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleAgents"), isMac)})`,
  );
</script>

<!-- The window's vertical tab strip: one button per side panel, each opening
     and closing its own. Only Agents lives here for now. -->
<nav class="activity-bar" aria-label={t("activityBar.label")}>
  {#if appConfigStore.agentsInActivityBar}
    <button
      class="activity-item"
      class:active={agentsStore.open}
      title={agentsHint}
      aria-label={agentsHint}
      aria-pressed={agentsStore.open}
      onclick={() => agentsStore.toggle()}
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
