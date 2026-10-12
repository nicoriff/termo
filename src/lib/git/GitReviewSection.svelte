<script lang="ts">
  import { ago, formatDuration } from "$lib/agents/agent-spec";
  import { renderMarkdownLite } from "$lib/agents/markdown-lite";
  import { ENGINE_NAMES } from "$lib/agents/orchestration-spec";
  import { latestThought, stepLine, toolSteps } from "$lib/agents/planning-progress";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import type { Review, ReviewFinding, ReviewScope } from "$lib/core/git-client";
  import { diffJump } from "$lib/editor/diff-jump.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { gitStore } from "./git-store.svelte";
  import { gitReviewStore } from "./git-review-store.svelte";
  import {
    countFindings,
    findingPlace,
    groupByFile,
    SCOPE_LABEL,
    SEVERITIES,
    SEVERITY_LABEL,
    shownFindings,
    splitFences,
  } from "./git-review";
  import { splitPath } from "./git-summary";

  /** The Git panel's reviews of the branch in view: asking for one, watching
   * it work, and what's done with each thing it found. */
  const snap = $derived(gitStore.snapshot);
  const reviews = $derived(gitReviewStore.reviews);
  const running = $derived(gitReviewStore.busy ? gitReviewStore.running : null);
  const pr = $derived(gitStore.pr?.kind === "ok" ? gitStore.pr.pr : null);

  let scope = $state<"changes" | "branch">(appConfigStore.gitReviewScope);
  /** The review being read; the newest when none was picked. */
  let pickedId = $state<string | null>(null);
  let expanded = $state<string[]>([]);
  let withDismissed = $state(false);
  /** The finding whose "fix" is waiting for an agent to be picked. */
  let pickingFixerFor = $state<string | null>(null);
  /** Findings ticked to leave on the pull request. */
  let ticked = $state<string[]>([]);
  /** The comment about to be published, shown first. */
  let preview = $state<string | null>(null);
  let noteDraft = $state<string | null>(null);
  let now = $state(Date.now());

  const review = $derived(reviews.find((candidate) => candidate.id === pickedId) ?? reviews[0] ?? null);
  const shown = $derived(review ? shownFindings(review.findings, appConfigStore.gitReviewMinSeverity, withDismissed) : []);
  const groups = $derived(groupByFile(shown));
  const counts = $derived(countFindings(review?.findings ?? []));
  /** Left out for being less serious than Preferences asks to see. */
  const hidden = $derived(review ? counts.open - shownFindings(review.findings, appConfigStore.gitReviewMinSeverity, false).length : 0);
  /** The branch moved on since: the review is of an earlier state of it. */
  const moved = $derived(!!review && review.scope.kind !== "commit" && !!snap?.head && review.head !== snap.head.sha);
  const publishable = $derived(shown.filter((finding) => ticked.includes(finding.id)));

  /** Why there's nothing to review in the scope picked, when there isn't. */
  const nothing = $derived.by(() => {
    if (!snap) return null;
    if (scope === "changes") return gitStore.counts.total === 0 ? t("git.review.nothingChanges") : null;
    if (snap.detached || !snap.base) return t("git.review.noBase");
    return snap.base.ahead === 0 ? t("git.review.nothingBranch", { base: snap.base.name }) : null;
  });

  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), running ? 1000 : 30_000);
    return () => clearInterval(timer);
  });

  // Another review is another set of findings: nothing ticked carries over.
  $effect(() => {
    void review?.id;
    ticked = [];
    preview = null;
    pickingFixerFor = null;
  });

  function scopeName(of: ReviewScope): string {
    return of.kind === "commit" ? t("git.review.scope.commit", { sha: of.sha.slice(0, 7) }) : t(SCOPE_LABEL[of.kind]);
  }

  function age(ms: number): string {
    const value = ago(ms, now);
    return t(value.key, value.params);
  }

  function start() {
    pickedId = null;
    void gitReviewStore.review({ kind: scope });
  }

  function toggle(id: string) {
    expanded = expanded.includes(id) ? expanded.filter((other) => other !== id) : [...expanded, id];
  }

  function tick(id: string) {
    ticked = ticked.includes(id) ? ticked.filter((other) => other !== id) : [...ticked, id];
    preview = null;
  }

  /** The file against what the change started from, at the finding's line. */
  function openInDiff(of: Review, finding: ReviewFinding) {
    const target = { root: of.root, path: finding.path, from: of.from, to: of.scope.kind === "commit" ? of.scope.sha : null };
    diffJump.to(target, finding.line);
    workspaceStore.openDiff(target);
  }

  function fix(of: Review, finding: ReviewFinding) {
    const fixer = gitReviewStore.fixer;
    if (fixer) void gitReviewStore.fix(of, finding, fixer.id);
    else pickingFixerFor = finding.id;
  }

  function fixWith(of: Review, finding: ReviewFinding, agentId: string) {
    if (!agentId) return;
    appConfigStore.setGitReview({ fixAgentId: agentId });
    pickingFixerFor = null;
    void gitReviewStore.fix(of, finding, agentId);
  }

  async function showPreview(of: Review) {
    preview = await gitReviewStore.comment(of, publishable.map((finding) => finding.id)).catch(() => null);
  }

  async function publish(of: Review) {
    const done = await gitReviewStore.publish(of, publishable.map((finding) => finding.id));
    if (done) {
      ticked = [];
      preview = null;
    }
  }

  /** A push is waiting on this: it mustn't sit below the fold. */
  function intoView(node: HTMLElement) {
    node.scrollIntoView({ block: "nearest" });
  }

  // What was being typed is of the repository it was typed in.
  $effect(() => {
    void gitStore.root;
    noteDraft = null;
  });

  function saveNote() {
    const draft = noteDraft;
    noteDraft = null;
    if (draft === null || draft === gitReviewStore.stored.note) return;
    void gitReviewStore.setNote(draft);
  }
