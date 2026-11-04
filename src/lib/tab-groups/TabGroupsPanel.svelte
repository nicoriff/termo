<script lang="ts">
  import { sectionHeader } from "$lib/app/side-panel-host.svelte";
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { collectSessions, isConsoleTab, type SessionLocation, type SessionTab } from "$lib/layout/split-tree";
  import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
  import type { AiSessionStatus } from "$lib/ai-mode/classifier";
  import { openLink } from "$lib/core/links";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { GROUP_COLORS, type GroupAlert, type TabGroup } from "./tab-groups";
  import { tabGroupsStore } from "./tab-groups-store.svelte";
  import { consolePortsStore } from "./console-ports.svelte";

  /** Every tab group, as an accordion: each one's open tabs (a click brings
   * one to the front) or, for a group whose tabs are closed, the ones it
   * last had (a click opens the group again, showing that one). */

  const FOLDED_KEY = "termo.tabGroups.folded";
  const UNGROUPED_KEY = "termo.tabGroups.showUngrouped";

  function loadFolded(): string[] {
    try {
      const saved = JSON.parse(localStorage.getItem(FOLDED_KEY) ?? "null");
      return Array.isArray(saved) ? saved : [];
    } catch {
      return [];
    }
  }

  let folded = $state<string[]>(loadFolded());
  let showUngrouped = $state(loadShowUngrouped());

  function loadShowUngrouped(): boolean {
    try {
      return localStorage.getItem(UNGROUPED_KEY) === "1";
    } catch {
      return false;
    }
  }

  function toggleUngrouped() {
    showUngrouped = !showUngrouped;
    try {
      localStorage.setItem(UNGROUPED_KEY, showUngrouped ? "1" : "0");
    } catch {
      // Only a convenience.
    }
  }

  /** The window's tabs in no group, in the order they're in. */
  const ungrouped = $derived(
    workspaceStore.root ? collectSessions(workspaceStore.root).filter((loc) => !loc.session.groupId) : [],
  );

  // The ports the consoles serve, asked for while the panel is open.
  $effect(() => consolePortsStore.watch());

  function toggleFolded(groupId: string) {
    folded = folded.includes(groupId) ? folded.filter((id) => id !== groupId) : [...folded, groupId];
    try {
      localStorage.setItem(FOLDED_KEY, JSON.stringify(folded));
    } catch {
      // Only a convenience.
    }
  }

  function newGroup(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const id = tabGroupsStore.create();
    tabGroupsStore.openEditor(id, box.left, box.bottom);
  }

  function openActions(event: MouseEvent, group: TabGroup) {
    event.stopPropagation();
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    tabGroupsStore.openEditor(group.id, box.left, box.bottom);
  }

  /** The same colors as the tabs' own status dots (see TileChrome). */
  function alertColor(alert: GroupAlert): string {
    const colors = themeStore.current.colors;
    return alert === "error" ? colors.red : alert === "awaiting" ? colors.yellow : colors.green;
  }

  /** The same colors as the tabs' own agent dots (see TileChrome). */
  function statusColor(status: AiSessionStatus): string {
    const colors = themeStore.current.colors;
    return status === "running" ? colors.blue : status === "awaiting" ? colors.yellow : status === "error" ? colors.red : colors.green;
  }

  function portUrl(port: number): string {
    return `http://localhost:${port}`;
  }

  function countLabel(n: number): string {
    return n === 1 ? t("tabGroups.tabsOne") : t("tabGroups.tabs", { n });
  }

  function liveTitle(session: SessionTab): string {
    return tabTitleStore.titleFor(session.id, session.title);
  }
  sectionHeader({ actions: headerActions });
</script>

