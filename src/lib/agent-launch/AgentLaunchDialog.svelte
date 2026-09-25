<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { createFolder, listFolderRoots, listFolders } from "$lib/core/folder-client";
  import { isMac } from "$lib/core/platform";
  import { agentSessionsStore } from "$lib/ai-mode/agent-sessions-store.svelte";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import ConsoleIcon from "./ConsoleIcon.svelte";
  import { breadcrumbs, samePath, typeAheadIndex } from "./folder-nav";
  import { launchAgent } from "./launch-agent";

  /** How the step change slides in. */
  const SLIDE_PX = 70;
  const SLIDE_MS = 170;
  /** Width of the strips showing the folder behind (left) and the selected
   * folder's contents (right) on either side of the current one. */
  const LEFT_PEEK = 120;
  const RIGHT_PEEK = 120;
  const COLUMN_GAP = 8;
  /** The preview waits this long after the selection stops moving, so holding
   * an arrow key doesn't read every folder it passes. */
  const PREVIEW_DELAY_MS = 90;
  /** Letters typed within this long of each other build one search. */
  const TYPE_AHEAD_RESET_MS = 700;
  const RECENT_LIMIT = 5;

  type Tag = "home" | "drive" | "recent";
  interface Item {
    name: string;
    path: string;
    tag?: Tag;
  }
  /** One folder's subfolders — or, at the start of the chain (path null), the
   * places to start from. */
  interface Column {
    path: string | null;
    items: Item[];
    selected: number;
    status: "loading" | "ready" | "error";
    error?: string;
  }

  const launch = $derived(uiState.agentLaunch!);
  const toolName = $derived(TOOL_DISPLAY_NAMES[launch.tool]);

  let step = $state<"choose" | "browse">("choose");
  let choice = $state(0);
  /**
   * The carousel: every folder walked through, from the starting places down
   * to the current one at `depth`, plus the selected folder's contents right
   * after it as a preview. Going deeper turns that preview into the current
   * column; going back leaves the one just left in place as the preview — so
   * both directions slide an already-loaded column into the middle.
   */
  let columns = $state<Column[]>([]);
  let depth = $state(0);
  let error = $state<string | null>(null);
  let dialogEl: HTMLDivElement | undefined = $state();
  /** The "new folder" row at the top of the current column, while it's open. */
  let creating = $state(false);
  let newName = $state("");
  let newNameEl: HTMLInputElement | undefined = $state();
  const NEW_FOLDER_KEY = isMac ? "⌘N" : "Ctrl+N";
  let viewportWidth = $state(520);

  const columnWidth = $derived(Math.max(200, viewportWidth - LEFT_PEEK - RIGHT_PEEK - 2 * COLUMN_GAP));
  const offset = $derived(LEFT_PEEK + COLUMN_GAP - depth * (columnWidth + COLUMN_GAP));
  const current = $derived(columns[depth]);

  /** Folder listings already read while this dialog is open. */
  const cache = new Map<string, Item[]>();
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  /** Set when → was pressed before the preview had loaded: enter it once it
   * does. */
  let enterWhenLoaded = false;
  let typed = "";
  let typedAt = 0;
  let errorTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    dialogEl?.focus();
  });

  function close() {
    clearTimeout(previewTimer);
    uiState.closeAgentLaunch();
    uiState.refocusTerminals();
  }

  function start(cwd: string | undefined) {
    const { tool, tileId } = launch;
    close();
    void launchAgent(tool, tileId, cwd);
  }

  function showError(message: string) {
    error = message;
    clearTimeout(errorTimer);
    errorTimer = setTimeout(() => (error = null), 3500);
  }

  /** Home and the drives, then folders agents ran in lately — the likeliest
   * places to start from. */
  async function rootItems(): Promise<Item[]> {
    const roots = await listFolderRoots();
    const items: Item[] = [];
    if (roots.home) items.push({ ...roots.home, tag: "home" });
    for (const drive of roots.drives) items.push({ ...drive, tag: "drive" });
    let recent = 0;
    for (const session of agentSessionsStore.sessions) {
      const path = session.cwd ?? session.projectDir;
      if (!path || items.some((item) => samePath(item.path, path))) continue;
      items.push({ name: path.split(/[\\/]/).filter(Boolean).pop() ?? path, path, tag: "recent" });
      if (++recent >= RECENT_LIMIT) break;
    }
    return items;
  }

  async function openBrowser() {
    step = "browse";
    if (columns.length > 0) return;
    columns = [{ path: null, items: [], selected: 0, status: "loading" }];
    depth = 0;
    try {
      columns[0].items = await rootItems();
      columns[0].status = "ready";
      schedulePreview(0);
    } catch (err) {
      columns[0].status = "error";
      columns[0].error = String(err);
    }
  }

  /** Makes the column after `depth` show what's inside the selected folder,
   * reading it after a short pause (or at once, when it's cached). */
  function schedulePreview(delay = PREVIEW_DELAY_MS) {
    clearTimeout(previewTimer);
    const at = depth;
    const item = columns[at]?.items[columns[at].selected];
    const preview = columns[at + 1];
    if (!item) {
      columns = columns.slice(0, at + 1);
      return;
    }
    if (preview?.path && samePath(preview.path, item.path)) {
      columns = columns.slice(0, at + 2);
      return;
    }
    const cached = cache.get(item.path);
    columns = [
      ...columns.slice(0, at + 1),
      { path: item.path, items: cached ?? [], selected: 0, status: cached ? "ready" : "loading" },
    ];
    if (cached) return afterPreviewLoaded(at);
    previewTimer = setTimeout(() => void loadPreview(at, item.path), delay);
  }

  async function loadPreview(at: number, path: string) {
    let items: Item[] | null = null;
    let failure: string | null = null;
    try {
      items = await listFolders(path);
      cache.set(path, items);
    } catch (err) {
      failure = String(err);
    }
    // The selection may have moved on while this was read.
    const preview = columns[at + 1];
    if (depth !== at || !preview?.path || !samePath(preview.path, path)) return;
    if (items) {
      preview.items = items;
      preview.status = "ready";
    } else {
      preview.status = "error";
      preview.error = failure ?? undefined;
    }
    afterPreviewLoaded(at);
  }

  function afterPreviewLoaded(at: number) {
    if (enterWhenLoaded && depth === at) {
      enterWhenLoaded = false;
      enter();
    }
  }

  function enter() {
    const preview = columns[depth + 1];
    if (!preview) return;
    if (preview.status === "loading") {
      enterWhenLoaded = true;
      // Don't make an explicit → wait out the preview's pause.
      clearTimeout(previewTimer);
      void loadPreview(depth, preview.path!);
      return;
    }
    if (preview.status === "error") {
      showError(preview.error ?? "");
      return;
    }
    creating = false;
    depth += 1;
    schedulePreview();
  }

  function back() {
    enterWhenLoaded = false;
    creating = false;
    if (depth === 0) {
      step = "choose";
      return;
    }
    // The column being left stays right after the new current one: it's the
    // contents of the folder still selected there.
    depth -= 1;
    columns = columns.slice(0, depth + 2);
  }

  function goToDepth(target: number) {
    while (depth > target) back();
  }

  function select(index: number) {
    if (!current || current.items.length === 0) return;
    const next = Math.min(current.items.length - 1, Math.max(0, index));
    if (next === current.selected) return;
    current.selected = next;
    enterWhenLoaded = false;
    schedulePreview();
  }

  function typeAhead(letter: string) {
    if (!current) return;
    const now = Date.now();
    typed = now - typedAt > TYPE_AHEAD_RESET_MS ? letter : typed + letter;
    typedAt = now;
    const index = typeAheadIndex(
      current.items.map((item) => item.name),
      typed,
      current.selected,
    );
    if (index !== null) select(index);
  }

  /** New folders go inside the current one, so not among the starting
   * places, which have no folder of their own. */
  const canCreate = $derived(step === "browse" && !!current?.path && current.status === "ready");

  function startCreate() {
    if (!canCreate) return;
    newName = "";
    creating = true;
    queueMicrotask(() => newNameEl?.focus());
  }

  function cancelCreate() {
    creating = false;
    dialogEl?.focus();
  }

  /** Creates it, lists the folder again with it in place, and selects it —
   * ready for Enter to open the agent right there. */
  async function commitCreate() {
    const column = current;
    const name = newName.trim();
    if (!column?.path || !name) return cancelCreate();
    try {
      const created = await createFolder(column.path, name);
      const items = await listFolders(column.path);
      cache.set(column.path, items);
      cache.set(created.path, []);
      column.items = items;
      creating = false;
      dialogEl?.focus();
      const index = items.findIndex((item) => samePath(item.path, created.path));
      if (index >= 0) column.selected = index;
      schedulePreview(0);
    } catch (err) {
      showError(String(err));
      newNameEl?.focus();
    }
  }

  function onNewNameKeydown(event: KeyboardEvent) {
    // Typing a name mustn't move the selection or jump by letter.
    event.stopPropagation();
    if (event.key === "Enter") {
      event.preventDefault();
      void commitCreate();
    } else if (event.key === "Escape") {
      event.preventDefault();
      cancelCreate();
    }
  }

  function onKeydown(event: KeyboardEvent) {
    // This dialog owns the keyboard while it's open (uiState.overlayOpen keeps
    // the window-level handlers out).
    event.stopPropagation();
    // Ctrl/Cmd+N as asked, and +Shift too: that's "new folder" in Explorer
    // and Finder, so it works from habit as well.
    if ((isMac ? event.metaKey : event.ctrlKey) && event.key.toLowerCase() === "n") {
      event.preventDefault();
      startCreate();
      return;
    }
    if (step === "choose") {
      if (event.key === "ArrowUp" || event.key === "ArrowDown") choice = choice === 0 ? 1 : 0;
      else if (event.key === "Enter") choice === 0 ? start(launch.here ?? undefined) : void openBrowser();
      else if (event.key === "ArrowRight" && choice === 1) void openBrowser();
      else if (event.key === "Escape") close();
      else return;
      event.preventDefault();
      return;
    }

    const selected = current?.selected ?? 0;
    if (event.key === "ArrowDown") select(selected + 1);
    else if (event.key === "ArrowUp") select(selected - 1);
    else if (event.key === "PageDown") select(selected + 8);
    else if (event.key === "PageUp") select(selected - 8);
    else if (event.key === "Home") select(0);
    else if (event.key === "End") select(Infinity);
    else if (event.key === "ArrowRight") enter();
    else if (event.key === "ArrowLeft" || event.key === "Backspace") back();
    else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      if (current?.path) start(current.path);
    } else if (event.key === "Enter") {
      const item = current?.items[selected];
      if (item) start(item.path);
    } else if (event.key === "Escape") step = "choose";
    else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) typeAhead(event.key);
    else return;
    event.preventDefault();
  }

  /** Scrolls a column (only that column — scrollIntoView would also scroll
   * the carousel sideways) so its highlighted row is in view: nudged just
   * inside the edge it went past while moving through the list, centered
   * when it was far off, as in a column just stepped back into. */
  function keepVisible(node: HTMLElement, highlighted: boolean) {
    const reveal = (on: boolean) => {
      const column = node.parentElement;
      if (!on || !column) return;
      const top = node.offsetTop - column.offsetTop;
      const bottom = top + node.offsetHeight;
      const viewTop = column.scrollTop;
      const viewBottom = viewTop + column.clientHeight;
      if (top >= viewTop && bottom <= viewBottom) return;
      const nearby = top > viewTop - node.offsetHeight * 2 && bottom < viewBottom + node.offsetHeight * 2;
      if (!nearby) column.scrollTop = top - (column.clientHeight - node.offsetHeight) / 2;
      else if (top < viewTop) column.scrollTop = top - 6;
      else column.scrollTop = bottom - column.clientHeight + 6;
    };
    reveal(highlighted);
    return { update: reveal };
  }

  /** The breadcrumb for each column after the starting places. */
  function columnLabel(path: string): string {
    return breadcrumbs(path).at(-1)?.label ?? path;
  }

  function onColumnClick(index: number) {
    if (index < depth) goToDepth(index);
    else if (index === depth + 1) enter();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="launch-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<div class="launch-anchor">
  <div
    class="launch-dialog"
    role="dialog"
    tabindex="-1"
    aria-label={t("agentLaunch.title", { tool: toolName })}
    bind:this={dialogEl}
    onpointerdown={(e) => e.stopPropagation()}
    onkeydown={onKeydown}
    transition:fly={{ y: -16, duration: 140 }}
  >
    <header class="launch-header">
      <ConsoleIcon kind={launch.tool} size={16} />
      <h2>{t("agentLaunch.title", { tool: toolName })}</h2>
    </header>

    {#if step === "choose"}
      <div class="launch-choices" in:fly={{ x: -SLIDE_PX, duration: SLIDE_MS, easing: cubicOut }}>
        <button
          class="launch-row"
          class:selected={choice === 0}
          onpointermove={() => (choice = 0)}
          onclick={() => start(launch.here ?? undefined)}
        >
          <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true">
            <path d="M1.5 4.5v8h13v-7h-6l-1.5-1.5h-5.5z" fill="none" stroke="currentColor" stroke-width="1.1" />
          </svg>
          <span class="row-main">
            <span class="row-title">{launch.here ? t("agentLaunch.here") : t("agentLaunch.hereUnknown")}</span>
            {#if launch.here}<span class="row-sub">{launch.here}</span>{/if}
          </span>
          <kbd>Enter</kbd>
        </button>
        <button
          class="launch-row"
          class:selected={choice === 1}
          onpointermove={() => (choice = 1)}
          onclick={() => void openBrowser()}
        >
          <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="7" cy="7" r="4.5" fill="none" stroke="currentColor" stroke-width="1.1" />
            <line x1="10.3" y1="10.3" x2="14" y2="14" stroke="currentColor" stroke-width="1.2" />
          </svg>
          <span class="row-main">
            <span class="row-title">{t("agentLaunch.other")}</span>
            <span class="row-sub">{t("agentLaunch.otherHint")}</span>
          </span>
          <svg class="row-chevron" viewBox="0 0 6 10" aria-hidden="true">
            <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
          </svg>
        </button>
      </div>
    {:else}
      <div class="browser" in:fly={{ x: SLIDE_PX, duration: SLIDE_MS, easing: cubicOut }}>
        <nav class="crumbs">
          {#if depth > 0}
            <button class="crumb" onclick={() => goToDepth(0)}>{t("agentLaunch.places")}</button>
          {:else}
            <span class="crumb current">{t("agentLaunch.places")}</span>
          {/if}
          {#each columns.slice(1, depth + 1) as column, index (index)}
            <span class="crumb-sep">›</span>
            {#if index + 1 < depth}
              <button class="crumb" onclick={() => goToDepth(index + 1)}>{columnLabel(column.path!)}</button>
            {:else}
              <span class="crumb current">{columnLabel(column.path!)}</span>
            {/if}
          {/each}
          <button
            class="new-folder"
            disabled={!canCreate}
            title={canCreate ? `${t("agentLaunch.newFolder")} (${NEW_FOLDER_KEY})` : t("agentLaunch.newFolderUnavailable")}
            onclick={startCreate}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M1.5 4.5v8h13v-7h-6l-1.5-1.5h-5.5z" fill="none" stroke="currentColor" />
              <path d="M8 7v4M6 9h4" stroke="currentColor" />
            </svg>
            <span>{t("agentLaunch.newFolder")}</span>
          </button>
        </nav>

        <div class="viewport" bind:clientWidth={viewportWidth}>
          <div class="track" style="transform: translateX({offset}px)">
            {#each columns as column, index (index)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="column"
                class:current={index === depth}
                class:peek={index !== depth}
                class:behind={index < depth}
                style="width:{columnWidth}px"
                onclick={() => onColumnClick(index)}
              >
                {#if creating && index === depth}
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div class="folder new-folder-row" onclick={(e) => e.stopPropagation()}>
                    <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true">
                      <path d="M1.5 4.5v8h13v-7h-6l-1.5-1.5h-5.5z" fill="none" stroke="currentColor" />
                      <path d="M8 7v4M6 9h4" stroke="currentColor" />
                    </svg>
                    <input
                      class="new-folder-input"
                      bind:this={newNameEl}
                      bind:value={newName}
                      placeholder={t("agentLaunch.newFolderPlaceholder")}
                      aria-label={t("agentLaunch.newFolderPlaceholder")}
                      spellcheck="false"
                      onkeydown={onNewNameKeydown}
                      onblur={() => !newName.trim() && (creating = false)}
                    />
                  </div>
                {/if}
                {#each column.items as item, row (item.path)}
                  {#if item.tag === "recent" && column.items[row - 1]?.tag !== "recent"}
                    <div class="section-divider"></div>
                    <p class="section-title">{t("agentLaunch.recentTitle")}</p>
                  {/if}
                  <button
                    class="folder"
                    class:selected={index === depth && !creating && row === column.selected}
                    tabindex="-1"
                    use:keepVisible={row === column.selected}
                    onclick={(e) => {
                      if (index !== depth) return;
                      e.stopPropagation();
                      select(row);
                    }}
                    ondblclick={() => index === depth && enter()}
                  >
                    {#if item.tag === "drive"}
                      <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true">
                        <rect x="1.5" y="4.5" width="13" height="7" rx="1.5" fill="none" stroke="currentColor" />
                        <circle cx="12" cy="8" r="0.9" fill="currentColor" />
                      </svg>
                    {:else if item.tag === "home"}
                      <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true">
                        <path d="M2 8l6-5.5L14 8M3.8 6.5v7h8.4v-7" fill="none" stroke="currentColor" />
                      </svg>
                    {:else}
                      <svg class="row-icon" viewBox="0 0 16 16" aria-hidden="true">
                        <path d="M1.5 4.5v8h13v-7h-6l-1.5-1.5h-5.5z" fill="none" stroke="currentColor" />
                      </svg>
                    {/if}
                    <span class="folder-name">{item.name}</span>
                    {#if item.tag === "recent"}
                      <span class="folder-path">{item.path}</span>
                    {:else if item.tag}
                      <span class="folder-tag">{t(`agentLaunch.${item.tag}`)}</span>
                    {/if}
                  </button>
                {:else}
                  <p class="column-note">
                    {column.status === "loading"
                      ? t("agentLaunch.loading")
                      : column.status === "error"
                        ? (column.error ?? "")
                        : t("agentLaunch.empty")}
                  </p>
                {/each}
              </div>
            {/each}
          </div>
        </div>

        {#if error}
          <p class="browser-error" transition:fade={{ duration: 100 }}>{error}</p>
        {/if}

        <footer class="keys">
          <span><kbd>↑</kbd><kbd>↓</kbd> {t("agentLaunch.keyMove")}</span>
          <span><kbd>→</kbd> {t("agentLaunch.keyEnter")}</span>
          <span><kbd>←</kbd> {t("agentLaunch.keyBack")}</span>
          <span><kbd>Enter</kbd> {t("agentLaunch.keyOpen")}</span>
          {#if current?.path}
            <span><kbd>Ctrl+Enter</kbd> {t("agentLaunch.keyOpenHere")}</span>
          {/if}
          <span><kbd>a–z</kbd> {t("agentLaunch.keyType")}</span>
          {#if canCreate}
            <span><kbd>{NEW_FOLDER_KEY}</kbd> {t("agentLaunch.keyNewFolder")}</span>
          {/if}
        </footer>
      </div>
    {/if}
  </div>
</div>

<style>
  .launch-backdrop {
    position: absolute;
    inset: 0;
    z-index: 399;
    background: rgba(0, 0, 0, 0.55);
  }

  /* The dialog carries the fly transition, which animates transform — so the
     centering lives on this wrapper, same as the other dialogs. */
  .launch-anchor {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 400;
    display: flex;
    justify-content: center;
    pointer-events: none;
  }

  .launch-dialog {
    pointer-events: auto;
    width: min(680px, 94%);
    margin-top: 8vh;
    display: flex;
    flex-direction: column;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
    color: var(--titlebar-fg, #d4d4d4);
    overflow: hidden;
  }

  .launch-dialog:focus {
    outline: none;
  }

  .launch-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .launch-header h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }

  .launch-choices {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
  }

  /* Rows share the tile picker's selection: a tinted fill inside an accent
     border, drawn at the same size so selecting doesn't shift anything. */
  .launch-row,
  .folder {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
    box-sizing: border-box;
    width: 100%;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      background 90ms ease,
      border-color 90ms ease;
  }

  .launch-row {
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.03);
    border-color: var(--tabbar-border, #2a2a2a);
  }

  .launch-row.selected,
  .folder.selected {
    background: rgba(58, 109, 240, 0.16);
    border-color: var(--pane-active-border, #3a6df0);
  }

  .row-icon {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    opacity: 0.85;
  }

  .row-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .row-title {
    font-size: 13px;
  }

  .row-sub {
    font-size: 11px;
    opacity: 0.6;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-chevron {
    width: 6px;
    height: 10px;
    flex-shrink: 0;
    opacity: 0.5;
  }

  .browser {
    display: flex;
    flex-direction: column;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 30px;
    padding: 4px 10px;
    overflow: hidden;
    white-space: nowrap;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    font-size: 12px;
  }

  .crumb {
    padding: 2px 5px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    opacity: 0.65;
    cursor: pointer;
  }

  .crumb:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .crumb.current {
    opacity: 1;
    font-weight: 600;
    cursor: default;
  }

  .crumb-sep {
    opacity: 0.35;
  }

  .new-folder {
    margin-left: auto;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .new-folder svg {
    width: 14px;
    height: 14px;
  }

  .new-folder:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .new-folder:disabled {
    opacity: 0.35;
    cursor: default;
  }

  /* Sits where the new folder will appear, drawn as a selected row so it's
     clear that's where typing goes. */
  .new-folder-row {
    cursor: default;
    background: rgba(58, 109, 240, 0.16);
    border-color: var(--pane-active-border, #3a6df0);
  }

  .new-folder-input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    outline: none;
  }

  /* The carousel window: the current column in full, with the column behind
     it and the preview ahead cut off at the edges, fading out. */
  .viewport {
    position: relative;
    height: 320px;
    overflow: hidden;
    -webkit-mask-image: linear-gradient(
      to right,
      transparent 0,
      #000 32px,
      #000 calc(100% - 48px),
      transparent 100%
    );
    mask-image: linear-gradient(to right, transparent 0, #000 32px, #000 calc(100% - 48px), transparent 100%);
  }

  /* The whole strip of columns moves as one, which is what makes a step
     deeper or back read as sliding along rather than redrawing. */
  .track {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    display: flex;
    gap: 8px;
    transition: transform 220ms cubic-bezier(0.22, 1, 0.36, 1);
    will-change: transform;
  }

  .column {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 0;
    overflow-y: auto;
    scrollbar-width: thin;
    transition: opacity 220ms ease;
  }

  /* The columns on either side: dimmed, and a click on one moves to it. */
  .column.peek {
    opacity: 0.2;
    cursor: pointer;
    scrollbar-width: none;
  }

  .column.peek .folder {
    pointer-events: none;
  }
  /* Only the right edge of the column behind shows, so its rows line up
     against that edge — the folder just left sits right beside the current
     column instead of off at the far, hidden end of its row. */
  .column.behind .folder {
    flex-direction: row-reverse;
  }

  .column.behind .folder-tag,
  .column.behind .folder-path,
  .column.behind .section-title {
    display: none;
  }

  .folder {
    padding: 6px 10px;
    font-size: 12px;
  }

  .column.current .folder:hover:not(.selected) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .folder-name {
    flex-shrink: 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder-tag {
    flex-shrink: 0;
    padding: 0 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    font-size: 10px;
    opacity: 0.75;
  }

  .folder-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    opacity: 0.45;
  }

  /* Keeps the recent folders visibly apart from the filesystem above them. */
  .section-divider {
    flex-shrink: 0;
    height: 1px;
    margin: 6px 8px 2px;
    background: var(--tabbar-border, #2a2a2a);
  }

  .section-title {
    flex-shrink: 0;
    margin: 0;
    padding: 4px 10px 2px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.55;
  }

  .column-note {
    margin: 0;
    padding: 10px 12px;
    font-size: 12px;
    opacity: 0.55;
  }

  .browser-error {
    margin: 0;
    padding: 6px 14px;
    font-size: 11px;
    color: #ff6b6b;
  }

  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    padding: 8px 14px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
    font-size: 11px;
    opacity: 0.7;
  }

  kbd {
    display: inline-block;
    min-width: 14px;
    margin-right: 2px;
    padding: 0 4px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.05);
    font: inherit;
    font-size: 10px;
    text-align: center;
  }

  .launch-row kbd {
    flex-shrink: 0;
    opacity: 0.6;
  }
</style>
