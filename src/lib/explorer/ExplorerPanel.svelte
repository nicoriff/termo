<script lang="ts">
  import { untrack } from "svelte";
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { explorerRoot, gitStatus, listEntries, type GitMark } from "$lib/core/explorer-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { fileTabs } from "$lib/editor/file-tabs.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { sessionFolder, workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { collectTiles, fileName, isFileTab } from "$lib/layout/split-tree";
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import SidebarResizeHandle from "$lib/app/SidebarResizeHandle.svelte";
  import { explorerStore } from "./explorer-store.svelte";
  import { folderHasChanges, visibleRows, type TreeEntry } from "./explorer-tree";
  import { fileBadge } from "./file-icons";
  import { MARK_LETTER, markColor } from "./git-marks";

  /** How often git is asked again while the panel is open: edits made from
   * the console (or an agent) show up without a manual refresh. */
  const GIT_POLL_MS = 4000;

  /** Horizontal step per nesting level, and where the first level starts. */
  const INDENT = 16;
  const INSET = 8;

  const focusedTab = $derived(workspaceStore.focusedSession);

  /** The folder that tab is in; the tree shows the repository around it. */
  const folder = $derived(focusedTab ? (sessionFolder(focusedTab) ?? null) : null);
  const onFile = $derived(focusedTab !== null && isFileTab(focusedTab));

  let root = $state<string | null>(null);
  let isRepo = $state(false);
  const children = new SvelteMap<string, TreeEntry[]>();
  const expanded = new SvelteSet<string>();
  let marks = $state<Record<string, GitMark>>({});
  let selected = $state(0);
  let panelEl: HTMLDivElement | undefined = $state();
  let listEl: HTMLDivElement | undefined = $state();

  const rows = $derived(root ? visibleRows(root, children, expanded) : []);
  const changedPaths = $derived(Object.keys(marks));

  // A new folder can mean a new repository: start that tree over. A tab that
  // is in no folder (an agent's runs, a project) leaves the tree as it was,
  // and so does a file from the tree itself: outside a repository its folder
  // would otherwise become the root.
  $effect(() => {
    const at = folder;
    if (!at) return;
    const showing = untrack(() => root);
    if (showing && onFile && (at === showing || folderHasChanges(showing, [at]))) return;
    let cancelled = false;
    void explorerRoot(at).then(async (found) => {
      if (cancelled) return;
      if (found.path !== root) {
        children.clear();
        expanded.clear();
        selected = 0;
        root = found.path;
        isRepo = found.isRepo;
        await load(found.path);
        await refreshMarks();
      }
    });
    return () => {
      cancelled = true;
    };
  });

  // Marks follow saves right away, and anything else on a short poll.
  $effect(() => {
    void fileTabs.savedTick;
    void refreshMarks();
    const timer = setInterval(() => void refreshMarks(), GIT_POLL_MS);
    return () => clearInterval(timer);
  });

  $effect(() => {
    panelEl?.focus();
  });

  async function load(dir: string) {
    try {
      children.set(dir, await listEntries(dir));
    } catch {
      children.set(dir, []);
    }
  }

  async function refreshMarks() {
    if (!root || !isRepo) {
      marks = {};
      return;
    }
    marks = await gitStatus(root);
  }

  /** Lists again every folder that's showing, keeping what's open. */
  async function refresh() {
    if (!root) return;
    await Promise.all([root, ...expanded].map((dir) => load(dir)));
    await refreshMarks();
  }

  async function toggleFolder(path: string) {
    if (expanded.has(path)) {
      expanded.delete(path);
      return;
    }
    if (!children.has(path)) await load(path);
    expanded.add(path);
  }

  function activate(index: number) {
    const row = rows[index];
    if (!row) return;
    selected = index;
    if (row.isDir) void toggleFolder(row.path);
    else {
      const tileId = workspaceStore.focusedTileId ?? (workspaceStore.root ? collectTiles(workspaceStore.root)[0]?.id : undefined);
      if (tileId) workspaceStore.openFile(tileId, row.path);
    }
  }

  function close() {
    explorerStore.setOpen(false);
    uiState.refocusTerminals();
  }

  function markFor(path: string, isDir: boolean): GitMark | "folder" | null {
    if (!isDir) return marks[path] ?? null;
    return folderHasChanges(path, changedPaths) ? "folder" : null;
  }

  function onKeydown(event: KeyboardEvent) {
    // The panel has the keyboard while it has focus; none of these should
    // reach the console underneath.
    event.stopPropagation();
    const row = rows[selected];
    if (event.key === "ArrowDown") selected = Math.min(rows.length - 1, selected + 1);
    else if (event.key === "ArrowUp") selected = Math.max(0, selected - 1);
    else if (event.key === "Enter") activate(selected);
    else if (event.key === "ArrowRight" && row?.isDir) {
      if (!expanded.has(row.path)) void toggleFolder(row.path);
      else if (rows[selected + 1]?.depth > row.depth) selected++;
    } else if (event.key === "ArrowLeft" && row) {
      if (row.isDir && expanded.has(row.path)) expanded.delete(row.path);
      else {
        // Up to the folder this row is in.
        for (let i = selected - 1; i >= 0; i--) {
          if (rows[i].depth < row.depth) {
            selected = i;
            break;
          }
        }
      }
    } else if (event.key === "Escape") uiState.refocusTerminals();
    else return;
    event.preventDefault();
  }

  /** Keeps the selected row on screen as the keyboard moves it. */
  function keepVisible(node: HTMLElement, on: boolean) {
    const reveal = (show: boolean) => show && node.scrollIntoView({ block: "nearest" });
    reveal(on);
    return { update: reveal };
  }
</script>

<aside
  class="explorer"
  style="--explorer-width:{agentsStore.width}px"
  transition:slide={{ axis: "x", duration: 180, easing: cubicOut }}
  aria-label={t("explorer.toggle")}
>
  <SidebarResizeHandle />
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="explorer-inner" role="tree" tabindex="0" aria-label={t("explorer.toggle")} bind:this={panelEl} onkeydown={onKeydown}>
  <header class="explorer-header">
    <svg class="header-icon" viewBox="0 0 16 16" aria-hidden="true">
      <path d="M1.5 4.5v8h13v-7h-6l-1.5-1.5h-5.5z" fill="none" stroke="currentColor" stroke-width="1.1" />
    </svg>
    <div class="header-text">
      <span class="header-name" title={root ?? ""}>{root ? fileName(root) : t("explorer.toggle")}</span>
      {#if root && !isRepo}
        <span class="header-note">{t("explorer.notRepo")}</span>
      {/if}
    </div>
    <button class="header-button" title={t("explorer.refresh")} aria-label={t("explorer.refresh")} onclick={() => void refresh()}>
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M13 8a5 5 0 1 1-1.5-3.55M13 2.5v2.5h-2.5" fill="none" stroke="currentColor" stroke-width="1.2" />
      </svg>
    </button>
    <button class="header-button" title={t("explorer.close")} aria-label={t("explorer.close")} onclick={close}>
      <svg viewBox="0 0 10 10" aria-hidden="true">
        <path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" />
      </svg>
    </button>
  </header>

  <div class="explorer-list" bind:this={listEl}>
    {#if !root && !folder}
      <p class="explorer-note">{t("explorer.noFolder")}</p>
    {:else if !root}
      <p class="explorer-note">{t("explorer.loading")}</p>
    {:else if rows.length === 0}
      <p class="explorer-note">{t("explorer.empty")}</p>
    {:else}
      {#each rows as row, index (row.path)}
        {@const mark = markFor(row.path, row.isDir)}
        <button
          class="explorer-row"
          class:selected={index === selected}
          role="treeitem"
          aria-selected={index === selected}
          aria-expanded={row.isDir ? expanded.has(row.path) : undefined}
          tabindex="-1"
          style="padding-left:{INSET + row.depth * INDENT}px; {mark ? `color:${markColor(mark, themeStore.current.colors)}` : ''}"
          title={mark && mark !== "folder" ? `${row.path} — ${t(`explorer.marks.${mark}`)}` : row.path}
          use:keepVisible={index === selected}
          onclick={() => activate(index)}
        >
          <!-- One guide per ancestor, lined up under that ancestor's chevron,
               so rows at the same level read as one column. -->
          {#each { length: row.depth } as _, level (level)}
            <span class="guide" style="left:{INSET + level * INDENT + 3}px"></span>
          {/each}
          {#if row.isDir}
            {@const open = expanded.has(row.path)}
            <svg class="chevron" class:open viewBox="0 0 6 10" aria-hidden="true">
              <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
            </svg>
            <svg class="kind-icon" viewBox="0 0 16 16" aria-hidden="true" style="color:{themeStore.current.colors.blue}">
              {#if open}
                <path d="M1.5 13V3.5h4.5l1.5 1.5h5V7" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
                <path d="M1.5 13l2-6h11l-2 6z" fill="currentColor" fill-opacity="0.35" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
              {:else}
                <path d="M1.5 13V3.5h4.5l1.5 1.5h7V13z" fill="currentColor" fill-opacity="0.35" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
              {/if}
            </svg>
          {:else}
            {@const badge = fileBadge(row.name)}
            <span class="chevron-spacer"></span>
            {#if badge}
              <span class="kind-badge" style="color:{themeStore.current.colors[badge.color]}">{badge.label}</span>
            {:else}
              <svg class="kind-icon file" viewBox="0 0 16 16" aria-hidden="true">
                <path d="M3.5 1.5h6l3 3v10h-9z M9.5 1.5v3h3" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
              </svg>
            {/if}
          {/if}
          <span class="row-name">{row.name}</span>
          {#if mark === "folder"}
            <span class="row-dot" title={t("explorer.marks.folder")}></span>
          {:else if mark}
            <span class="row-mark">{MARK_LETTER[mark]}</span>
          {/if}
        </button>
      {/each}
    {/if}
  </div>
  </div>
</aside>

<style>
  /* Docked at the window's left edge, like the agents panel: the consoles
     make room for it. The width animates; the content inside keeps its own
     fixed width so nothing reflows while it slides. */
  .explorer {
    position: relative;
    flex-shrink: 0;
    width: var(--explorer-width, 264px);
    overflow: hidden;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .explorer-inner {
    width: var(--explorer-width, 264px);
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .explorer-inner:focus {
    outline: none;
  }

  .explorer-header {
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

  .header-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .header-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-note {
    font-size: 10px;
    opacity: 0.55;
  }

  .header-button {
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

  .header-button svg {
    width: 12px;
    height: 12px;
  }

  .header-button:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .explorer-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px 0;
    scrollbar-width: thin;
  }

  .explorer-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    flex-shrink: 0;
    padding-top: 3px;
    padding-bottom: 3px;
    padding-right: 10px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .explorer-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .explorer-row.selected {
    background: rgba(58, 109, 240, 0.18);
  }

  .explorer-inner:focus-within .explorer-row.selected {
    box-shadow: inset 2px 0 0 var(--pane-active-border, #3a6df0);
  }

  .chevron {
    width: 6px;
    height: 10px;
    flex-shrink: 0;
    opacity: 0.6;
    transition: transform 120ms ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .chevron-spacer {
    width: 6px;
    flex-shrink: 0;
  }

  .guide {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    /* Not currentColor: a changed row is tinted, its guides shouldn't be. */
    background: var(--titlebar-fg, #d4d4d4);
    opacity: 0.14;
    pointer-events: none;
  }

  /* Folders and plain files share the same 15px slot as the badges, so the
     names of one level all start at the same x. */
  .kind-icon {
    width: 15px;
    height: 15px;
    flex-shrink: 0;
  }

  .kind-icon.file {
    opacity: 0.55;
  }

  .kind-badge {
    width: 15px;
    flex-shrink: 0;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: -0.3px;
    text-align: center;
    line-height: 15px;
  }

  .row-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Git's letter for the file, in the file's own mark color. */
  .row-mark {
    flex-shrink: 0;
    font-size: 10px;
    font-weight: 700;
  }

  .row-dot {
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    border-radius: 50%;
    background: currentColor;
  }

  .explorer-note {
    margin: 0;
    padding: 12px;
    opacity: 0.6;
  }
</style>
