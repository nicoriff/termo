<script lang="ts">
  import { untrack } from "svelte";
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { t } from "$lib/i18n/i18n.svelte";
  import { ago } from "$lib/agents/agent-spec";
  import { agentsStore } from "$lib/agents/agents-store.svelte";
  import SidebarResizeHandle from "$lib/app/SidebarResizeHandle.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { gitGraph, type Branch, type FileChange, type GraphCommit } from "$lib/core/git-client";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { MARK_LETTER, markColor } from "$lib/explorer/git-marks";
  import { fileName, type DiffTarget } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import CommitGraph from "./CommitGraph.svelte";
  import DivergenceGraph from "./DivergenceGraph.svelte";
  import ForgeIcon from "./ForgeIcon.svelte";
  import GitPrSection from "./GitPrSection.svelte";
  import { commitSides, layoutGraph } from "./git-graph";
  import { gitStore } from "./git-store.svelte";
  import { CHANGE_GROUPS, GROUP_LABEL, groupChanges, markIn, splitPath, type ChangeGroup } from "./git-summary";

  /** How much history the panel's own graph shows; the tab has the rest. */
  const GRAPH_COMMITS = 30;
  const COLLAPSED_KEY = "termo.git.collapsedSections";

  type SectionId = "changes" | "pr" | "branches" | "history" | "remotes";

  function loadCollapsed(): SectionId[] {
    try {
      const saved = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? '["remotes"]');
      return Array.isArray(saved) ? saved : [];
    } catch {
      return ["remotes"];
    }
  }

  const snap = $derived(gitStore.snapshot);
  const root = $derived(gitStore.root);
  const groups = $derived(groupChanges(snap?.files ?? []));
  const counts = $derived(gitStore.counts);
  const busy = $derived(gitStore.running !== null);
  const forge = $derived(gitStore.forge);
  const repoName = $derived(forge ? `${forge.owner}/${forge.repo}` : root ? fileName(root) : t("git.title"));
  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleGit"), isMac));

  let collapsed = $state<SectionId[]>(loadCollapsed());
  let menuOpen = $state(false);
  /** The name being typed for a new branch; null while not asked for one. */
  let newBranch = $state<string | null>(null);
  let amend = $state(false);
  /** What's waiting on its "this can't be undone": a file's row, or a
   * whole section. */
  let confirming = $state<string | null>(null);
  let outputOpen = $state(false);
  let showRemoteBranches = $state(false);
  let copied = $state<string | null>(null);
  let now = $state(Date.now());
  let messageEl: HTMLTextAreaElement | undefined = $state();
  let commits = $state<GraphCommit[]>([]);

  const message = $derived(gitStore.draft(root));
  /** With nothing staged, a commit takes everything: the common case of
   * "commit what I did" without staging file by file. */
  const commitsAll = $derived(counts.staged === 0);
  const canCommit = $derived(
    !busy &&
      counts.conflicts === 0 &&
      (amend ? message.trim() !== "" || snap?.head != null : message.trim() !== "" && counts.total > 0),
  );

  const localBranches = $derived(gitStore.branches.filter((branch) => !branch.remote));
  const remoteBranches = $derived(gitStore.branches.filter((branch) => branch.remote));
  const shownBranches = $derived(showRemoteBranches ? [...localBranches, ...remoteBranches] : localBranches);
  /** What the longest bar stands for: the branch furthest from its upstream. */
  const farthest = $derived(Math.max(1, ...gitStore.branches.map((branch) => Math.max(branch.ahead, branch.behind))));

  const graphRows = $derived(layoutGraph(commits).rows);
  const graphSides = $derived.by(() => {
    const head = commits.find((commit) => commit.detachedHead || commit.refs.some((ref) => ref.kind === "head"))?.sha ?? null;
    const tracked = snap?.upstream;
    const upstream = tracked ? (commits.find((commit) => commit.refs.some((ref) => ref.kind === "remote" && ref.name === tracked))?.sha ?? null) : null;
    return commitSides(commits, head, upstream);
  });

  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(timer);
  });

  // The panel's own slice of history, read again whenever it may have moved.
  $effect(() => {
    void gitStore.historyKey;
    if (!root || collapsed.includes("history")) return;
    // historyKey says when to read again; an edited file isn't a reason to.
    const extra = untrack(() => [snap?.upstream, snap?.base?.name].filter((name): name is string => !!name));
    let cancelled = false;
    void gitGraph(root, "current", extra, 0, GRAPH_COMMITS).then((page) => {
      if (!cancelled) commits = page;
    });
    return () => {
      cancelled = true;
    };
  });

  // "Commit…" from the status bar or the palette lands in the message.
  $effect(() => {
    if (gitStore.commitFocusNonce > 0) messageEl?.focus();
  });

  function toggleSection(id: SectionId) {
    collapsed = collapsed.includes(id) ? collapsed.filter((other) => other !== id) : [...collapsed, id];
    try {
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify(collapsed));
    } catch {
      // Only a convenience.
    }
  }

  function close() {
    gitStore.setOpen(false);
    uiState.refocusTerminals();
  }

  function age(ms: number): string {
    const value = ago(ms, now);
    return t(value.key, value.params);
  }

  /** The two versions a file's row compares, by the section it's in. */
  function diffOf(file: FileChange, group: ChangeGroup): DiffTarget {
    const base = { root: root ?? "", path: file.path };
    if (group === "staged") return { ...base, from: "HEAD", to: "", ...(file.origPath ? { fromPath: file.origPath } : {}) };
    if (group === "conflicts") return { ...base, from: "HEAD", to: null };
    // Nothing in the index to compare a new file with.
    if (group === "untracked") return { ...base, from: null, to: null };
    return { ...base, from: "", to: null };
  }

  /** A staged rename is two paths in the index: both go back. */
  const pathsOf = (file: FileChange) => (file.origPath ? [file.path, file.origPath] : [file.path]);

  function stage(files: FileChange[]) {
    void gitStore.run({ kind: "stage", paths: files.map((file) => file.path) });
  }

  function unstage(files: FileChange[]) {
    void gitStore.run({ kind: "unstage", paths: files.flatMap(pathsOf) });
  }

  function discard(files: FileChange[]) {
    confirming = null;
    void gitStore.run({
      kind: "discard",
      tracked: files.filter((file) => file.unstaged !== "untracked").map((file) => file.path),
      untracked: files.filter((file) => file.unstaged === "untracked").map((file) => file.path),
    });
  }

  function commit(push: boolean) {
    if (!canCommit) return;
    // Amending with nothing staged only rewords: what's on disk stays out.
    void gitStore.commit(message, { amend, all: commitsAll && !amend, push }).then((done) => {
      if (done) amend = false;
    });
  }

  function onMessageKey(event: KeyboardEvent) {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      commit(false);
    }
  }

  function switchTo(branch: Branch) {
    // A remote's branch that's already here under its own name: that one.
    const short = branch.remote ? branch.name.slice(branch.name.indexOf("/") + 1) : branch.name;
    const local = localBranches.find((candidate) => candidate.name === short);
    void gitStore.run(branch.remote && !local ? { kind: "checkout", branch: branch.name, remote: true } : { kind: "checkout", branch: short, remote: false });
  }

  function createBranch() {
    const name = newBranch?.trim();
    if (!name) return;
    void gitStore.run({ kind: "createBranch", name }).then((done) => {
      if (done) newBranch = null;
    });
  }

  function runFromMenu(action: () => void) {
    menuOpen = false;
    action();
  }

  function copy(text: string) {
    void writeText(text).then(() => {
      copied = text;
      setTimeout(() => {
        if (copied === text) copied = null;
      }, 1500);
    });
  }

  function open(url: string | null | undefined) {
    if (url && /^https?:\/\//i.test(url)) void openUrl(url);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    if (menuOpen || newBranch !== null || confirming !== null) {
      menuOpen = false;
      newBranch = null;
      confirming = null;
    } else uiState.refocusTerminals();
  }

  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onpointerdown={() => (menuOpen = false)} />