</script>

<!-- An agent's text: its prose as the little Markdown the app knows, its
     fenced code as code. -->
{#snippet rich(text: string)}
  {#each splitFences(text) as part, index (index)}
    {#if part.code}
      <pre class="code">{part.text}</pre>
    {:else}
      <div class="prose">{@html renderMarkdownLite(part.text)}</div>
    {/if}
  {/each}
{/snippet}

<div class="review">
  {#if !appConfigStore.aiModeEnabled}
    <p class="note">{t("git.review.aiOff")}</p>
  {:else}
    {#if gitReviewStore.gate}
      {@const gate = gitReviewStore.gate}
      <div class="gate" role="alertdialog" aria-label={t("git.review.gate.title")} use:intoView>
        <p>
          {#if gate.kind === "findings"}
            {t(gate.count === 1 ? "git.review.gate.findingsOne" : "git.review.gate.findings", { n: String(gate.count) })}
          {:else}
            {t("git.review.gate.failed")}{gate.message ? ` ${gate.message}` : ""}
          {/if}
        </p>
        <div class="row">
          <button class="small" onclick={() => gitReviewStore.answerGate(false)}>{t("git.review.gate.stop")}</button>
          <button class="small danger" onclick={() => gitReviewStore.answerGate(true)}>{t("git.review.gate.go")}</button>
        </div>
      </div>
    {/if}

    {#if running}
      {@const thought = latestThought(running.steps, 200)}
      <div class="working" aria-live="polite">
        <div class="row">
          <span class="spinner" aria-hidden="true"></span>
          <span class="working-title">
            {running.scope ? t("git.review.working", { what: scopeName(running.scope).toLowerCase() }) : t("git.review.workingElsewhere")}
          </span>
          <span class="elapsed">{formatDuration(Math.max(0, now - running.startedAt))}</span>
          <button class="small" onclick={() => gitReviewStore.cancel()}>{t("git.review.cancel")}</button>
        </div>
        {#if thought}
          <p class="thought">{thought}</p>
        {/if}
        {#each toolSteps(running.steps, 4) as step, index (index)}
          {@const line = stepLine(step, t)}
          <p class="step"><span aria-hidden="true">{line.icon}</span> {line.text}</p>
        {/each}
      </div>
    {:else}
      <div class="row">
        <select bind:value={scope} aria-label={t("git.review.scopeLabel")}>
          <option value="changes">{t("git.review.scope.changes")}</option>
          <option value="branch">{t("git.review.scope.branch")}</option>
        </select>
        <button
          class="small primary"
          disabled={!!nothing || !snap || gitReviewStore.running !== null}
          title={nothing ?? t("git.review.runHint")}
          onclick={start}
        >
          {t("git.review.run")}
        </button>
      </div>
      {#if nothing}
        <p class="note">{nothing}</p>
      {/if}
    {/if}

    {#if gitReviewStore.error}
      <p class="error">{gitReviewStore.error}</p>
    {/if}
    {#if gitReviewStore.notice}
      <p class="notice">{t(gitReviewStore.notice.key, gitReviewStore.notice.params)}</p>
    {/if}

    {#if reviews.length > 1}
      <div class="chips" role="tablist" aria-label={t("git.review.kept")}>
        <span class="chips-label">{t("git.review.keptShort")}</span>
        {#each reviews as kept (kept.id)}
          <button class="chip" role="tab" aria-selected={kept.id === review?.id} class:current={kept.id === review?.id} onclick={() => (pickedId = kept.id)}>
            {scopeName(kept.scope)}
          </button>
        {/each}
      </div>
    {/if}

    {#if review}
      <p class="meta">
        {scopeName(review.scope)}{review.base ? ` ← ${review.base}` : ""} · {ENGINE_NAMES[review.engine]}{review.model ? ` (${review.model})` : ""}
        · {age(review.createdAt)}{review.costUsd !== null ? ` · $${review.costUsd.toFixed(2)}` : ""}
      </p>
      {#if moved}
        <p class="note warn">{t("git.review.moved")}</p>
      {/if}
      {#if review.partial}
        <p class="note warn">{t("git.review.partial")}</p>
      {/if}
      {#if review.summary}
        <div class="summary">{@render rich(review.summary)}</div>
      {/if}

      {#if counts.open > 0 || counts.dismissed > 0}
        <div class="chips">
          {#each SEVERITIES as severity (severity)}
            {#if counts[severity] > 0}
              <span class="count {severity}">{counts[severity]} {t(SEVERITY_LABEL[severity]).toLowerCase()}</span>
            {/if}
          {/each}
          {#if counts.dismissed > 0}
            <button class="link" onclick={() => (withDismissed = !withDismissed)}>
              {t(withDismissed ? "git.review.hideDismissed" : "git.review.showDismissed", { n: String(counts.dismissed) })}
            </button>
          {/if}
        </div>
      {/if}
      {#if review.findings.length === 0}
        <p class="note clean">{t("git.review.clean")}</p>
      {:else if shown.length === 0 && !withDismissed}
        <p class="note">{t(hidden > 0 ? "git.review.allHidden" : "git.review.allDismissed")}</p>
      {/if}
      {#if hidden > 0 && shown.length > 0}
        <p class="note">{t("git.review.hidden", { n: String(hidden) })}</p>
      {/if}

      {#each groups as group (group.path)}
        {@const parts = splitPath(group.path)}
        <div class="file">
          <p class="file-head" title={group.path}>
            <span class="file-name">{parts.name}</span>
            {#if parts.folder}<span class="file-folder">{parts.folder}</span>{/if}
          </p>
          {#each group.findings as finding (finding.id)}
            {@const open = expanded.includes(finding.id)}
            <div class="finding {finding.severity}" class:dismissed={finding.dismissed} class:stale={finding.stale}>
              <div class="finding-head">
                {#if pr && !finding.dismissed}
                  <input
                    type="checkbox"
                    checked={ticked.includes(finding.id)}
                    title={t("git.review.tick")}
                    aria-label={t("git.review.tick")}
                    onchange={() => tick(finding.id)}
                  />
                {/if}
                <button class="finding-title" aria-expanded={open} onclick={() => toggle(finding.id)}>
                  <span class="pill {finding.severity}">{t(SEVERITY_LABEL[finding.severity])}</span>
                  <span class="title-text">{finding.title}</span>
                </button>
              </div>
              <button class="place" title={t("git.review.openDiff")} onclick={() => openInDiff(review, finding)}>
                {findingPlace({ ...finding, path: parts.name })}
              </button>
              {#if finding.stale}
                <span class="tag">{t("git.review.stale")}</span>
              {/if}
              {#if open}
                <div class="body">{@render rich(finding.body)}</div>
                {#if finding.suggestion}
                  <div class="fix-hint">{@render rich(finding.suggestion)}</div>
                {/if}
                <div class="row actions">
                  {#if finding.dismissed}
                    <button class="small" onclick={() => gitReviewStore.dismiss(review, finding, false)}>{t("git.review.restore")}</button>
                  {:else}
                    <button class="small" disabled={gitReviewStore.acting} title={t("git.review.fixHint")} onclick={() => fix(review, finding)}>
                      {gitReviewStore.fixer ? t("git.review.fixWith", { agent: gitReviewStore.fixer.name }) : t("git.review.fix")}
                    </button>
                    {#if gitReviewStore.console}
                      <button
                        class="small"
                        title={t("git.review.sendHint")}
                        onclick={() => gitReviewStore.sendToConsole(finding, t("git.review.fixLead"))}
                      >
                        {t("git.review.send", { tool: TOOL_DISPLAY_NAMES[gitReviewStore.console.tool] })}
                      </button>
                    {/if}
                    <button class="small" onclick={() => gitReviewStore.dismiss(review, finding, true)}>{t("git.review.dismiss")}</button>
                  {/if}
                </div>
                {#if pickingFixerFor === finding.id}
                  {#if gitReviewStore.fixers.length === 0}
                    <p class="note">{t("git.review.noFixers")}</p>
                  {:else}
                    <select aria-label={t("git.review.pickFixer")} onchange={(event) => fixWith(review, finding, event.currentTarget.value)}>
                      <option value="">{t("git.review.pickFixer")}</option>
                      {#each gitReviewStore.fixers as agent (agent.id)}
                        <option value={agent.id}>{agent.icon} {agent.name}</option>
                      {/each}
                    </select>
                  {/if}
                {/if}
              {/if}
            </div>
          {/each}
        </div>
      {/each}

      {#if pr && publishable.length > 0}
        <div class="publish">
          {#if preview === null}
            <button class="small" disabled={gitReviewStore.acting} onclick={() => showPreview(review)}>
              {t("git.review.publish", { n: String(publishable.length), pr: String(pr.number) })}
            </button>
          {:else}
            <p class="note">{t("git.review.publishPreview", { pr: String(pr.number) })}</p>
            <pre class="preview">{preview}</pre>
            <div class="row">
              <button class="small" onclick={() => (preview = null)}>{t("git.no")}</button>
              <button class="small primary" disabled={gitReviewStore.acting} onclick={() => publish(review)}>{t("git.review.publishConfirm")}</button>
            </div>
          {/if}
        </div>
      {/if}

      <button class="link" onclick={() => gitReviewStore.clear(review)}>{t("git.review.clear")}</button>
    {:else if !running}
      <p class="note">{t("git.review.none")}</p>
    {/if}

    <details class="repo-note">
      <summary>{t("git.review.note")}</summary>
      <textarea
        rows="3"
        placeholder={t("git.review.notePlaceholder")}
        value={noteDraft ?? gitReviewStore.stored.note}
        oninput={(event) => (noteDraft = event.currentTarget.value)}
        onblur={saveNote}
      ></textarea>
    </details>
  {/if}
</div>

<style>
  .review {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 2px 10px 8px;
  }

  p {
    margin: 0;
  }

  .row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
  }

  .note,
  .meta {
    font-size: 11px;
    opacity: 0.65;
  }

  .note.warn {
    color: #d7a72a;
    opacity: 1;
  }

  .note.clean {
    color: #3fa04a;
    opacity: 1;
  }

  .error {
    align-self: stretch;
    color: #e05a5a;
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .notice {
    font-size: 11px;
    color: #3fa04a;
  }

  .gate {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 7px 8px;
    border: 1px solid #d7a72a;
    border-radius: 5px;
    background: rgba(215, 167, 42, 0.12);
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }

  .working {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .working-title {
    flex: 1;
    min-width: 0;
    font-weight: 600;
  }

  .elapsed {
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    opacity: 0.65;
  }

  .thought,
  .step {
    font-size: 11px;
    opacity: 0.7;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .thought {
    white-space: normal;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    font-style: italic;
  }

  .spinner {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border: 1.6px solid var(--pane-active-border, #3a6df0);
    border-right-color: transparent;
    border-radius: 50%;
    animation: review-spin 0.8s linear infinite;
  }

  @keyframes review-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }

  .chip,
  .count {
    padding: 0 7px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 10.5px;
    line-height: 16px;
    white-space: nowrap;
  }

  .chip {
    cursor: pointer;
    opacity: 0.7;
  }

  .chip.current {
    opacity: 1;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .serious {
    --severity: #e05a5a;
  }

  .warning {
    --severity: #d7a72a;
  }

  .suggestion {
    --severity: #5b8def;
  }

  .count {
    color: var(--severity);
  }

  .summary,
  .body,
  .fix-hint {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    font-size: 11.5px;
    user-select: text;
  }

  .prose {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .prose :global(code),
  .code {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
  }

  .code {
    margin: 0;
    padding: 5px 7px;
    overflow-x: auto;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.22);
    white-space: pre;
    scrollbar-width: thin;
  }

  .chips-label {
    font-size: 10.5px;
    opacity: 0.6;
  }

  .file {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .file-head {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin-top: 4px;
    font-size: 11.5px;
    white-space: nowrap;
    overflow: hidden;
  }

  .file-name {
    font-weight: 600;
  }

  .file-folder {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 10.5px;
    opacity: 0.55;
  }

  .finding {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 3px 6px;
    padding: 4px 6px 5px 8px;
    border-left: 2px solid var(--severity);
    border-radius: 0 4px 4px 0;
    background: rgba(128, 128, 128, 0.08);
    min-width: 0;
  }

  .finding.dismissed,
  .finding.stale {
    opacity: 0.55;
  }

  .finding-head {
    flex-basis: 100%;
    display: flex;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
  }

  .finding-title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    text-align: left;
    cursor: pointer;
  }

  .title-text {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .finding.dismissed .title-text {
    text-decoration: line-through;
  }

  .pill {
    flex-shrink: 0;
    padding: 0 5px;
    border-radius: 7px;
    background: var(--severity);
    color: #fff;
    font-size: 9.5px;
    font-weight: 700;
    line-height: 14px;
  }

  .place {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 10.5px;
    opacity: 0.7;
    cursor: pointer;
  }

  .place:hover,
  .link:hover {
    opacity: 1;
    text-decoration: underline;
  }

  .tag {
    font-size: 10px;
    opacity: 0.8;
  }

  .body,
  .fix-hint,
  .actions,
  .finding select,
  .finding .note {
    flex-basis: 100%;
  }

  .fix-hint {
    padding: 4px 6px;
    border-radius: 4px;
    background: rgba(128, 128, 128, 0.14);
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 10.5px;
    opacity: 0.6;
    cursor: pointer;
  }

  .publish {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 5px;
  }

  .preview {
    align-self: stretch;
    max-height: 180px;
    margin: 0;
    padding: 6px 8px;
    overflow: auto;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 10.5px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
    scrollbar-width: thin;
  }

  .repo-note {
    align-self: stretch;
    font-size: 11px;
  }

  .repo-note summary {
    opacity: 0.65;
    cursor: pointer;
  }

  select,
  textarea {
    box-sizing: border-box;
    padding: 3px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    color: inherit;
    font: inherit;
    font-size: 11.5px;
  }

  textarea {
    width: 100%;
    margin-top: 4px;
    resize: vertical;
  }

  select:focus,
  textarea:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  input[type="checkbox"] {
    flex-shrink: 0;
    margin: 2px 0 0;
    accent-color: var(--pane-active-border, #3a6df0);
  }

  button.small {
    padding: 3px 9px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  button.small:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  button.small:disabled {
    opacity: 0.45;
    cursor: default;
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

  button.danger {
    border-color: #cf4a4a;
    color: #e05a5a;
  }
</style>