{#snippet liveRow(loc: SessionLocation)}
  {@const session = loc.session}
  {@const isConsole = isConsoleTab(session)}
  {@const status = isConsole ? aiStatusStore.byId[session.id]?.status : undefined}
  {@const ports = consolePortsStore.byTab[session.id] ?? []}
  <li class="tab-row" class:active={workspaceStore.focusedSessionId === session.id}>
    <button class="tab-button" title={session.filePath ?? liveTitle(session)} onclick={() => tabGroupsStore.focusTab(session.id)}>
      {#if isConsole}
        <ConsoleIcon kind={tabTitleStore.kindFor(session)} size={12} />
      {:else}
        <span class="tab-glyph" aria-hidden="true"></span>
      {/if}
      <span class="tab-title">{liveTitle(session)}</span>
    </button>
    <button
      class="icon-button tab-close"
      title={t("tabs.close")}
      aria-label={t("tabs.close")}
      onclick={() => workspaceStore.closeTab(loc.tileId, session.id)}
    >
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
    </button>
    {#if isConsole}
      <!-- What the console is (its agent, else its shell), what the agent is
           doing and what the console serves. Part of the tab: a click beside
           them goes to it too (the title button is the keyboard's way). -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="tab-meta" onclick={(event) => event.target === event.currentTarget && tabGroupsStore.focusTab(session.id)}>
        <span class="tab-type">{tabTitleStore.typeFor(session)}</span>
        {#if status}
          <span class="tab-status" class:urgent={status === "awaiting" || status === "error"} style="--status:{statusColor(status)}">
            {t(`aiMode.status.${status}`)}
          </span>
        {/if}
        {#each ports as served (served.port)}
          {#if served.http}
            <button
              class="port http"
              title={t("tabGroups.openPort", { url: portUrl(served.port) })}
              onclick={() => void openLink(portUrl(served.port)).catch(() => {})}
            >
              :{served.port}
              <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M3.5 1.5h5v5M8.5 1.5 2 8" fill="none" stroke="currentColor" stroke-width="1.1" /></svg>
            </button>
          {:else}
            <span class="port" title={t("tabGroups.portHint", { port: String(served.port) })}>:{served.port}</span>
          {/if}
        {/each}
      </div>
    {/if}
  </li>
{/snippet}

<aside
  class="groups-panel"
  aria-label={t("tabGroups.title")}
>
  <div class="groups-inner">

    <div class="groups-body">
      {#if tabGroupsStore.groups.length === 0 && !showUngrouped}
        <p class="empty-note">{t("tabGroups.empty")}</p>
      {/if}
      {#each tabGroupsStore.groups as group (group.id)}
        {@const live = tabGroupsStore.liveTabs.get(group.id) ?? []}
        {@const isOpen = live.length > 0}
        {@const expanded = !folded.includes(group.id)}
        {@const name = group.name || t("tabGroups.unnamed")}
        {@const alert = tabGroupsStore.alertFor(group.id)}
        <section class="group" style="--group-color:{GROUP_COLORS[group.color]}">
          <div class="group-head" class:closed={!isOpen}>
            <button
              class="group-toggle"
              aria-expanded={expanded}
              title={isOpen ? name : t("tabGroups.closedHint")}
              onclick={() => toggleFolded(group.id)}
            >
              <svg class="chevron" class:expanded viewBox="0 0 6 10" width="6" height="10" aria-hidden="true">
                <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
              </svg>
              <span class="group-dot" aria-hidden="true"></span>
              <span class="group-name" class:unnamed={!group.name}>{name}</span>
              {#if alert}
                <span
                  class="group-alert"
                  class:urgent={alert !== "finished"}
                  style="background:{alertColor(alert)}"
                  title={t("tabGroups.alertHint", { name, what: t(`aiMode.notify.${alert}`) })}
                ></span>
              {/if}
              <span class="group-count">
                {isOpen ? countLabel(live.length) : t("tabGroups.closed")}
              </span>
            </button>
            {#if !isOpen}
              <button
                class="icon-button"
                title={t("tabGroups.reopen")}
                aria-label={t("tabGroups.reopen")}
                onclick={() => tabGroupsStore.reopen(group.id)}
              >
                <svg viewBox="0 0 12 12" aria-hidden="true">
                  <path d="M2 6h7M6.5 3.5 9 6 6.5 8.5" fill="none" stroke="currentColor" stroke-width="1.2" />
                </svg>
              </button>
            {/if}
            <button
              class="icon-button"
              title={t("tabGroups.edit")}
              aria-label={t("tabGroups.actions", { name })}
              aria-haspopup="dialog"
              onclick={(e) => openActions(e, group)}
            >
              <svg viewBox="0 0 12 12" aria-hidden="true">
                <circle cx="2.5" cy="6" r="1" fill="currentColor" />
                <circle cx="6" cy="6" r="1" fill="currentColor" />
                <circle cx="9.5" cy="6" r="1" fill="currentColor" />
              </svg>
            </button>
          </div>
          {#if expanded}
            <ul class="group-tabs" transition:slide={{ duration: 140, easing: cubicOut }}>
              {#if isOpen}
                {#each live as loc (loc.session.id)}
                  {@render liveRow(loc)}
                {/each}
              {:else if group.tabs.length === 0}
                <li class="tab-empty">{t("tabGroups.emptyGroup")}</li>
              {:else}
                {#each group.tabs as tab, index (index)}
                  <li class="tab-row closed">
                    <button class="tab-button" title={t("tabGroups.closedHint")} onclick={() => tabGroupsStore.reopen(group.id, index)}>
                      <span class="tab-glyph" aria-hidden="true"></span>
                      <span class="tab-title">{tab.title}</span>
                      {#if tab.cwd ?? tab.filePath}
                        <span class="tab-where">{tab.cwd ?? tab.filePath}</span>
                      {/if}
                    </button>
                  </li>
                {/each}
              {/if}
            </ul>
          {/if}
        </section>
      {/each}

      {#if showUngrouped}
        <section class="group ungrouped" transition:slide={{ duration: 140, easing: cubicOut }}>
          <div class="group-head">
            <div class="group-label">
              <span class="group-name">{t("tabGroups.ungrouped")}</span>
              <span class="group-count">{countLabel(ungrouped.length)}</span>
            </div>
          </div>
          <ul class="group-tabs">
            {#each ungrouped as loc (loc.session.id)}
              {@render liveRow(loc)}
            {:else}
              <li class="tab-empty">{t("tabGroups.ungroupedEmpty")}</li>
            {/each}
          </ul>
        </section>
      {/if}
    </div>
  </div>
</aside>

<!-- Its header's buttons, shown in the side panel's header for it
     (see SidePanelColumn). -->

{#snippet headerActions()}
      <button
        class="icon-button"
        class:pressed={showUngrouped}
        title={showUngrouped ? t("tabGroups.hideUngrouped") : t("tabGroups.showUngrouped")}
        aria-label={showUngrouped ? t("tabGroups.hideUngrouped") : t("tabGroups.showUngrouped")}
        aria-pressed={showUngrouped}
        onclick={toggleUngrouped}
      >
        <!-- Loose tabs: a stack of them, not in a folder. -->
        <svg viewBox="0 0 12 12" aria-hidden="true">
          <rect x="1" y="1.5" width="7" height="4" rx="0.8" fill="none" stroke="currentColor" stroke-width="1" />
          <rect x="4" y="6.5" width="7" height="4" rx="0.8" fill="none" stroke="currentColor" stroke-width="1" stroke-dasharray="1.6 1.1" />
        </svg>
      </button>
      <button class="icon-button" title={t("tabGroups.newGroup")} aria-label={t("tabGroups.newGroup")} onclick={newGroup}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 0v10M0 5h10" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
{/snippet}

<style>
  .groups-panel {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .groups-inner {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .icon-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    opacity: 0.7;
    cursor: pointer;
  }

  .icon-button svg {
    width: 12px;
    height: 12px;
  }

  .icon-button:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .icon-button.pressed {
    opacity: 1;
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.25));
  }

  .groups-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
    padding: 4px 0;
  }

  .empty-note {
    margin: 0;
    padding: 12px;
    opacity: 0.6;
    line-height: 1.45;
  }

  .group {
    margin: 2px 6px;
    border-left: 3px solid var(--group-color);
    border-radius: 4px;
    background: color-mix(in srgb, var(--group-color) 6%, transparent);
  }

  .group-head {
    display: flex;
    align-items: center;
    padding-right: 2px;
  }

  .group-toggle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 6px 6px 8px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .chevron {
    flex-shrink: 0;
    opacity: 0.6;
    transition: transform 120ms ease;
  }

  .chevron.expanded {
    transform: rotate(90deg);
  }

  .group-dot {
    width: 9px;
    height: 9px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--group-color);
  }

  .group-name {
    min-width: 0;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group-name.unnamed {
    font-style: italic;
    font-weight: 400;
    opacity: 0.7;
  }

  /* One of the group's tabs has news; waiting on you or an error pulses. */
  .group-alert {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    margin-left: auto;
    border-radius: 50%;
  }

  .group-alert.urgent {
    animation: group-alert-pulse 1.4s ease-in-out infinite;
  }

  @keyframes group-alert-pulse {
    50% {
      transform: scale(1.35);
    }
  }

  .group-alert + .group-count {
    margin-left: 0;
  }

  .group-count {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 10px;
    opacity: 0.55;
  }

  /* The tabs in no group: no color of their own. */
  .group.ungrouped {
    --group-color: color-mix(in srgb, currentColor 30%, transparent);
    margin-top: 8px;
  }

  .group-label {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 6px 6px 21px;
  }

  .group-head.closed .group-name {
    opacity: 0.7;
  }

  .group-tabs {
    margin: 0;
    padding: 0 0 4px;
    list-style: none;
  }

  /* The tab, its close button and its ports are one block: hovered and
     selected together. */
  .tab-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    margin: 1px 4px;
    padding-right: 2px;
    border-radius: 5px;
  }

  .tab-row:hover,
  .tab-row:focus-within {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .tab-row.active {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.35));
  }

  .tab-button {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 6px 5px 24px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .tab-button:focus-visible {
    outline: none;
  }

  .tab-row.closed .tab-button {
    opacity: 0.6;
  }

  .tab-glyph {
    width: 10px;
    height: 10px;
    flex-shrink: 0;
    border: 1px solid currentColor;
    border-radius: 2px;
    box-sizing: border-box;
    opacity: 0.6;
  }

  .tab-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* What the agent in the console is doing, in the tab dots' colors. */
  /* Under the title: what the console is, what its agent is doing and the
     ports it serves. */
  .tab-meta {
    flex-basis: 100%;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin-top: -3px;
    padding: 0 6px 6px 43px;
    font-size: 10px;
    cursor: pointer;
  }

  .tab-type {
    opacity: 0.6;
    white-space: nowrap;
  }

  .tab-status {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    opacity: 0.85;
  }

  .tab-status::before {
    content: "";
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--status);
  }

  .tab-status.urgent::before {
    animation: group-alert-pulse 1.4s ease-in-out infinite;
  }

  .port:not(.http) {
    cursor: default;
  }

  .port {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 6px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 10px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    opacity: 0.75;
  }

  .port.http {
    border-color: color-mix(in srgb, var(--pane-active-border, #3a6df0) 60%, transparent);
    color: var(--pane-active-border, #6a9bff);
    opacity: 1;
    cursor: pointer;
  }

  .port.http:hover {
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 18%, transparent);
  }

  .port svg {
    width: 8px;
    height: 8px;
  }

  .tab-where {
    margin-left: auto;
    min-width: 0;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    font-size: 10px;
    opacity: 0.6;
  }

  .tab-close {
    opacity: 0;
  }

  .tab-row:hover .tab-close,
  .tab-close:focus-visible {
    opacity: 0.7;
  }

  .tab-empty {
    padding: 4px 8px 6px 28px;
    font-size: 11px;
    opacity: 0.5;
  }
</style>