{#snippet sectionHead(id: SectionId, label: string, count: number | null)}
  <button class="section-head" aria-expanded={!collapsed.includes(id)} onclick={() => toggleSection(id)}>
    <svg class="chevron" class:open={!collapsed.includes(id)} viewBox="0 0 6 10" aria-hidden="true">
      <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
    </svg>
    <span class="section-title">{label}</span>
    {#if count !== null && count > 0}
      <span class="section-count">{count}</span>
    {/if}
  </button>
{/snippet}

<aside
  class="git-panel"
  style="--panel-width:{agentsStore.width}px"
  transition:slide={{ axis: "x", duration: 180, easing: cubicOut }}
  aria-label={t("git.title")}
>
  <SidebarResizeHandle />
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="git-inner" onkeydown={onKeydown}>
    <header class="git-header">
      <ForgeIcon kind={snap ? (forge?.kind ?? "generic") : null} />
      <div class="header-text">
        <span class="header-name" title={root ?? ""}>{repoName}</span>
        {#if forge}
          <span class="header-note">{forge.host}</span>
        {/if}
      </div>
      {#if forge?.webUrl}
        <button class="icon-button" title={t("git.openInBrowser")} aria-label={t("git.openInBrowser")} onclick={() => open(forge.webUrl)}>
          <svg viewBox="0 0 12 12" aria-hidden="true">
            <path d="M5 2.5H2.5v7h7V7M7 2.5h2.5V5M9.5 2.5 5.5 6.5" fill="none" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </button>
      {/if}
      <button class="icon-button" title={t("git.refresh")} aria-label={t("git.refresh")} onclick={() => void gitStore.refresh()}>
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M13 8a5 5 0 1 1-1.5-3.55M13 2.5v2.5h-2.5" fill="none" stroke="currentColor" stroke-width="1.2" />
        </svg>
      </button>
      <button class="icon-button" title={`${t("git.close")} (${shortcut})`} aria-label={t("git.close")} onclick={close}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    {#if !snap}
      <p class="empty-note">{t(!gitStore.loaded ? "git.loading" : "git.notRepo")}</p>
    {:else}
      <div class="git-body">
        <!-- The branch, and how far it is from what it tracks and from the base. -->
        <section class="branch-block">
          <div class="branch-line">
            <svg class="branch-icon" viewBox="0 0 16 16" aria-hidden="true">
              <path d="M5 3.8v8.4M5 8.2c4.4 0 6-.6 6-3.4" fill="none" stroke="currentColor" stroke-width="1.3" />
              <circle cx="5" cy="3" r="1.6" fill="none" stroke="currentColor" stroke-width="1.3" />
              <circle cx="5" cy="13" r="1.6" fill="none" stroke="currentColor" stroke-width="1.3" />
              <circle cx="11" cy="4" r="1.6" fill="none" stroke="currentColor" stroke-width="1.3" />
            </svg>
            <span class="branch-current" title={snap.head?.sha ?? ""}>
              {snap.branch ?? t("git.detached", { sha: snap.head?.sha.slice(0, 7) ?? "" })}
            </span>
            {#if snap.lastFetch}
              <span class="fetched" title={t("git.lastFetchHint")}>{t("git.lastFetch", { time: age(snap.lastFetch) })}</span>
            {/if}
          </div>

          {#if snap.upstream && !snap.upstreamGone}
            <DivergenceGraph ours={snap.branch ?? "HEAD"} theirs={snap.upstream} ahead={snap.ahead} behind={snap.behind} />
          {:else if snap.upstreamGone}
            <p class="notice warn">{t("git.upstreamGone", { upstream: snap.upstream ?? "" })}</p>
          {:else if snap.branch && snap.remotes.length > 0}
            <p class="notice">{t("git.noUpstream")}</p>
          {/if}
          {#if snap.base}
            <DivergenceGraph ours={snap.branch ?? "HEAD"} theirs={snap.base.name} ahead={snap.base.ahead} behind={snap.base.behind} />
          {/if}

          {#if snap.state}
            <p class="notice warn">{t(`git.states.${snap.state}`)}</p>
          {/if}
          {#if snap.locked && !busy}
            <p class="notice">{t("git.locked")}</p>
          {:else if gitStore.consoleRunsGit && !busy}
            <p class="notice">{t("git.consoleRunsGit")}</p>
          {/if}
          {#if snap.changesUnavailable}
            <p class="notice warn">{t("git.changesUnavailable")}</p>
          {/if}

          {#if gitStore.running}
            <div class="running">
              <span class="spinner" aria-hidden="true"></span>
              <span class="running-text">
                <span>{t(`git.ops.${gitStore.running.op}`)}</span>
                {#if gitStore.running.lines.length > 0}
                  <span class="running-line">{gitStore.running.lines[gitStore.running.lines.length - 1]}</span>
                {/if}
              </span>
              <button class="small" onclick={() => gitStore.cancel()}>{t("git.cancel")}</button>
            </div>
          {/if}
          {#if gitStore.error}
            <div class="error-box" role="alert">
              <pre>{gitStore.error}</pre>
              <button class="icon-button" title={t("git.dismiss")} aria-label={t("git.dismiss")} onclick={() => (gitStore.error = null)}>
                <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
              </button>
            </div>
          {:else if gitStore.lastOutput && !busy}
            <button class="link" aria-expanded={outputOpen} onclick={() => (outputOpen = !outputOpen)}>
              {t(outputOpen ? "git.hideOutput" : "git.showOutput")}
            </button>
            {#if outputOpen}
              <pre class="output">{gitStore.lastOutput}</pre>
            {/if}
          {/if}

          <div class="actions">
            <button class="action" disabled={busy || snap.remotes.length === 0} title={t("git.actions.fetchHint")} onclick={() => void gitStore.run({ kind: "fetch" })}>
              {t("git.actions.fetch")}
            </button>
            <button
              class="action"
              disabled={busy || !snap.upstream || snap.upstreamGone}
              title={t("git.actions.pullHint")}
              onclick={() => void gitStore.run({ kind: "pull" })}
            >
              {t("git.actions.pull")}{#if snap.behind > 0}<b>↓{snap.behind}</b>{/if}
            </button>
            <button
              class="action"
              disabled={busy || snap.detached || snap.remotes.length === 0}
              title={t(snap.upstream && !snap.upstreamGone ? "git.actions.pushHint" : "git.actions.publishHint")}
              onclick={() => void gitStore.run({ kind: "push" })}
            >
              {t(snap.upstream && !snap.upstreamGone ? "git.actions.push" : "git.actions.publish")}{#if snap.ahead > 0}<b>↑{snap.ahead}</b>{/if}
            </button>
            <div class="menu-anchor">
              <button
                class="action more"
                title={t("git.actions.more")}
                aria-label={t("git.actions.more")}
                aria-haspopup="menu"
                aria-expanded={menuOpen}
                onpointerdown={(event) => event.stopPropagation()}
                onclick={() => (menuOpen = !menuOpen)}
              >
                ⋯
              </button>
              {#if menuOpen}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div class="menu" role="menu" tabindex="-1" onpointerdown={(event) => event.stopPropagation()}>
                  <button role="menuitem" disabled={busy || snap.detached || snap.remotes.length === 0} onclick={() => runFromMenu(() => void gitStore.run({ kind: "sync" }))}>
                    {t("git.actions.sync")}
                  </button>
                  <button role="menuitem" disabled={busy} onclick={() => runFromMenu(() => (newBranch = ""))}>{t("git.actions.newBranch")}…</button>
                  <button role="menuitem" disabled={busy || counts.total === 0} onclick={() => runFromMenu(() => void gitStore.run({ kind: "stashPush" }))}>
                    {t("git.actions.stash")}
                  </button>
                  <button role="menuitem" disabled={busy || snap.stashCount === 0} onclick={() => runFromMenu(() => void gitStore.run({ kind: "stashPop" }))}>
                    {t("git.actions.stashPop")}{#if snap.stashCount > 0}&nbsp;({snap.stashCount}){/if}
                  </button>
                  <button role="menuitem" onclick={() => runFromMenu(() => root && workspaceStore.openGitGraph(root, t("git.graph.tab", { name: fileName(root) })))}>
                    {t("git.graph.open")}
                  </button>
                </div>
              {/if}
            </div>
          </div>
          {#if newBranch !== null}
            <form
              class="inline-form"
              onsubmit={(event) => {
                event.preventDefault();
                createBranch();
              }}
            >
              <input type="text" bind:value={newBranch} placeholder={t("git.actions.newBranchPlaceholder")} spellcheck="false" use:focusOnMount />
              <button type="submit" class="small" disabled={busy || !newBranch.trim()}>{t("git.actions.create")}</button>
              <button type="button" class="small" onclick={() => (newBranch = null)}>{t("git.cancel")}</button>
            </form>
          {/if}
        </section>

        <!-- What changed, and the commit that takes it. -->
        <section>
          {@render sectionHead("changes", t("git.changes.title"), counts.total)}
          {#if !collapsed.includes("changes")}
            {#if counts.total === 0}
              <p class="section-note">{t(snap.changesUnavailable ? "git.changes.unavailable" : "git.changes.clean")}</p>
            {/if}
            {#each CHANGE_GROUPS as group (group)}
              {@const files = groups[group]}
              {#if files.length > 0}
                <div class="group-head">
                  <span class="group-title">{t(GROUP_LABEL[group])}</span>
                  <span class="group-count">{files.length}</span>
                  <span class="group-actions">
                    {#if confirming === `all:${group}`}
                      <button class="small danger" disabled={busy} onclick={() => discard(files)}>{t("git.changes.confirmDiscard")}</button>
                      <button class="small" onclick={() => (confirming = null)}>{t("git.no")}</button>
                    {:else}
                      {#if group === "changes" || group === "untracked"}
                        <button class="row-button" disabled={busy} title={t("git.changes.discardAll")} aria-label={t("git.changes.discardAll")} onclick={() => (confirming = `all:${group}`)}>
                          <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 6.5a3.5 3.5 0 1 0 1-2.8M2.5 2v2h2" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
                        </button>
                      {/if}
                      {#if group === "staged"}
                        <button class="row-button" disabled={busy} title={t("git.changes.unstageAll")} aria-label={t("git.changes.unstageAll")} onclick={() => unstage(files)}>
                          <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 6h7" stroke="currentColor" stroke-width="1.4" /></svg>
                        </button>
                      {:else}
                        <button class="row-button" disabled={busy} title={t("git.changes.stageAll")} aria-label={t("git.changes.stageAll")} onclick={() => stage(files)}>
                          <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 2.5v7M2.5 6h7" stroke="currentColor" stroke-width="1.4" /></svg>
                        </button>
                      {/if}
                    {/if}
                  </span>
                </div>
                {#each files as file (file.path)}
                  {@const mark = markIn(file, group)}
                  {@const parts = splitPath(file.path)}
                  {@const key = `${group}:${file.path}`}
                  <div class="file-row" style="color:{markColor(mark, themeStore.current.colors)}">
                    <button
                      class="file-open"
                      title={`${file.path} — ${t(`explorer.marks.${mark}`)}${file.origPath ? ` (${file.origPath})` : ""}`}
                      onclick={() => root && workspaceStore.openDiff(diffOf(file, group))}
                    >
                      <span class="file-name">{parts.name}</span>
                      {#if parts.folder}
                        <span class="file-folder">{parts.folder}</span>
                      {/if}
                    </button>
                    {#if confirming === key}
                      <button class="small danger" disabled={busy} onclick={() => discard([file])}>{t("git.changes.confirmDiscard")}</button>
                      <button class="small" onclick={() => (confirming = null)}>{t("git.no")}</button>
                    {:else}
                      <span class="file-actions">
                        {#if group === "changes" || group === "untracked"}
                          <button class="row-button" disabled={busy} title={t("git.changes.discard")} aria-label={t("git.changes.discard")} onclick={() => (confirming = key)}>
                            <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 6.5a3.5 3.5 0 1 0 1-2.8M2.5 2v2h2" fill="none" stroke="currentColor" stroke-width="1.2" /></svg>
                          </button>
                        {/if}
                        {#if group === "staged"}
                          <button class="row-button" disabled={busy} title={t("git.changes.unstage")} aria-label={t("git.changes.unstage")} onclick={() => unstage([file])}>
                            <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 6h7" stroke="currentColor" stroke-width="1.4" /></svg>
                          </button>
                        {:else}
                          <button class="row-button" disabled={busy} title={t("git.changes.stage")} aria-label={t("git.changes.stage")} onclick={() => stage([file])}>
                            <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 2.5v7M2.5 6h7" stroke="currentColor" stroke-width="1.4" /></svg>
                          </button>
                        {/if}
                      </span>
                      <span class="file-mark">{MARK_LETTER[mark]}</span>
                    {/if}
                  </div>
                {/each}
              {/if}
            {/each}

            <div class="commit-box">
              <textarea
                rows="3"
                placeholder={t(amend ? "git.commit.amendPlaceholder" : "git.commit.placeholder")}
                value={message}
                bind:this={messageEl}
                oninput={(event) => root && gitStore.setDraft(root, event.currentTarget.value)}
                onkeydown={onMessageKey}
              ></textarea>
              <div class="commit-actions">
                <button class="small primary" disabled={!canCommit} title={t("git.commit.hint")} onclick={() => commit(false)}>
                  {t(amend ? "git.commit.amendAction" : commitsAll ? "git.commit.all" : "git.commit.staged")}
                </button>
                <button class="small" disabled={!canCommit || snap.detached || snap.remotes.length === 0} onclick={() => commit(true)}>
                  {t("git.commit.andPush")}
                </button>
                <label class="check" title={t("git.commit.amendHint")}>
                  <input type="checkbox" bind:checked={amend} disabled={!snap.head} />
                  <span>{t("git.commit.amend")}</span>
                </label>
              </div>
              {#if counts.conflicts > 0}
                <p class="section-note warn">{t("git.commit.conflicts")}</p>
              {/if}
            </div>
          {/if}
        </section>

        {#if appConfigStore.gitPrEnabled}
          <section>
            {@render sectionHead("pr", t("git.pr.section"), null)}
            {#if !collapsed.includes("pr")}
              <GitPrSection />
            {/if}
          </section>
        {/if}

        <section>
          {@render sectionHead("branches", t("git.branches.title"), localBranches.length)}
          {#if !collapsed.includes("branches")}
            {#each shownBranches as branch (branch.remote ? `r:${branch.name}` : branch.name)}
              <div class="branch-row" class:current={branch.current} class:remote={branch.remote}>
                <span class="branch-dot" aria-hidden="true"></span>
                <span class="branch-text" title={`${branch.name}\n${branch.sha} · ${branch.subject}`}>
                  <span class="branch-name">{branch.name}</span>
                  <span class="branch-sub">
                    {#if branch.gone}
                      {t("git.branches.gone")}
                    {:else if !branch.remote && !branch.upstream}
                      {t("git.branches.local")}
                    {:else}
                      {age(branch.lastCommitTime * 1000)}
                    {/if}
                  </span>
                </span>
                {#if branch.ahead > 0 || branch.behind > 0}
                  <span class="bars" title={t("git.branches.track", { ahead: String(branch.ahead), behind: String(branch.behind), upstream: branch.upstream ?? "" })}>
                    <span class="bar-pair">
                      <i class="bar behind" style="width:{Math.round((branch.behind / farthest) * 22)}px"></i>
                      <i class="bar ahead" style="width:{Math.round((branch.ahead / farthest) * 22)}px"></i>
                    </span>
                    <span class="bar-numbers">
                      {#if branch.behind > 0}↓{branch.behind}{/if}
                      {#if branch.ahead > 0}↑{branch.ahead}{/if}
                    </span>
                  </span>
                {/if}
                {#if !branch.current}
                  <button class="small switch" disabled={busy} title={t("git.branches.switchHint", { name: branch.name })} onclick={() => switchTo(branch)}>
                    {t("git.branches.switch")}
                  </button>
                {/if}
              </div>
            {/each}
            {#if remoteBranches.length > 0}
              <button class="link padded" onclick={() => (showRemoteBranches = !showRemoteBranches)}>
                {showRemoteBranches ? t("git.branches.hideRemote") : t("git.branches.showRemote", { n: String(remoteBranches.length) })}
              </button>
            {/if}
          {/if}
        </section>

        <section>
          {@render sectionHead("history", t("git.graph.title"), null)}
          {#if !collapsed.includes("history")}
            {#if graphRows.length === 0}
              <p class="section-note">{t("git.graph.empty")}</p>
            {:else}
              <CommitGraph rows={graphRows} sides={graphSides} compact {now} />
            {/if}
            <button class="link padded" onclick={() => root && workspaceStore.openGitGraph(root, t("git.graph.tab", { name: fileName(root) }))}>
              {t("git.graph.open")} →
            </button>
          {/if}
        </section>

        <section>
          {@render sectionHead("remotes", t("git.remotes.title"), snap.remotes.length)}
          {#if !collapsed.includes("remotes")}
            {#if snap.remotes.length === 0}
              <p class="section-note">{t("git.remotes.none")}</p>
            {/if}
            {#each snap.remotes as remote (remote.name)}
              <div class="remote-row">
                <ForgeIcon kind={remote.forge?.kind ?? "generic"} size={13} />
                <span class="remote-text">
                  <span class="remote-name">{remote.name}</span>
                  <span class="remote-url" title={remote.fetchUrl}>{remote.fetchUrl}</span>
                  {#if remote.pushUrl !== remote.fetchUrl}
                    <span class="remote-url" title={remote.pushUrl}>{t("git.remotes.push")}: {remote.pushUrl}</span>
                  {/if}
                </span>
                <button class="small" onclick={() => copy(remote.fetchUrl)}>{t(copied === remote.fetchUrl ? "git.copied" : "git.copy")}</button>
              </div>
            {/each}
          {/if}
        </section>
      </div>
    {/if}
  </div>
</aside>

<style>
  /* Docked at the window's left edge, like the explorer and the agents
     panel: the consoles make room for it. The width animates; the content
     inside keeps its own fixed width so nothing reflows while it slides. */
  .git-panel {
    position: relative;
    flex-shrink: 0;
    width: var(--panel-width, 264px);
    overflow: hidden;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .git-inner {
    width: var(--panel-width, 264px);
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .git-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 6px 0 10px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
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

  .git-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .empty-note,
  .section-note {
    margin: 0;
    padding: 12px;
    opacity: 0.6;
  }

  .section-note {
    padding: 4px 10px 8px;
    font-size: 11px;
  }

  .section-note.warn {
    padding: 4px 0 0;
    color: #d7a72a;
    opacity: 1;
  }

  section {
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .branch-block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px 10px;
  }

  .branch-line {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .branch-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    opacity: 0.8;
  }

  .branch-current {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .fetched {
    flex-shrink: 0;
    font-size: 10px;
    opacity: 0.5;
  }

  .notice {
    margin: 0;
    padding: 4px 7px;
    border-radius: 4px;
    background: rgba(128, 128, 128, 0.14);
    font-size: 11px;
  }

  .notice.warn {
    background: rgba(215, 167, 42, 0.16);
  }

  .running {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 7px;
    border-radius: 4px;
    background: rgba(58, 109, 240, 0.14);
  }

  .running-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .running-line {
    font-size: 10.5px;
    opacity: 0.65;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spinner {
    flex-shrink: 0;
    width: 11px;
    height: 11px;
    border: 2px solid var(--pane-active-border, #3a6df0);
    border-right-color: transparent;
    border-radius: 50%;
    animation: git-spin 0.8s linear infinite;
  }

  @keyframes git-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .error-box {
    display: flex;
    align-items: flex-start;
    gap: 4px;
    padding: 5px 2px 5px 7px;
    border-radius: 4px;
    background: rgba(224, 90, 90, 0.14);
  }

  .error-box pre,
  .output {
    flex: 1;
    min-width: 0;
    margin: 0;
    max-height: 140px;
    overflow: auto;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 10.5px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    scrollbar-width: thin;
    user-select: text;
  }

  .output {
    padding: 5px 7px;
    border-radius: 4px;
    background: rgba(128, 128, 128, 0.14);
  }

  .actions {
    display: flex;
    gap: 4px;
  }

  .action {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 4px 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    white-space: nowrap;
    cursor: pointer;
  }

  .action b {
    font-weight: 700;
    color: var(--pane-active-border, #3a6df0);
  }

  .action.more {
    flex: 0 0 26px;
    font-size: 14px;
    line-height: 1;
  }

  .action:hover:not(:disabled),
  .small:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .menu-anchor {
    position: relative;
    flex: 0 0 26px;
    display: flex;
  }

  .menu {
    position: absolute;
    top: calc(100% + 3px);
    right: 0;
    z-index: 30;
    min-width: 170px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: var(--tabbar-active-bg, #1e1e1e);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
  }

  .menu button {
    padding: 5px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }

  .menu button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .inline-form {
    display: flex;
    gap: 4px;
  }

  .inline-form input {
    flex: 1;
    min-width: 0;
  }

  input[type="text"],
  textarea {
    box-sizing: border-box;
    padding: 4px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    color: inherit;
    font: inherit;
    font-size: 12px;
  }

  input[type="text"]:focus,
  textarea:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .small {
    flex-shrink: 0;
    padding: 2px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .small.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .small.primary:hover:not(:disabled) {
    background: var(--pane-active-border, #3a6df0);
    filter: brightness(1.1);
  }

  .small.danger {
    border-color: #cf4a4a;
    color: #e05a5a;
  }

  .link {
    align-self: flex-start;
    padding: 0;
    border: none;
    background: none;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  .link.padded {
    display: block;
    padding: 4px 10px 8px;
  }

  .section-head {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .section-head:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .section-title {
    flex: 1;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.7;
  }

  .section-count,
  .group-count {
    min-width: 16px;
    padding: 0 4px;
    box-sizing: border-box;
    border-radius: 8px;
    background: rgba(128, 128, 128, 0.25);
    font-size: 10px;
    line-height: 15px;
    text-align: center;
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

  .group-head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 22px;
    padding: 2px 6px 2px 10px;
  }

  .group-title {
    font-size: 11px;
    font-weight: 600;
    opacity: 0.75;
  }

  .group-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .file-row {
    display: flex;
    align-items: center;
    gap: 4px;
    min-height: 22px;
    padding: 0 6px 0 10px;
  }

  .file-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .file-open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 3px 0;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .file-name {
    flex-shrink: 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The folder is context, not the change: in the panel's own color. */
  .file-folder {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 10.5px;
    opacity: 0.5;
  }

  .file-mark {
    flex-shrink: 0;
    width: 10px;
    font-size: 10px;
    font-weight: 700;
    text-align: center;
  }

  /* The row's buttons show when it's pointed at or has the keyboard. */
  .file-actions {
    display: none;
    align-items: center;
    gap: 2px;
  }

  .file-row:hover .file-actions,
  .file-row:focus-within .file-actions {
    display: flex;
  }

  .row-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    opacity: 0.75;
    cursor: pointer;
  }

  .row-button svg {
    width: 12px;
    height: 12px;
  }

  .row-button:hover:not(:disabled) {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.12));
  }

  .commit-box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px 10px 10px;
  }

  .commit-box textarea {
    width: 100%;
    min-height: 48px;
    resize: vertical;
  }

  .commit-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .commit-actions .small {
    padding: 3px 9px;
    font-size: 11.5px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    cursor: pointer;
  }

  .check input {
    accent-color: var(--pane-active-border, #3a6df0);
  }

  .branch-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 30px;
    padding: 0 8px 0 10px;
  }

  .branch-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .branch-dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border: 1.5px solid currentColor;
    border-radius: 50%;
    opacity: 0.45;
  }

  .branch-row.current .branch-dot {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    opacity: 1;
  }

  .branch-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .branch-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .branch-row.current .branch-name {
    font-weight: 700;
  }

  .branch-row.remote .branch-name {
    opacity: 0.8;
  }

  .branch-sub {
    font-size: 10px;
    opacity: 0.5;
  }

  /* Behind grows to the left of the middle, ahead to the right: at a glance,
     which way each branch leans. */
  .bars {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .bar-pair {
    display: grid;
    grid-template-columns: 22px 22px;
    gap: 1px;
    height: 4px;
  }

  .bar {
    height: 4px;
    min-width: 0;
    border-radius: 2px;
  }

  .bar.behind {
    justify-self: end;
    background: var(--git-behind, #d7a72a);
  }

  .bar.ahead {
    justify-self: start;
    background: var(--pane-active-border, #3a6df0);
  }

  .bar-numbers {
    font-size: 9.5px;
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }

  .switch {
    display: none;
  }

  .branch-row:hover .switch,
  .branch-row:focus-within .switch {
    display: block;
  }

  .remote-row {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 3px 8px 6px 10px;
  }

  .remote-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .remote-name {
    font-weight: 600;
  }

  .remote-url {
    font-size: 10.5px;
    opacity: 0.6;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
