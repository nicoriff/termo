<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import SidePanelSection from "./SidePanelSection.svelte";
  import SidePanelIcon from "./SidePanelIcon.svelte";
  import { SectionHost } from "./side-panel-host.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import SidebarResizeHandle from "./SidebarResizeHandle.svelte";
  import { sidePanelStore } from "./side-panel-store.svelte";
  import { SIDE_PANEL_SECTIONS } from "./side-panel-sections";
  import { startSectionDrag } from "./side-panel-drag";
  import type { SidePanelId } from "./side-panel-layout";

  /** The accordion headers' height, and the least a group shrinks to. */
  const ROW_PX = 28;
  const GROUP_MIN_PX = 120;

  let columnEl: HTMLElement | undefined = $state();
  let resizing = $state(false);

  /** A section off in Preferences (Git, or Modo IA's) stays in the layout for
   * when it's back on, without showing. */
  function available(id: SidePanelId): boolean {
    if (id === "git") return appConfigStore.gitEnabled;
    if (id === "agents" || id === "kanban") return appConfigStore.aiModeEnabled;
    return true;
  }

  const groups = $derived(
    sidePanelStore.layout.groups
      .map((group, index) => ({ ...group, index, panels: group.panels.filter(available) }))
      .filter((group) => group.panels.length > 0),
  );
  const shown = $derived(sidePanelStore.shown && groups.length > 0);
  const drag = $derived(sidePanelStore.drag);

  /** A section unfolding takes the room the one folding gives up: both
   * animate their share of the group's height at once. */
  function grow(_node: HTMLElement) {
    return { duration: 200, easing: cubicOut, css: (k: number) => `flex-grow: ${k}; overflow: hidden;` };
  }

  function title(id: SidePanelId): string {
    return t(SIDE_PANEL_SECTIONS[id].title);
  }

  function close(id: SidePanelId) {
    sidePanelStore.close(id);
    if (!sidePanelStore.shown) uiState.refocusTerminals();
  }

  /** What each section hands its header: its buttons and a note. */
  const hosts = new Map<SidePanelId, SectionHost>();
  function hostOf(id: SidePanelId): SectionHost {
    let host = hosts.get(id);
    if (!host) hosts.set(id, (host = new SectionHost()));
    return host;
  }

  /** Dragging the border between two groups: each keeps at least a few
   * rows' worth, with its folded headers. */
  function startGroupResize(event: PointerEvent, index: number) {
    if (event.button !== 0 || !columnEl) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const height = columnEl.getBoundingClientRect().height;
    const rows = Math.max(groups[index].panels.length, groups[index + 1].panels.length);
    const min = Math.max(GROUP_MIN_PX, rows * ROW_PX + 80) / height;
    let lastY = event.clientY;
    const from = groups[index].index;
    resizing = true;
    const move = (e: PointerEvent) => {
      sidePanelStore.resize(from, (e.clientY - lastY) / height, min);
      lastY = e.clientY;
    };
    const end = () => {
      resizing = false;
      sidePanelStore.resize(from, 0, min, true);
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  }

  /** Double-click: the two groups share their height evenly. */
  function evenGroups(index: number) {
    const a = groups[index];
    const b = groups[index + 1];
    sidePanelStore.resize(a.index, (a.size + b.size) / 2 - a.size, 0, true);
  }

  function onResizeKey(event: KeyboardEvent, index: number) {
    if (!columnEl) return;
    const step = (event.shiftKey ? 40 : 10) / columnEl.getBoundingClientRect().height;
    if (event.key === "ArrowUp") sidePanelStore.resize(groups[index].index, -step, GROUP_MIN_PX / columnEl.clientHeight, true);
    else if (event.key === "ArrowDown") sidePanelStore.resize(groups[index].index, step, GROUP_MIN_PX / columnEl.clientHeight, true);
    else return;
    event.preventDefault();
  }
</script>

{#if shown}
  <aside
    class="side-panel"
    style="--side-width:{sidePanelStore.width}px"
    transition:slide={{ axis: "x", duration: 180, easing: cubicOut }}
    aria-label={t("sidePanel.label")}
  >
    <SidebarResizeHandle />
    <!-- Its own fixed width, so nothing reflows while the column slides. -->
    <div class="side-inner" bind:this={columnEl}>
      {#each groups as group, g (group.panels[0])}
        {@const target = drag?.target}
        {#if g > 0}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <div
            class="group-divider"
            class:resizing
            role="separator"
            aria-orientation="horizontal"
            aria-label={t("sidePanel.resizeGroups")}
            title={t("sidePanel.resizeGroups")}
            tabindex="0"
            onpointerdown={(e) => startGroupResize(e, g - 1)}
            ondblclick={() => evenGroups(g - 1)}
            onkeydown={(e) => onResizeKey(e, g - 1)}
          ></div>
        {/if}
        <div
          class="group"
          class:drop-into={target?.kind === "accordion" && target.group === group.index}
          style="flex-grow:{group.size}"
          data-side-group={group.index}
        >
          {#each group.panels as id (id)}
            {@const expanded = id === group.expanded}
            {@const host = hostOf(id)}
            <!-- The section's one header: its name, what's in view, its own
                 buttons (handed over by the section), and closing it. A
                 click folds or unfolds it; dragging it moves it. -->
            <div class="section-row" class:expanded data-side-section>
              <button
                class="row-toggle"
                title={title(id)}
                aria-expanded={expanded}
                onpointerdown={(e) => startSectionDrag(e, id, title(id))}
                onclick={() => sidePanelStore.toggleExpanded(group.index, id)}
              >
                <svg class="chevron" viewBox="0 0 6 10" aria-hidden="true"><path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
                <span class="row-icon"><SidePanelIcon {id} /></span>
                <span class="row-title">{title(id)}</span>
                {#if expanded && host.note}
                  <span class="row-note" title={host.note}>{host.note}</span>
                {/if}
              </button>
              {#if expanded && host.actions}
                <div class="row-actions">{@render host.actions()}</div>
              {/if}
              <button
                class="row-close"
                title={t(SIDE_PANEL_SECTIONS[id].close)}
                aria-label={t(SIDE_PANEL_SECTIONS[id].close)}
                onclick={() => close(id)}
              >
                <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
              </button>
            </div>
            {#if expanded}
              <div class="section-body" transition:grow>
                <SidePanelSection {host} load={SIDE_PANEL_SECTIONS[id].load} />
              </div>
            {/if}
          {/each}
          {#if target?.kind === "group" && target.group === group.index}
            <div class="drop-line" class:above={target.side === "above"} class:below={target.side === "below"}></div>
          {/if}
        </div>
      {/each}
    </div>
  </aside>
{:else if drag}
  <!-- Hidden or empty: a drag opens it here. -->
  <div
    class="column-drop"
    class:over={drag.target?.kind === "column"}
    style="--side-width:{sidePanelStore.width}px"
    data-side-column-drop
  ></div>
{/if}

<style>
  /* Docked at the window's left edge: the consoles make room for it rather
     than being covered. */
  .side-panel {
    position: relative;
    flex-shrink: 0;
    width: var(--side-width, 264px);
    overflow: hidden;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
  }

  .side-inner {
    width: var(--side-width, 264px);
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .group {
    position: relative;
    flex-basis: 0;
    flex-shrink: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .group.drop-into {
    box-shadow: inset 0 0 0 1px var(--pane-active-border, #3a6df0);
  }

  .section-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .section-body > :global(*) {
    flex: 1;
    min-height: 0;
  }

  .section-row {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 2px;
    height: 28px;
    background: color-mix(in srgb, var(--titlebar-fg, #d4d4d4) 7%, transparent);
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .section-row:first-child {
    border-top: none;
  }

  /* Two folded headers in a row share one border. */
  .section-row + .section-row {
    border-top: none;
  }

  .row-toggle {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 8px 0 9px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    text-align: left;
    opacity: 0.8;
    cursor: pointer;
  }

  .section-row.expanded .row-toggle {
    opacity: 1;
  }

  .section-row:hover {
    background: color-mix(in srgb, var(--titlebar-fg, #d4d4d4) 11%, transparent);
  }

  .row-toggle:hover,
  .row-toggle:focus-visible {
    opacity: 1;
    outline: none;
  }

  .row-toggle:focus-visible {
    box-shadow: inset 0 0 0 1px var(--pane-active-border, #3a6df0);
  }

  /* Pointing right folded, down unfolded. */
  .chevron {
    flex-shrink: 0;
    width: 6px;
    height: 10px;
    transition: transform 200ms cubic-bezier(0.33, 1, 0.68, 1);
  }

  .section-row.expanded .chevron {
    transform: rotate(90deg);
  }

  .row-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .row-title {
    flex-shrink: 0;
    max-width: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The repository or folder in view, beside the name. */
  .row-note {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    opacity: 0.6;
  }

  /* The section's own buttons, as it styles them. */
  .row-actions {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .row-close {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    margin-right: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    opacity: 0;
    cursor: pointer;
  }

  .row-close svg {
    width: 9px;
    height: 9px;
  }

  .section-row:hover .row-close,
  .row-close:focus-visible {
    opacity: 0.7;
  }

  .row-close:hover {
    opacity: 1;
    background: var(--tab-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .group-divider {
    flex-shrink: 0;
    height: 5px;
    margin: -2px 0;
    position: relative;
    z-index: 5;
    cursor: ns-resize;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
    background-clip: content-box;
    outline: none;
  }

  .group-divider:hover,
  .group-divider:focus-visible,
  .group-divider.resizing {
    background: var(--divider-hover-bg, #3a6df0);
    opacity: 0.6;
  }

  :global(body:has(.group-divider.resizing)) {
    cursor: ns-resize;
    user-select: none;
  }

  /* Where a dragged section opens a group of its own. */
  .drop-line {
    position: absolute;
    left: 0;
    right: 0;
    height: 25%;
    pointer-events: none;
    background: var(--pane-active-border, #3a6df0);
    opacity: 0.18;
    border: 0 solid var(--pane-active-border, #3a6df0);
  }

  .drop-line.above {
    top: 0;
    border-top-width: 2px;
  }

  .drop-line.below {
    bottom: 0;
    border-bottom-width: 2px;
  }

  /* Hidden or empty, while a section is dragged: where it opens. */
  .column-drop {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 40px;
    z-index: 30;
    width: var(--side-width, 264px);
    border: 2px dashed var(--pane-active-border, #3a6df0);
    border-radius: 6px;
    opacity: 0.4;
    pointer-events: auto;
  }

  .column-drop.over {
    opacity: 1;
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 14%, transparent);
  }

  :global(.side-panel-drag-ghost) {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 10000;
    padding: 3px 8px;
    border-radius: 4px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 11px;
    font-weight: 600;
    pointer-events: none;
    white-space: nowrap;
  }

  :global(body.termo-dragging-section),
  :global(body.termo-dragging-section *) {
    cursor: grabbing !important;
    user-select: none;
  }
</style>
