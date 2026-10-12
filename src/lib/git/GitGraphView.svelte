<script lang="ts">
  import { untrack } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    gitCommitDetail,
    gitGraph,
    gitSnapshot,
    type CommitDetail,
    type CommitFile,
    type GraphCommit,
    type GraphScope,
  } from "$lib/core/git-client";
  import { MARK_LETTER, markColor } from "$lib/explorer/git-marks";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { fileName } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import CommitGraph from "./CommitGraph.svelte";
  import { commitSides, emptyGraphState, layoutGraph, type GraphRow, type GraphState } from "./git-graph";
  import { gitStore } from "./git-store.svelte";
  import { gitReviewStore } from "./git-review-store.svelte";
  import { splitPath } from "./git-summary";

  /** A repository's whole history as a graph, a page at a time, with any
   * commit's message and files a click away. */
  let { root }: { root: string } = $props();

  const PAGE = 200;

  let scope = $state<GraphScope>("all");
  let rows = $state<GraphRow<GraphCommit>[]>([]);
  let commits = $state<GraphCommit[]>([]);
  let layout: GraphState = emptyGraphState();
  let loading = $state(false);
  /** Whether the last page came back full: there may be more. */
  let more = $state(false);
  let selected = $state<string | null>(null);
  let detail = $state<CommitDetail | null>(null);
  let upstream = $state<string | null>(null);
  let copied = $state(false);
  /** Bumped by every fresh load, so a slow page of an older one is dropped. */
  let generation = 0;

  const sides = $derived.by(() => {
    const head = commits.find((commit) => commit.detachedHead || commit.refs.some((ref) => ref.kind === "head"))?.sha ?? null;
    const tracked = upstream
      ? (commits.find((commit) => commit.refs.some((ref) => ref.kind === "remote" && ref.name === upstream))?.sha ?? null)
      : null;
    return commitSides(commits, head, tracked);
  });

  /** Where the branch in hand and its upstream are, to tell what's still to
   * push or pull; this repository may not be the one the panel is on. */
  async function extras(): Promise<string[]> {
    const snapshot = gitStore.root === root ? gitStore.snapshot : await gitSnapshot(root).catch(() => null);
    upstream = snapshot?.upstream ?? null;
    return [snapshot?.upstream, snapshot?.base?.name].filter((name): name is string => !!name);
  }

  async function reload() {
    const mine = ++generation;
    loading = true;
    // As many as were showing, so a refresh doesn't fold the list back up.
    const want = Math.max(PAGE, Math.ceil(commits.length / PAGE) * PAGE);
    const page = await gitGraph(root, scope, await extras(), 0, Math.min(want, 500)).catch(() => []);
    if (mine !== generation) return;
    const laid = layoutGraph(page);
    layout = laid.state;
    rows = laid.rows;
    commits = page;
    more = page.length >= Math.min(want, 500);
    loading = false;
  }

  async function loadMore() {
    if (loading) return;
    const mine = generation;
    loading = true;
    const page = await gitGraph(root, scope, await extras(), commits.length, PAGE).catch(() => []);
    if (mine !== generation) return;
    const laid = layoutGraph(page, layout);
    layout = laid.state;
    rows = [...rows, ...laid.rows];
    commits = [...commits, ...page];
    more = page.length >= PAGE;
    loading = false;
  }

  // Read when it opens, when its scope changes, and whenever the panel
  // notices the history of this same repository moved.
  $effect(() => {
    void scope;
    if (gitStore.root === root) void gitStore.historyKey;
    // Only those bring a new read: not what reading itself looks at.
    untrack(() => void reload());
  });

  $effect(() => {
    const sha = selected;
    detail = null;
    if (!sha) return;
    let cancelled = false;
    void gitCommitDetail(root, sha).then((found) => {
      if (!cancelled) detail = found;
    });
    return () => {
      cancelled = true;
    };
  });

  function openFile(file: CommitFile) {
    if (!detail) return;
    workspaceStore.openDiff({
      root,
      path: file.path,
      // A first commit has nothing before it; neither does a file it added.
      from: file.mark === "added" ? null : (detail.parents[0] ?? null),
      to: detail.sha,
      ...(file.origPath ? { fromPath: file.origPath } : {}),
    });
  }

  function reviewCommit() {
    if (detail) gitReviewStore.ask({ kind: "commit", sha: detail.sha });
  }

  function copySha() {
    if (!detail) return;
    void writeText(detail.sha).then(() => {
      copied = true;
      setTimeout(() => (copied = false), 1500);
    });
  }
</script>

