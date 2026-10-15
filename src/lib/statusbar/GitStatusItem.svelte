<script lang="ts">
  import { menuKeys } from "$lib/core/menu-keys";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { gitBranches, type Branch, type GitOp } from "$lib/core/git-client";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { gitStore } from "$lib/git/git-store.svelte";
  import { gitReviewStore } from "$lib/git/git-review-store.svelte";
  import { changesLabel, syncLabel } from "$lib/git/git-summary";
  import ForgeIcon from "$lib/git/ForgeIcon.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { fileName } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";

  /** The repository of the tab in focus, on the status bar: its branch, what
   * there is to push and pull, what changed and its pull request — each one
   * as Preferences says — and, on a click, what to do with it. */
  const snap = $derived(gitStore.snapshot);
  const parts = $derived(appConfigStore.gitStatusParts);
  const sync = $derived(snap ? syncLabel(snap.ahead, snap.behind) : "");
  const changes = $derived(changesLabel(gitStore.counts));
  const pr = $derived(gitStore.pr?.kind === "ok" ? gitStore.pr.pr : null);
  const busy = $derived(gitStore.running !== null);
  const tracked = $derived(!!snap?.upstream && !snap.upstreamGone);
  const hasRemote = $derived((snap?.remotes.length ?? 0) > 0);

  let open = $state(false);
  let mode = $state<"main" | "branches" | "newBranch">("main");
  let branches = $state<Branch[]>([]);
  let filter = $state("");
  let newBranch = $state("");

  const shownBranches = $derived(
    branches.filter((branch) => !branch.current && branch.name.toLowerCase().includes(filter.trim().toLowerCase())).slice(0, 40),
  );

  const hint = $derived.by(() => {
    if (!snap) return "";
    const lines = [snap.root, snap.branch ?? t("git.detached", { sha: snap.head?.sha.slice(0, 7) ?? "" })];
    if (snap.upstream) lines.push(`${snap.upstream}: ${sync || t("git.statusBar.level")}`);
    if (gitStore.counts.total > 0) {
      lines.push(
        t("git.statusBar.changesHint", {
          staged: String(gitStore.counts.staged),
          changed: String(gitStore.counts.changed),
          untracked: String(gitStore.counts.untracked),
          conflicts: String(gitStore.counts.conflicts),
        }),
      );
    }
    return lines.join("\n");
  });

  function close(refocus = true) {
    open = false;
    mode = "main";
    filter = "";
    newBranch = "";
    if (refocus) uiState.refocusTerminals();
  }

  function toggle() {
    if (open) close();
    else open = true;
  }

  function run(op: GitOp) {
    close();
    void gitStore.run(op).then((done) => {
      // Why it failed is in the panel, which has room to say it.
      if (!done) gitStore.setOpen(true);
    });
  }

  function showBranches() {
    mode = "branches";
    if (snap) void gitBranches(snap.root).then((list) => (branches = list));
  }

  function switchTo(branch: Branch) {
    // A remote's branch that's already here under its own name: that one.
    const short = branch.remote ? branch.name.slice(branch.name.indexOf("/") + 1) : branch.name;
    const local = branches.some((candidate) => !candidate.remote && candidate.name === short);
    run(branch.remote && !local ? { kind: "checkout", branch: branch.name, remote: true } : { kind: "checkout", branch: short, remote: false });
  }

  function closeAnd(action: () => void) {
    close(false);
    action();
  }

  function browse(url: string | null | undefined) {
    close();
    if (url && /^https?:\/\//i.test(url)) void openUrl(url);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    if (mode === "main") close();
    else mode = "main";
  }

  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onpointerdown={() => open && close(false)} />

{#if appConfigStore.gitEnabled && appConfigStore.gitStatusBar && snap}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="git-anchor" onpointerdown={(event) => event.stopPropagation()} onkeydown={onKeydown}>
    <button class="git-item" class:open title={hint} aria-haspopup="menu" aria-expanded={open} onclick={toggle}>
      {#if busy}
        <span class="spinner" aria-label={t("git.working")}></span>
      {:else}
        <svg class="branch-icon" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M5 3.8v8.4M5 8.2c4.4 0 6-.6 6-3.4" fill="none" stroke="currentColor" stroke-width="1.4" />
          <circle cx="5" cy="3" r="1.6" fill="none" stroke="currentColor" stroke-width="1.4" />
          <circle cx="5" cy="13" r="1.6" fill="none" stroke="currentColor" stroke-width="1.4" />
          <circle cx="11" cy="4" r="1.6" fill="none" stroke="currentColor" stroke-width="1.4" />
        </svg>
      {/if}
      <!-- On one baseline: the tags are set smaller than the text beside them. -->
      <span class="texts">
        {#if parts.branch}
          <span class="branch">{snap.branch ?? snap.head?.sha.slice(0, 7) ?? "HEAD"}</span>
        {/if}
        {#if parts.sync && sync}
          <span class="sync">{sync}</span>
        {/if}
        {#if parts.changes && changes}
          <span class="changes" class:conflicts={gitStore.counts.conflicts > 0}>{changes}</span>
        {/if}
        {#if parts.pr && pr}
          <span class="pr {pr.state}">#{pr.number}</span>
        {/if}
        {#if snap.state}
          <span class="state">{t(`git.statesShort.${snap.state}`)}</span>
        {/if}
      </span>
    </button>

    {#if open}
      <div class="git-menu" role="menu" tabindex="-1" use:menuKeys={{ close, escape: false }}>
        <div class="menu-head">
          <ForgeIcon kind={gitStore.forge?.kind ?? "generic"} size={13} />
          <span class="menu-repo">{gitStore.forge ? `${gitStore.forge.owner}/${gitStore.forge.repo}` : fileName(snap.root)}</span>
        </div>
        {#if mode === "branches"}
          <input type="text" bind:value={filter} placeholder={t("git.statusBar.filterBranches")} spellcheck="false" use:focusOnMount />
          <div class="branch-list">
            {#each shownBranches as branch (branch.remote ? `r:${branch.name}` : branch.name)}
              <button role="menuitem" class:remote={branch.remote} disabled={busy} onclick={() => switchTo(branch)}>{branch.name}</button>
            {:else}
              <p class="menu-note">{t("git.statusBar.noBranches")}</p>
            {/each}
          </div>
          <button role="menuitem" class="back" onclick={() => (mode = "main")}>← {t("git.back")}</button>
        {:else if mode === "newBranch"}
          <form
            onsubmit={(event) => {
              event.preventDefault();
              if (newBranch.trim()) run({ kind: "createBranch", name: newBranch.trim() });
            }}
          >
            <input type="text" bind:value={newBranch} placeholder={t("git.actions.newBranchPlaceholder")} spellcheck="false" use:focusOnMount />
            <button type="submit" role="menuitem" disabled={busy || !newBranch.trim()}>{t("git.actions.create")}</button>
          </form>
          <button role="menuitem" class="back" onclick={() => (mode = "main")}>← {t("git.back")}</button>
        {:else}
          <button role="menuitem" onclick={() => closeAnd(() => gitStore.focusCommit())}>
            {t("git.actions.commit")}…{#if changes}<span class="aside">{changes}</span>{/if}
          </button>
          <button role="menuitem" disabled={busy || snap.detached || !hasRemote} onclick={() => run({ kind: "push" })}>
            {t(tracked ? "git.actions.push" : "git.actions.publish")}{#if snap.ahead > 0}<span class="aside">↑{snap.ahead}</span>{/if}
          </button>
          <button role="menuitem" disabled={busy || !tracked} onclick={() => run({ kind: "pull" })}>
            {t("git.actions.pull")}{#if snap.behind > 0}<span class="aside">↓{snap.behind}</span>{/if}
          </button>
          <button role="menuitem" disabled={busy || !hasRemote} onclick={() => run({ kind: "fetch" })}>{t("git.actions.fetch")}</button>
          <button role="menuitem" disabled={busy || snap.detached || !hasRemote} onclick={() => run({ kind: "sync" })}>{t("git.actions.sync")}</button>
          <hr />
          <button role="menuitem" disabled={busy} onclick={showBranches}>{t("git.actions.switchBranch")}…</button>
          <button role="menuitem" disabled={busy} onclick={() => (mode = "newBranch")}>{t("git.actions.newBranch")}…</button>
          <button role="menuitem" disabled={busy || gitStore.counts.total === 0} onclick={() => run({ kind: "stashPush" })}>{t("git.actions.stash")}</button>
          <button role="menuitem" disabled={busy || snap.stashCount === 0} onclick={() => run({ kind: "stashPop" })}>
            {t("git.actions.stashPop")}{#if snap.stashCount > 0}<span class="aside">{snap.stashCount}</span>{/if}
          </button>
          {#if gitReviewStore.available}
            <hr />
            <button
              role="menuitem"
              disabled={gitReviewStore.busy || gitStore.counts.total === 0}
              onclick={() => closeAnd(() => gitReviewStore.ask({ kind: "changes" }))}
            >
              {t("git.review.reviewChanges")}
            </button>
            <button
              role="menuitem"
              disabled={gitReviewStore.busy || snap.detached || !snap.base || snap.base.ahead === 0}
              onclick={() => closeAnd(() => gitReviewStore.ask({ kind: "branch" }))}
            >
              {t("git.review.reviewBranch")}
            </button>
          {/if}
          <hr />
          {#if pr}
            <button role="menuitem" onclick={() => browse(pr.url)}>{t("git.statusBar.openPr", { n: String(pr.number) })} ↗</button>
          {/if}
          {#if gitStore.forge?.webUrl}
            <button role="menuitem" onclick={() => browse(gitStore.forge?.webUrl)}>{t("git.openInBrowser")} ↗</button>
          {/if}
          <button role="menuitem" onclick={() => closeAnd(() => workspaceStore.openGitGraph(snap.root, t("git.graph.tab", { name: fileName(snap.root) })))}>
            {t("git.graph.open")}
          </button>
          <button role="menuitem" onclick={() => closeAnd(() => gitStore.toggle())}>{t(gitStore.open ? "git.statusBar.hidePanel" : "git.statusBar.showPanel")}</button>
        {/if}
      </div>
    {/if}
  </span>
{/if}

<style>
  .git-anchor {
    position: relative;
    margin-left: 12px;
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .git-item {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    height: 18px;
    padding: 0 6px;
    border: none;
    border-radius: 3px;
    background: none;
    color: inherit;
    font: inherit;
    opacity: 0.85;
    white-space: nowrap;
    cursor: pointer;
  }

  .git-item:hover,
  .git-item.open {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.1));
  }

  .branch-icon {
    width: 11px;
    height: 11px;
    flex-shrink: 0;
  }

  .texts {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }

  .texts:empty {
    display: none;
  }

  .branch {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 600;
  }

  .sync {
    color: var(--pane-active-border, #3a6df0);
    font-weight: 600;
  }

  .changes {
    color: #d7a72a;
  }

  .changes.conflicts {
    color: #e05a5a;
  }

  .pr {
    padding: 0 5px;
    border-radius: 7px;
    background: #3fa04a;
    color: #fff;
    font-size: 10px;
    line-height: 14px;
  }

  .pr.draft {
    background: #6e7681;
  }

  .pr.merged {
    background: #8250df;
  }

  .pr.closed {
    background: #cf4a4a;
  }

  .state {
    padding: 0 5px;
    border-radius: 7px;
    background: rgba(215, 167, 42, 0.25);
    font-size: 10px;
    line-height: 14px;
  }

  .spinner {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border: 1.6px solid var(--pane-active-border, #3a6df0);
    border-right-color: transparent;
    border-radius: 50%;
    animation: git-item-spin 0.8s linear infinite;
  }

  @keyframes git-item-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Opens upward: the bar is the window's bottom edge. From the item's
     right, which is at the bar's right end. */
  .git-menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 5px);
    z-index: 60;
    width: 230px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 7px;
    background: var(--tabbar-active-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    box-shadow: 0 -6px 22px rgba(0, 0, 0, 0.38);
  }

  .menu-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px 6px;
    opacity: 0.75;
  }

  .menu-repo {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
  }

  .git-menu button {
    display: flex;
    align-items: center;
    gap: 8px;
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

  .git-menu button:hover:not(:disabled),
  .git-menu button:focus-visible {
    outline: none;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .git-menu button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .git-menu button.remote {
    opacity: 0.75;
  }

  .git-menu button.back {
    opacity: 0.7;
  }

  .aside {
    margin-left: auto;
    font-size: 10.5px;
    opacity: 0.7;
  }

  hr {
    margin: 4px 6px;
    border: none;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .git-menu form {
    display: flex;
    flex-direction: column;
  }

  .git-menu input {
    margin: 0 4px 4px;
    padding: 4px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    color: inherit;
    font: inherit;
  }

  .git-menu input:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .branch-list {
    display: flex;
    flex-direction: column;
    max-height: 240px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .branch-list button {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .menu-note {
    margin: 0;
    padding: 6px 8px;
    opacity: 0.6;
  }
</style>
