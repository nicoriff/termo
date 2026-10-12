<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/i18n.svelte";
  import { renderMarkdownLite } from "$lib/agents/markdown-lite";
  import type { PrInfo, PrPatch } from "$lib/core/git-client";
  import { gitStore } from "./git-store.svelte";
  import { diffNames, parseNames } from "./git-summary";

  /** The pull request (or merge request) of the branch in hand: what it
   * says, changing it, and opening one when there's none. */
  const current = $derived(gitStore.pr);
  const snapshot = $derived(gitStore.snapshot);
  /** GitLab calls them merge requests. */
  const what = $derived(t(current && "cli" in current && current.cli === "glab" ? "git.pr.mergeRequest" : "git.pr.pullRequest"));

  let mode = $state<"view" | "edit" | "create">("view");
  let title = $state("");
  let body = $state("");
  let base = $state("");
  let draft = $state(false);
  let labels = $state("");
  let reviewers = $state("");
  let bodyOpen = $state(false);

  // Another branch, another request: whatever was being written was for the
  // one before.
  $effect(() => {
    void gitStore.branch;
    void gitStore.root;
    mode = "view";
    bodyOpen = false;
  });

  function startEdit(pr: PrInfo) {
    title = pr.title;
    body = pr.body;
    base = pr.base;
    draft = pr.state === "draft";
    labels = pr.labels.map((label) => label.name).join(", ");
    reviewers = pr.reviewers.join(", ");
    gitStore.prError = null;
    mode = "edit";
  }

  function startCreate() {
    title = snapshot?.head?.subject ?? snapshot?.branch ?? "";
    body = "";
    // `origin/main` is the base as git knows it; the service wants `main`.
    const remote = gitStore.remote?.name;
    const known = snapshot?.base?.name ?? "";
    base = remote && known.startsWith(`${remote}/`) ? known.slice(remote.length + 1) : known;
    draft = false;
    gitStore.prError = null;
    mode = "create";
  }

  /** Only what changed goes: nothing else of the request is touched. */
  async function save(pr: PrInfo) {
    const patch: PrPatch = {};
    if (title.trim() !== pr.title) patch.title = title.trim();
    if (body !== pr.body) patch.body = body;
    if (base.trim() && base.trim() !== pr.base) patch.base = base.trim();
    if (draft !== (pr.state === "draft")) patch.draft = draft;
    const labelChange = diffNames(pr.labels.map((label) => label.name), parseNames(labels));
    if (labelChange.add.length) patch.addLabels = labelChange.add;
    if (labelChange.remove.length) patch.removeLabels = labelChange.remove;
    const reviewerChange = diffNames(pr.reviewers, parseNames(reviewers));
    if (reviewerChange.add.length) patch.addReviewers = reviewerChange.add;
    if (reviewerChange.remove.length) patch.removeReviewers = reviewerChange.remove;
    if (Object.keys(patch).length === 0 || (await gitStore.updatePr(patch))) mode = "view";
  }

  async function create() {
    if (await gitStore.createPr({ title, body, base: base.trim() || null, draft })) mode = "view";
  }

  /** A label's own color behind its name, readable on either. */
  function labelStyle(color: string | null): string {
    return color ? `border-color:#${color}; background:#${color}33` : "";
  }

  function open(url: string | null | undefined) {
    if (url && /^https?:\/\//i.test(url)) void openUrl(url);
  }
</script>

{#snippet form(submit: () => void, submitLabel: string, withPeople: boolean, canDraft: boolean)}
  <form
    class="pr-form"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <label>
      <span>{t("git.pr.title")}</span>
      <input type="text" bind:value={title} spellcheck="false" />
    </label>
    <label>
      <span>{t("git.pr.description")}</span>
      <textarea rows="5" bind:value={body}></textarea>
    </label>
    <label>
      <span>{t("git.pr.base")}</span>
      <input type="text" bind:value={base} spellcheck="false" placeholder="main" />
    </label>
    {#if withPeople}
      <label>
        <span>{t("git.pr.labels")}</span>
        <input type="text" bind:value={labels} spellcheck="false" placeholder={t("git.pr.namesHint")} />
      </label>
      <label>
        <span>{t("git.pr.reviewers")}</span>
        <input type="text" bind:value={reviewers} spellcheck="false" placeholder={t("git.pr.namesHint")} />
      </label>
    {/if}
    {#if canDraft}
      <label class="check">
        <input type="checkbox" bind:checked={draft} />
        <span>{t("git.pr.draft")}</span>
      </label>
    {/if}
    {#if gitStore.prError}
      <p class="pr-error">{gitStore.prError}</p>
    {/if}
    <div class="pr-actions">
      <button type="submit" class="primary" disabled={gitStore.prBusy || !title.trim()}>
        {gitStore.prBusy ? t("git.working") : submitLabel}
      </button>
      <button type="button" disabled={gitStore.prBusy} onclick={() => (mode = "view")}>{t("git.cancel")}</button>
    </div>
  </form>
{/snippet}

<div class="pr">
  {#if !current}
    <p class="note">{t("git.pr.loading")}</p>
  {:else if current.kind === "unsupported"}
    <p class="note">{t("git.pr.unsupported")}</p>
    {#if gitStore.forge?.webUrl}
      <button class="link" onclick={() => open(gitStore.forge?.webUrl)}>{t("git.openInBrowser")} ↗</button>
    {/if}
  {:else if current.kind === "noCli"}
    <p class="note">{t("git.pr.noCli", { cli: current.cli, what })}</p>
    <button
      class="link"
      onclick={() => open(current.cli === "gh" ? "https://cli.github.com" : "https://gitlab.com/gitlab-org/cli")}
    >
      {t("git.pr.getCli", { cli: current.cli })} ↗
    </button>
  {:else if current.kind === "notSignedIn"}
    <p class="note">{t("git.pr.notSignedIn", { cli: current.cli })}</p>
    <code class="command">{current.cli} auth login</code>
  {:else if current.kind === "failed"}
    <p class="pr-error">{current.message}</p>
    <button class="link" disabled={gitStore.prBusy} onclick={() => void gitStore.loadPr(true)}>{t("git.retry")}</button>
  {:else if current.kind === "none"}
    {#if mode === "create"}
      {@render form(() => void create(), t("git.pr.create", { what }), false, true)}
    {:else if snapshot?.detached}
      <p class="note">{t("git.pr.detached", { what })}</p>
    {:else}
      <p class="note">{t("git.pr.none", { what })}</p>
      {#if !snapshot?.upstream || snapshot.upstreamGone}
        <!-- A request is between two branches on the service: this one has
             to be there first. -->
        <p class="note">{t("git.pr.publishFirst")}</p>
        <button class="small" disabled={gitStore.running !== null} onclick={() => void gitStore.run({ kind: "push" })}>
          {t("git.actions.publish")}
        </button>
      {:else}
        <button class="small" onclick={startCreate}>{t("git.pr.create", { what })}…</button>
      {/if}
    {/if}
  {:else}
    {@const pr = current.pr}
    {#if mode === "edit"}
      {@render form(() => void save(pr), t("git.save"), true, pr.state === "open" || pr.state === "draft")}
    {:else}
      <div class="pr-head">
        <span class="pr-state {pr.state}">{t(`git.pr.states.${pr.state}`)}</span>
        <button class="pr-number" title={t("git.openInBrowser")} onclick={() => open(pr.url)}>#{pr.number} ↗</button>
      </div>
      <p class="pr-title">{pr.title}</p>
      <p class="pr-meta">
        <span class="branch-name">{pr.base}</span> ← <span class="branch-name">{pr.head}</span>
        {#if pr.author}· @{pr.author}{/if}
      </p>
      <div class="pr-facts">
        {#if pr.reviewDecision}
          <span class="fact {pr.reviewDecision}">{t(`git.pr.review.${pr.reviewDecision}`)}</span>
        {/if}
        {#if pr.checks}
          <span
            class="fact"
            class:failed={pr.checks.failed > 0}
            class:pending={pr.checks.failed === 0 && pr.checks.pending > 0}
            class:passed={pr.checks.failed === 0 && pr.checks.pending === 0}
            title={t("git.pr.checksHint", {
              passed: String(pr.checks.passed),
              failed: String(pr.checks.failed),
              pending: String(pr.checks.pending),
            })}
          >
            {t("git.pr.checks")}
            {#if pr.checks.passed}✓{pr.checks.passed}{/if}
            {#if pr.checks.failed}✗{pr.checks.failed}{/if}
            {#if pr.checks.pending}…{pr.checks.pending}{/if}
          </span>
        {/if}
        {#if pr.mergeable === false}
          <span class="fact failed">{t("git.pr.conflicts")}</span>
        {/if}
        {#if pr.additions !== null && pr.deletions !== null}
          <span class="fact"><span class="plus">+{pr.additions}</span> <span class="minus">−{pr.deletions}</span></span>
        {/if}
        {#if pr.changedFiles !== null}
          <span class="fact">{t("git.pr.files", { n: String(pr.changedFiles) })}</span>
        {/if}
        {#if pr.comments > 0}
          <span class="fact">{t("git.pr.comments", { n: String(pr.comments) })}</span>
        {/if}
      </div>
      {#if pr.labels.length > 0}
        <div class="chips">
          {#each pr.labels as label (label.name)}
            <span class="chip" style={labelStyle(label.color)}>{label.name}</span>
          {/each}
        </div>
      {/if}
      {#if pr.reviewers.length > 0}
        <p class="pr-meta">{t("git.pr.reviewers")}: {pr.reviewers.map((name) => `@${name}`).join(", ")}</p>
      {/if}
      {#if pr.body.trim()}
        <button class="link" aria-expanded={bodyOpen} onclick={() => (bodyOpen = !bodyOpen)}>
          {t(bodyOpen ? "git.pr.hideDescription" : "git.pr.showDescription")}
        </button>
        {#if bodyOpen}
          <!-- Escaped before any markup is added: see markdown-lite.ts. -->
          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
          <div class="pr-body">{@html renderMarkdownLite(pr.body)}</div>
        {/if}
      {/if}
      {#if gitStore.prError}
        <p class="pr-error">{gitStore.prError}</p>
      {/if}
      <div class="pr-actions">
        <button class="small" onclick={() => startEdit(pr)}>{t("git.pr.edit")}</button>
        <button class="small" disabled={gitStore.prBusy} onclick={() => void gitStore.loadPr(true)}>
          {gitStore.prBusy ? t("git.working") : t("git.refresh")}
        </button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .pr {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 5px;
    padding: 2px 10px 8px;
  }

  .note,
  .pr-meta,
  .pr-title,
  .pr-error {
    margin: 0;
  }

  .note,
  .pr-meta {
    opacity: 0.65;
    font-size: 11px;
  }

  .pr-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pr-state {
    padding: 1px 7px;
    border-radius: 9px;
    font-size: 10px;
    font-weight: 700;
    color: #fff;
    background: #3fa04a;
  }

  .pr-state.draft {
    background: #6e7681;
  }

  .pr-state.merged {
    background: #8250df;
  }

  .pr-state.closed {
    background: #cf4a4a;
  }

  .pr-number {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    font-weight: 600;
    opacity: 0.8;
    cursor: pointer;
  }

  .pr-number:hover {
    opacity: 1;
    text-decoration: underline;
  }

  .pr-title {
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .branch-name {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 10.5px;
  }

  .pr-facts,
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .fact,
  .chip {
    padding: 0 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    font-size: 10.5px;
    line-height: 16px;
    white-space: nowrap;
  }

  .fact.passed,
  .fact.approved,
  .plus {
    color: #3fa04a;
  }

  .fact.failed,
  .fact.changesRequested,
  .minus {
    color: #e05a5a;
  }

  .fact.pending,
  .fact.reviewRequired {
    color: #d7a72a;
  }

  .pr-body {
    align-self: stretch;
    max-height: 220px;
    overflow-y: auto;
    padding: 6px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: 11.5px;
    scrollbar-width: thin;
  }

  .pr-body :global(code) {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
  }

  .pr-error {
    align-self: stretch;
    color: #e05a5a;
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .command {
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(128, 128, 128, 0.18);
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    user-select: text;
  }

  .pr-actions {
    display: flex;
    gap: 6px;
    margin-top: 2px;
  }

  .pr-form {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pr-form label {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .pr-form label > span {
    font-size: 10.5px;
    opacity: 0.65;
  }

  .pr-form label.check {
    flex-direction: row;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }

  .pr-form label.check > span {
    font-size: 11.5px;
    opacity: 1;
  }

  input[type="text"],
  textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 4px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    color: inherit;
    font: inherit;
    font-size: 12px;
    resize: vertical;
  }

  input[type="text"]:focus,
  textarea:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  input[type="checkbox"] {
    accent-color: var(--pane-active-border, #3a6df0);
  }

  button.small,
  .pr-form button {
    padding: 3px 9px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  button.small:hover:not(:disabled),
  .pr-form button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  button.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  button.primary:hover:not(:disabled) {
    background: var(--pane-active-border, #3a6df0);
    filter: brightness(1.1);
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  button.link {
    padding: 0;
    border: none;
    background: none;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  button.link:hover:not(:disabled) {
    text-decoration: underline;
  }
</style>