<div class="graph-view">
  <div class="graph-bar">
    <span class="graph-title" title={root}>{fileName(root)}</span>
    <div class="scope" role="group" aria-label={t("git.graph.scope")}>
      {#each ["all", "current"] as const as option (option)}
        <button class:active={scope === option} aria-pressed={scope === option} onclick={() => (scope = option)}>
          {t(`git.graph.scopes.${option}`)}
        </button>
      {/each}
    </div>
    <span class="legend"><i class="swatch ahead"></i>{t("git.graph.legendAhead")}</span>
    <span class="legend"><i class="swatch behind"></i>{t("git.graph.legendBehind")}</span>
    <button class="bar-button" disabled={loading} onclick={() => void reload()}>{t(loading ? "git.loading" : "git.refresh")}</button>
  </div>
  <div class="graph-main">
    <div class="graph-list">
      {#if rows.length === 0 && !loading}
        <p class="note">{t("git.graph.empty")}</p>
      {/if}
      <CommitGraph {rows} {sides} {selected} onpick={(commit) => (selected = selected === commit.sha ? null : commit.sha)} />
      {#if more}
        <button class="more" disabled={loading} onclick={() => void loadMore()}>
          {t(loading ? "git.loading" : "git.graph.more")}
        </button>
      {/if}
    </div>
    {#if selected}
      <aside class="detail" aria-label={t("git.graph.detail")}>
        {#if !detail}
          <p class="note">{t("git.loading")}</p>
        {:else}
          <div class="detail-head">
            <button class="sha" title={t("git.copy")} onclick={copySha}>{copied ? t("git.copied") : detail.sha.slice(0, 12)}</button>
            <button class="bar-button" aria-label={t("git.close")} onclick={() => (selected = null)}>✕</button>
          </div>
          <p class="who">
            {detail.author}
            <span class="dim">&lt;{detail.email}&gt;</span>
          </p>
          <p class="dim">{new Date(detail.time * 1000).toLocaleString(i18n.locale, { dateStyle: "full", timeStyle: "short" })}</p>
          <pre class="message">{detail.message}</pre>
          <!-- The reviews are of the repository in the panel: this tab's, when it's the one in view. -->
          {#if gitReviewStore.available && gitStore.root === root && detail.parents.length <= 1}
            <button class="review-commit" disabled={gitReviewStore.busy} onclick={reviewCommit}>
              {t("git.review.reviewCommit")}
            </button>
          {/if}
          <p class="files-title">{t("git.graph.files", { n: String(detail.files.length) })}</p>
          {#if detail.parents.length > 1}
            <p class="dim">{t("git.graph.mergeNote")}</p>
          {/if}
          {#each detail.files as file (file.path)}
            {@const parts = splitPath(file.path)}
            <button class="file" style="color:{markColor(file.mark, themeStore.current.colors)}" title={file.path} onclick={() => openFile(file)}>
              <span class="file-mark">{MARK_LETTER[file.mark]}</span>
              <span class="file-name">{parts.name}</span>
              <span class="file-folder">{parts.folder}</span>
            </button>
          {/each}
        {/if}
      </aside>
    {/if}
  </div>
</div>

<style>
  .graph-view {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--terminal-bg, transparent);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .graph-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 30px;
    padding: 0 10px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .graph-title {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .scope {
    display: flex;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    overflow: hidden;
  }

  .scope button {
    padding: 2px 9px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .scope button.active {
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 10.5px;
    opacity: 0.7;
    white-space: nowrap;
  }

  .legend:first-of-type {
    margin-left: auto;
  }

  .swatch {
    width: 8px;
    height: 8px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 1.6px solid currentColor;
  }

  .swatch.ahead {
    background: currentColor;
    box-shadow: 0 0 0 2px rgba(128, 128, 128, 0.45);
  }

  .bar-button,
  .more {
    padding: 2px 9px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .bar-button:hover:not(:disabled),
  .more:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .graph-main {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .graph-list {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 4px 0 10px;
    scrollbar-width: thin;
  }

  .more {
    display: block;
    margin: 8px auto 0;
  }

  .note {
    margin: 0;
    padding: 14px;
    opacity: 0.6;
  }

  .detail {
    flex: 0 0 min(340px, 42%);
    min-width: 0;
    overflow-y: auto;
    padding: 8px 10px 12px;
    border-left: 1px solid var(--tabbar-border, #2a2a2a);
    scrollbar-width: thin;
  }

  .detail p {
    margin: 0 0 3px;
  }

  .detail-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .sha {
    padding: 1px 7px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    cursor: pointer;
  }

  .who {
    font-weight: 600;
  }

  .dim {
    font-weight: 400;
    font-size: 11px;
    opacity: 0.6;
  }

  .message {
    margin: 8px 0;
    padding: 7px 8px;
    border-radius: 5px;
    background: rgba(128, 128, 128, 0.13);
    font-family: inherit;
    font-size: 12px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .review-commit {
    align-self: flex-start;
    margin-top: 8px;
    padding: 3px 9px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .review-commit:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .review-commit:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .files-title {
    margin-top: 8px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.7;
  }

  .file {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    padding: 3px 4px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .file:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .file-mark {
    flex-shrink: 0;
    width: 10px;
    font-size: 10px;
    font-weight: 700;
  }

  .file-name {
    flex-shrink: 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-folder {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 10.5px;
    opacity: 0.5;
  }
</style>
