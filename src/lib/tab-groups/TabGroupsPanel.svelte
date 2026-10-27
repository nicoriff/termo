<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import SidebarResizeHandle from "$lib/app/SidebarResizeHandle.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { isMac } from "$lib/core/platform";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { isConsoleTab, type SessionTab } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
  import { GROUP_COLORS, type TabGroup } from "./tab-groups";
  import { TAB_GROUPS_WIDTH, tabGroupsStore } from "./tab-groups-store.svelte";

  /** Every tab group, as an accordion: each one's open tabs (a click brings
   * one to the front) or, for a group whose tabs are closed, the ones it
   * last had (a click opens the group again, showing that one). */

  const FOLDED_KEY = "termo.tabGroups.folded";

  function loadFolded(): string[] {
    try {
      const saved = JSON.parse(localStorage.getItem(FOLDED_KEY) ?? "null");
      return Array.isArray(saved) ? saved : [];
    } catch {
      return [];
    }
  }

  let folded = $state<string[]>(loadFolded());
  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleTabGroups"), isMac));

  function toggleFolded(groupId: string) {
    folded = folded.includes(groupId) ? folded.filter((id) => id !== groupId) : [...folded, groupId];
    try {
      localStorage.setItem(FOLDED_KEY, JSON.stringify(folded));
    } catch {
      // Only a convenience.
    }
  }

  function close() {
    tabGroupsStore.setOpen(false);
    uiState.refocusTerminals();
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

  function countLabel(n: number): string {
    return n === 1 ? t("tabGroups.tabsOne") : t("tabGroups.tabs", { n });
  }

  function liveTitle(session: SessionTab): string {
    return tabTitleStore.titleFor(session.id, session.title);
  }
</script>

<aside
  class="groups-panel"
  style="--panel-width:{tabGroupsStore.width}px"
  transition:slide={{ axis: "x", duration: 180, easing: cubicOut }}
  aria-label={t("tabGroups.title")}
>
  <SidebarResizeHandle sizer={tabGroupsStore} limits={TAB_GROUPS_WIDTH} />
  <div class="groups-inner">
    <header class="groups-header">
      <svg class="header-icon" viewBox="0 0 16 16" aria-hidden="true">
        <rect x="1.5" y="4" width="13" height="9.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.1" />
        <path d="M1.5 6.5h13M4.5 4V2.5h4V4" fill="none" stroke="currentColor" stroke-width="1.1" />
      </svg>
      <span class="header-name">{t("tabGroups.title")}</span>
      <button class="icon-button" title={t("tabGroups.newGroup")} aria-label={t("tabGroups.newGroup")} onclick={newGroup}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M5 0v10M0 5h10" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
      <button class="icon-button" title={`${t("tabGroups.title")} (${shortcut})`} aria-label={t("tabGroups.closePanel")} onclick={close}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    <div class="groups-body">
      {#if tabGroupsStore.groups.length === 0}
        <p class="empty-note">{t("tabGroups.empty")}</p>
      {/if}
      {#each tabGroupsStore.groups as group (group.id)}
        {@const live = tabGroupsStore.liveTabs.get(group.id) ?? []}
        {@const isOpen = live.length > 0}
        {@const expanded = !folded.includes(group.id)}
        {@const name = group.name || t("tabGroups.unnamed")}
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
                  {@const session = loc.session}
                  <li class="tab-row" class:active={workspaceStore.focusedSessionId === session.id}>
                    <button class="tab-button" title={session.filePath ?? liveTitle(session)} onclick={() => tabGroupsStore.focusTab(session.id)}>
                      {#if isConsoleTab(session)}
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
                  </li>
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
    </div>
  </div>
</aside>

<style>
  .groups-panel {
    position: relative;
    flex-shrink: 0;
    width: var(--panel-width, 280px);
    overflow: hidden;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .groups-inner {
    width: var(--panel-width, 280px);
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .groups-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 6px 0 10px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .header-icon {
    width: 15px;
    height: 15px;
    flex-shrink: 0;
    opacity: 0.8;
  }

  .header-name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .group-count {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 10px;
    opacity: 0.55;
  }

  .group-head.closed .group-name {
    opacity: 0.7;
  }

  .group-tabs {
    margin: 0;
    padding: 0 0 4px;
    list-style: none;
  }

  .tab-row {
    display: flex;
    align-items: center;
    padding-right: 2px;
  }

  .tab-button {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 6px 5px 28px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .tab-button:hover,
  .tab-button:focus-visible {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
    outline: none;
  }

  .tab-row.active .tab-button {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.35));
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
