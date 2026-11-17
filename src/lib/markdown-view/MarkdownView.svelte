<script lang="ts">
  import { fileStamps, readTextFile } from "$lib/core/explorer-client";
  import { openLink } from "$lib/core/links";
  import { isWindows } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";
  import { fileFolder } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { resolveMarkdownPath } from "$lib/terminal/markdown-links";
  import { linkTarget, renderMarkdownDocument } from "./markdown-render";

  /** A Markdown file rendered, to read: what an agent wrote, kept up to date
   * as it goes on writing it. Not an editor — "Open in the editor" is. */
  let { sessionId, path, active }: { sessionId: string; path: string; active: boolean } = $props();

  /** How often the file is checked for changes while the tab is in sight. */
  const WATCH_MS = 1000;

  let html = $state("");
  let error = $state<string | null>(null);
  let loaded = $state(false);
  let bodyEl: HTMLElement | undefined = $state();
  /** When the file was last written, as read; null before the first read. */
  let stamp: number | null = null;
  /** Bumped by every read, so a slow one never lands over a newer one. */
  let generation = 0;

  async function load() {
    const mine = ++generation;
    try {
      const [[written], text] = await Promise.all([fileStamps([path]), readTextFile(path)]);
      if (mine !== generation) return;
      stamp = written;
      // Where the reader was stays put: an agent rewriting the file
      // shouldn't throw them back to the top.
      const scroll = bodyEl?.scrollTop ?? 0;
      html = renderMarkdownDocument(text);
      error = null;
      requestAnimationFrame(() => {
        if (bodyEl) bodyEl.scrollTop = scroll;
      });
    } catch (err) {
      if (mine !== generation) return;
      error = String(err);
      stamp = null;
    } finally {
      if (mine === generation) loaded = true;
    }
  }

  $effect(() => {
    void path;
    loaded = false;
    stamp = null;
    void load();
  });

  // Re-read when the file changes, but only while the tab is in sight; one
  // coming back into view checks at once.
  $effect(() => {
    if (!active) return;
    let stopped = false;
    const check = async () => {
      const [written] = await fileStamps([path]).catch(() => [null]);
      if (stopped) return;
      if (written !== stamp) void load();
    };
    void check();
    const timer = setInterval(check, WATCH_MS);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  });

  function openInEditor() {
    const tileId = workspaceStore.focusedTileId;
    if (tileId) workspaceStore.openFile(tileId, path);
  }

  function follow(anchor: HTMLElement) {
    const target = linkTarget(anchor.dataset.href ?? "");
    if (target.kind === "web") {
      void openLink(target.url).catch(() => {});
    } else if (target.kind === "anchor") {
      jumpTo(target.id);
    } else if (target.kind === "file") {
      const resolved = resolveMarkdownPath(target.path, fileFolder(path), isWindows);
      if (!resolved) return;
      if (/\.(md|markdown|mdx)$/i.test(resolved)) workspaceStore.openMarkdown(resolved, sessionId);
      else {
        const tileId = workspaceStore.focusedTileId;
        if (tileId) workspaceStore.openFile(tileId, resolved);
      }
    }
  }

  function jumpTo(id: string) {
    bodyEl?.querySelector(`[id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: "start" });
  }

  function linkFrom(event: Event): HTMLElement | null {
    return (event.target as HTMLElement | null)?.closest<HTMLElement>("a[data-href]") ?? null;
  }

  function onClick(event: MouseEvent) {
    const anchor = linkFrom(event);
    if (!anchor) return;
    event.preventDefault();
    follow(anchor);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter") return;
    const anchor = linkFrom(event);
    if (!anchor) return;
    event.preventDefault();
    follow(anchor);
  }
</script>

<div class="markdown-view">
  <header class="md-header">
    <span class="md-path" title={path}>{path}</span>
    <button class="md-button" onclick={openInEditor}>{t("markdownView.openInEditor")}</button>
  </header>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <article class="md-body" bind:this={bodyEl} onclick={onClick} onkeydown={onKeydown}>
    {#if error}
      <p class="md-error">{t("markdownView.readFailed", { error })}</p>
    {:else if loaded}
      <div class="md-doc">
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        {@html html}
      </div>
    {/if}
  </article>
</div>

<style>
  .markdown-view {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--terminal-bg, #1a1a1a);
    color: var(--titlebar-fg, #d4d4d4);
    overflow: hidden;
  }

  .md-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 14px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    font-size: 12px;
  }

  .md-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.65;
  }

  .md-button {
    flex-shrink: 0;
    padding: 3px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .md-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .md-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 24px 32px 48px;
  }

  .md-error {
    opacity: 0.75;
  }

  .md-doc {
    max-width: 860px;
    margin: 0 auto;
    font-size: 14px;
    line-height: 1.6;
    overflow-wrap: break-word;
    user-select: text;
  }

  .md-doc :global(> :first-child) {
    margin-top: 0;
  }

  .md-doc :global(p),
  .md-doc :global(ul),
  .md-doc :global(ol),
  .md-doc :global(pre),
  .md-doc :global(blockquote),
  .md-doc :global(table) {
    margin: 0 0 14px;
  }

  .md-doc :global(h1),
  .md-doc :global(h2),
  .md-doc :global(h3),
  .md-doc :global(h4),
  .md-doc :global(h5),
  .md-doc :global(h6) {
    margin: 24px 0 12px;
    font-weight: 600;
    line-height: 1.3;
  }

  .md-doc :global(h1),
  .md-doc :global(h2) {
    padding-bottom: 6px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .md-doc :global(h1) {
    font-size: 1.9em;
  }

  .md-doc :global(h2) {
    font-size: 1.45em;
  }

  .md-doc :global(h3) {
    font-size: 1.2em;
  }

  .md-doc :global(h4) {
    font-size: 1.05em;
  }

  .md-doc :global(h5),
  .md-doc :global(h6) {
    font-size: 0.95em;
    opacity: 0.85;
  }

  .md-doc :global(ul),
  .md-doc :global(ol) {
    padding-left: 24px;
  }

  .md-doc :global(li) {
    margin-bottom: 4px;
  }

  .md-doc :global(li > ul),
  .md-doc :global(li > ol),
  .md-doc :global(li > p) {
    margin: 4px 0 0;
  }

  /* A task list's checkboxes stand in for the bullet. */
  .md-doc :global(li:has(> input[type="checkbox"])),
  .md-doc :global(li:has(> p > input[type="checkbox"])) {
    list-style: none;
    margin-left: -20px;
  }

  .md-doc :global(input[type="checkbox"]) {
    margin: 0 6px 0 0;
    vertical-align: middle;
  }

  .md-doc :global(code) {
    padding: 1px 5px;
    border-radius: 4px;
    font-family: var(--terminal-font-family, ui-monospace, Consolas, monospace);
    font-size: 0.88em;
    background: rgba(127, 127, 127, 0.2);
  }

  .md-doc :global(pre) {
    padding: 12px 14px;
    overflow-x: auto;
    border-radius: 6px;
    background: rgba(127, 127, 127, 0.14);
    line-height: 1.45;
  }

  .md-doc :global(pre code) {
    padding: 0;
    background: none;
    white-space: pre;
  }

  .md-doc :global(blockquote) {
    padding: 0 0 0 12px;
    border-left: 3px solid var(--tabbar-border, #2a2a2a);
    opacity: 0.85;
  }

  .md-doc :global(hr) {
    border: none;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
    margin: 20px 0;
  }

  .md-doc :global(table) {
    display: block;
    width: max-content;
    max-width: 100%;
    overflow-x: auto;
    border-collapse: collapse;
  }

  .md-doc :global(th),
  .md-doc :global(td) {
    padding: 5px 12px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .md-doc :global(th) {
    font-weight: 600;
    background: rgba(127, 127, 127, 0.1);
  }

  .md-doc :global(a[data-href]) {
    color: var(--pane-active-border, #3a6df0);
    text-decoration: underline;
    cursor: pointer;
  }

  .md-doc :global(img) {
    max-width: 100%;
  }

  .md-doc :global(.md-image-missing) {
    padding: 1px 6px;
    border: 1px dashed var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    font-size: 0.9em;
    opacity: 0.7;
  }
</style>
