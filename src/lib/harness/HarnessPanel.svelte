<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t } from "$lib/i18n/i18n.svelte";
  import SidebarResizeHandle from "$lib/app/SidebarResizeHandle.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { harnessDetectChecks, type HarnessFileChange, type ProposedRule } from "$lib/core/harness-client";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { fileName } from "$lib/layout/split-tree";
  import { formatDuration } from "$lib/agents/agent-spec";
  import AiEnginePicker from "$lib/agents/AiEnginePicker.svelte";
  import { aiEngine } from "$lib/agents/ai-engine.svelte";
  import { latestThought, stepLine, toolSteps } from "$lib/agents/planning-progress";
  import HarnessDiff from "./HarnessDiff.svelte";
  import HarnessFileEditor from "./HarnessFileEditor.svelte";
  import HarnessRuleSection from "./HarnessRuleSection.svelte";
  import {
    AGENTS,
    CHANGE_KIND_LABEL,
    COMMAND_PACKS,
    NEEDS_TRUST,
    addPack,
    changeKind,
    groupExternal,
    hasPack,
    hasProposed,
    matchingRule,
    testCommand,
  } from "./harness-model";
  import { HARNESS_WIDTH, harnessStore } from "./harness-store.svelte";

  const COLLAPSED_KEY = "termo.harness.collapsed";

  /** The sections below the two lists, folded until they're wanted. */
  type SectionId = "assist" | "agents" | "tester" | "files" | "external";
  const FOLDED: SectionId[] = ["assist", "agents", "tester", "files", "external"];

  function loadCollapsed(): SectionId[] {
    try {
      const saved = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "null");
      return Array.isArray(saved) ? saved : FOLDED;
    } catch {
      return FOLDED;
    }
  }

  const root = $derived(harnessStore.root);
  const draft = $derived(harnessStore.draft);
  const review = $derived(harnessStore.review);
  const busy = $derived(harnessStore.busy);
  const repoName = $derived(root ? fileName(root) : t("harness.title"));
  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleHarness"), isMac));
  const pendingCount = $derived(harnessStore.saved?.pending.length ?? 0);
  const assist = $derived(harnessStore.assist);
  const proposals = $derived(harnessStore.proposals);
  const unified = $derived(harnessStore.unified);
  let now = $state(Date.now());

  // The time an agent has been at it, by the second while one is.
  $effect(() => {
    if (!assist) return;
    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });

  const ACCESS_LABEL = { noEdit: "harness.paths.noEdit", noRead: "harness.paths.noRead" } as const;
  const DECISION_LABEL = {
    deny: "harness.commands.deny",
    ask: "harness.commands.ask",
    allow: "harness.commands.allow",
  } as const;

  /** What a proposal is, in a word or two. */
  function proposalLabel(rule: ProposedRule): string {
    if (rule.kind === "path") return t(ACCESS_LABEL[rule.access]);
    if (rule.kind === "command") return t(DECISION_LABEL[rule.decision]);
    return t("harness.assist.check");
  }

  let testInput = $state("");
  /** Checks the repository's files suggest, for the root they were read in. */
  let detected = $state<{ root: string; checks: string[] } | null>(null);
  const suggestedChecks = $derived(
    detected && detected.root === root && draft
      ? detected.checks.filter((command) => !draft.checks.some((check) => check.command.trim() === command))
      : [],
  );
  const verdicts = $derived(draft && testInput.trim() ? testCommand(draft, testInput) : []);
  const testRule = $derived(draft && testInput.trim() ? matchingRule(draft, testInput) : null);

  const VERDICT_LABEL = {
    deny: "harness.tester.deny",
    ask: "harness.tester.ask",
    allow: "harness.tester.allow",
    text: "harness.tester.text",
    none: "harness.tester.none",
  } as const;

  // What the repository suggests as checks, read again for each repository.
  $effect(() => {
    const current = root;
    if (!current || detected?.root === current) return;
    void harnessDetectChecks(current)
      .then((checks) => {
        if (root === current) detected = { root: current, checks };
      })
      .catch(() => {});
  });

  const externalGroups = $derived(groupExternal(harnessStore.external));
  const instructionFiles = $derived(harnessStore.saved?.instructionFiles ?? []);
  const targeted = $derived(draft ? AGENTS.filter((agent) => draft.agents.includes(agent)) : []);
  const trusting = $derived(targeted.filter((agent) => NEEDS_TRUST.includes(agent)));

  let collapsed = $state<SectionId[]>(loadCollapsed());
  /** The files on review shown open, by path. */
  let expanded = $state<string[]>([]);

  function toggleSection(id: SectionId) {
    collapsed = collapsed.includes(id) ? collapsed.filter((other) => other !== id) : [...collapsed, id];
    try {
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify(collapsed));
    } catch {
      // Only a convenience.
    }
  }

  function close() {
    harnessStore.setOpen(false);
    uiState.refocusTerminals();
  }

  function toggleAgent(agent: (typeof AGENTS)[number], on: boolean) {
    harnessStore.edit((spec) => {
      spec.agents = on ? [...spec.agents.filter((other) => other !== agent), agent] : spec.agents.filter((other) => other !== agent);
    });
  }

  function toggleFile(path: string) {
    expanded = expanded.includes(path) ? expanded.filter((other) => other !== path) : [...expanded, path];
  }

  async function startReview() {
    expanded = [];
    await harnessStore.startReview();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    // Escape inside an editor is the editor's.
    if ((event.target as HTMLElement | null)?.closest(".cm-editor")) return;
    if (review !== null) harnessStore.cancelReview();
    else if (harnessStore.editingFile !== null) harnessStore.editFile(null);
    else uiState.refocusTerminals();
  }
</script>

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

{#snippet changeList(changes: HarnessFileChange[])}
  <ul class="review-list">
    {#each changes as change (change.path)}
      {@const kind = changeKind(change)}
      <li>
        <button class="review-file" aria-expanded={expanded.includes(change.path)} onclick={() => toggleFile(change.path)}>
          <svg class="chevron" class:open={expanded.includes(change.path)} viewBox="0 0 6 10" aria-hidden="true">
            <path d="M1 1l4 4-4 4" fill="none" stroke="currentColor" />
          </svg>
          <span class="review-path">{change.path}</span>
          <span class="review-kind {kind}">{t(CHANGE_KIND_LABEL[kind])}</span>
        </button>
        {#if expanded.includes(change.path)}
          <div class="review-diff">
            <HarnessDiff path={change.path} before={change.before} after={change.after} />
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}


<aside
  class="harness-panel"
  style="--panel-width:{harnessStore.width}px"
  transition:slide={{ axis: "x", duration: 180, easing: cubicOut }}
  aria-label={t("harness.title")}
>
  <SidebarResizeHandle sizer={harnessStore} limits={HARNESS_WIDTH} />
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="harness-inner" onkeydown={onKeydown}>
    <header class="harness-header">
      <svg class="header-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
        <path d="M8.6 12.2l2.4 2.4 4.4-4.6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      <div class="header-text">
        <span class="header-name" title={root ?? ""}>{repoName}</span>
        <span class="header-note">{t("harness.title")}</span>
      </div>
      <button class="icon-button" title={t("harness.refresh")} aria-label={t("harness.refresh")} onclick={() => void harnessStore.reload()}>
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M13 8a5 5 0 1 1-1.5-3.55M13 2.5v2.5h-2.5" fill="none" stroke="currentColor" stroke-width="1.2" />
        </svg>
      </button>
      <button class="icon-button" title={`${t("harness.close")} (${shortcut})`} aria-label={t("harness.close")} onclick={close}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1L1 9" stroke="currentColor" stroke-width="1.1" /></svg>
      </button>
    </header>

    {#if !root}
      <p class="empty-note">{t(!harnessStore.loaded ? "harness.loading" : "harness.notRepo")}</p>
    {:else if !draft}
      {#if harnessStore.error}
        <div class="error-box"><pre>{harnessStore.error}</pre></div>
      {:else}
        <p class="empty-note">{t("harness.loading")}</p>
      {/if}
    {:else if harnessStore.editingFile !== null}
      <div class="harness-body">
        {#key harnessStore.editingFile}
          <HarnessFileEditor path={harnessStore.editingFile} />
        {/key}
      </div>
    {:else if unified && unified.root === root}
      <!-- The instruction files as an agent unified them, before they're written. -->
      <div class="harness-body">
        <div class="review-head">
          <button class="link" onclick={() => harnessStore.closeUnified()}>← {t("harness.review.back")}</button>
          <span class="review-title">{t("harness.assist.unifyTitle")}</span>
        </div>
        {#if unified.summary}
          <p class="section-note summary">{unified.summary}</p>
        {/if}
        {#if unified.changes.length === 0}
          <p class="section-note">{t("harness.assist.unifyNothing")}</p>
        {:else}
          <p class="section-note">{t("harness.assist.unifyIntro", { n: String(unified.changes.length) })}</p>
          {@render changeList(unified.changes)}
        {/if}
        {#if harnessStore.assistError}
          <div class="error-box padded"><pre>{harnessStore.assistError}</pre></div>
        {/if}
        <div class="review-actions">
          <button class="small" onclick={() => harnessStore.closeUnified()}>{t("harness.assist.discard")}</button>
          <button class="small primary" disabled={busy || unified.changes.length === 0} onclick={() => void harnessStore.applyUnified()}>
            {busy ? t("harness.review.applying") : t("harness.review.apply")}
          </button>
        </div>
      </div>
    {:else if review !== null}
      <!-- What applying writes, file by file, before it's written. -->
      <div class="harness-body">
        <div class="review-head">
          <button class="link" onclick={() => harnessStore.cancelReview()}>← {t("harness.review.back")}</button>
          <span class="review-title">{t("harness.review.title")}</span>
        </div>
        {#if review.length === 0}
          <p class="section-note">{t("harness.review.nothing")}</p>
        {:else}
          <p class="section-note">{t("harness.review.intro", { n: String(review.length) })}</p>
          {@render changeList(review)}
        {/if}
        {#if harnessStore.error}
          <div class="error-box padded"><pre>{harnessStore.error}</pre></div>
        {/if}
        <div class="review-actions">
          <button class="small" onclick={() => harnessStore.cancelReview()}>{t("harness.review.back")}</button>
          <button class="small primary" disabled={busy || review.length === 0} onclick={() => void harnessStore.apply()}>
            {busy ? t("harness.review.applying") : t("harness.review.apply")}
          </button>
        </div>
      </div>
    {:else}
      <div class="harness-body">
        <!-- Where the rules stand against the files the agents read. -->
        <section class="status-block">
          <div class="status-line">
            <span class="status-dot {harnessStore.status}" aria-hidden="true"></span>
            <span class="status-text">
              {#if harnessStore.status === "draft"}
                {t("harness.status.draft")}
              {:else if harnessStore.status === "pending"}
                {t("harness.status.pending", { n: String(pendingCount) })}
              {:else if harnessStore.status === "synced"}
                {t("harness.status.synced")}
              {:else}
                {t("harness.status.none")}
              {/if}
            </span>
          </div>
          <p class="status-hint">{t("harness.status.hint")}</p>
          {#if harnessStore.status === "draft" || harnessStore.status === "pending"}
            <div class="status-actions">
              <button class="small primary" disabled={busy} onclick={() => void startReview()}>
                {harnessStore.status === "draft" ? t("harness.status.review") : t("harness.status.sync")}
              </button>
              {#if harnessStore.dirty}
                <button class="small" disabled={busy} onclick={() => harnessStore.discard()}>{t("harness.status.discard")}</button>
              {/if}
            </div>
          {/if}
          {#if harnessStore.error}
            <div class="error-box"><pre>{harnessStore.error}</pre></div>
          {/if}
        </section>

        <!-- The rules, as two plain lists: what the agents do, and what
             they don't. -->
        <HarnessRuleSection side="do" spec={draft} {root}>
          {#snippet footer()}
            {#if suggestedChecks.length > 0}
              <div class="chips" role="group" aria-label={t("harness.checks.suggested")}>
                <span class="chips-label">{t("harness.checks.suggested")}</span>
                {#each suggestedChecks as command (command)}
                  <button class="chip mono" onclick={() => harnessStore.edit((spec) => spec.checks.push({ command }))}>+ {command}</button>
                {/each}
              </div>
            {/if}
          {/snippet}
        </HarnessRuleSection>

        <HarnessRuleSection side="dont" spec={draft} {root}>
          {#snippet footer()}
            <div class="chips" role="group" aria-label={t("harness.packs.title")}>
              <span class="chips-label">{t("harness.packs.title")}</span>
              {#each COMMAND_PACKS as pack (pack.id)}
                {@const added = hasPack(draft, pack)}
                <button
                  class="chip"
                  disabled={added}
                  title={pack.rules.map((rule) => `${t(DECISION_LABEL[rule.decision])}: ${rule.prefix}`).join("\n")}
                  onclick={() => harnessStore.edit((spec) => addPack(spec, pack))}
                >
                  {added ? "✓" : "+"} {t(pack.label)}
                </button>
              {/each}
            </div>
          {/snippet}
        </HarnessRuleSection>

        <!-- The rest, folded away until wanted. -->
        <p class="more-title">{t("harness.more")}</p>

        <!-- An agent that looks at the repository: rules it proposes, and
             the instruction files unified. -->
        <section>
          {@render sectionHead("assist", t("harness.assist.title"), proposals && proposals.root === root ? proposals.items.length : null)}
          {#if !collapsed.includes("assist")}
            <div class="section-content">
              <p class="hint">{t("harness.assist.hint")}</p>
              {#if assist && assist.root === root}
                {@const thought = latestThought(assist.steps, 200)}
                <div class="working" aria-live="polite">
                  <div class="working-line">
                    <span class="spinner" aria-hidden="true"></span>
                    <span class="working-title">
                      {t(assist.kind === "propose" ? "harness.assist.proposing" : "harness.assist.unifying")}
                    </span>
                    <span class="elapsed">{formatDuration(Math.max(0, now - assist.startedAt))}</span>
                    <button class="small" onclick={() => harnessStore.cancelAssist()}>{t("harness.assist.cancel")}</button>
                  </div>
                  {#if thought}
                    <p class="thought">{thought}</p>
                  {/if}
                  {#each toolSteps(assist.steps, 4) as step, index (index)}
                    {@const line = stepLine(step, t)}
                    <p class="step"><span aria-hidden="true">{line.icon}</span> {line.text}</p>
                  {/each}
                </div>
              {:else if aiEngine.value}
                <div class="engine-row">
                  <AiEnginePicker id="harness-engine" />
                </div>
                <div class="assist-actions">
                  <button class="small primary" disabled={!!assist} title={t("harness.assist.proposeHint")} onclick={() => void harnessStore.propose(aiEngine.value!)}>
                    {t("harness.assist.propose")}
                  </button>
                  <button
                    class="small"
                    disabled={!!assist || instructionFiles.length < 2}
                    title={instructionFiles.length < 2 ? t("harness.assist.unifyNeedsTwo") : t("harness.assist.unifyHint")}
                    onclick={() => void harnessStore.unify(aiEngine.value!)}
                  >
                    {t("harness.assist.unify")}
                  </button>
                </div>
              {:else}
                <p class="hint">{t("harness.assist.noEngine")}</p>
              {/if}
              {#if harnessStore.assistError && !unified}
                <div class="error-box"><pre>{harnessStore.assistError}</pre></div>
              {/if}
              {#if proposals && proposals.root === root}
                {#if proposals.summary}
                  <p class="summary">{proposals.summary}</p>
                {/if}
                {#if proposals.items.length === 0}
                  <p class="hint">{t("harness.assist.noProposals")}</p>
                {:else}
                  <ul class="proposal-list">
                    {#each proposals.items as proposal, index (index)}
                      {@const already = hasProposed(draft, proposal.rule)}
                      <li class="proposal">
                        <div class="proposal-rule">
                          <span class="proposal-kind {proposal.rule.kind === 'command' ? proposal.rule.decision : proposal.rule.kind}">
                            {proposalLabel(proposal.rule)}
                          </span>
                          <code>{proposal.rule.kind === "path" ? proposal.rule.pattern : proposal.rule.kind === "command" ? proposal.rule.prefix : proposal.rule.command}</code>
                        </div>
                        {#if proposal.reason}
                          <p class="proposal-reason">{proposal.reason}</p>
                        {/if}
                        {#if proposal.source}
                          <p class="proposal-source" title={proposal.source.file}>
                            {t("harness.assist.from", { file: proposal.source.file })} <q>{proposal.source.quote}</q>
                          </p>
                        {/if}
                        <div class="proposal-actions">
                          <button class="small" onclick={() => harnessStore.dismissProposal(proposal)}>{t("harness.assist.dismiss")}</button>
                          <button class="small primary" disabled={already} onclick={() => harnessStore.acceptProposal(proposal)}>
                            {already ? t("harness.assist.added") : t("harness.add")}
                          </button>
                        </div>
                      </li>
                    {/each}
                  </ul>
                  <div class="assist-actions end">
                    <button class="link" onclick={() => harnessStore.clearProposals()}>{t("harness.assist.dismissAll")}</button>
                    <button
                      class="small"
                      onclick={() => {
                        for (const proposal of [...(proposals?.items ?? [])]) harnessStore.acceptProposal(proposal);
                      }}
                    >
                      {t("harness.assist.addAll")}
                    </button>
                  </div>
                {/if}
              {/if}
            </div>
          {/if}
        </section>

        <section>
          {@render sectionHead("agents", t("harness.agents.title"), null)}
          {#if !collapsed.includes("agents")}
            <div class="section-content">
              <p class="hint">{t("harness.agents.hint")}</p>
              {#each AGENTS as agent (agent)}
                <label class="check-row">
                  <input
                    type="checkbox"
                    checked={draft.agents.includes(agent)}
                    onchange={(event) => toggleAgent(agent, event.currentTarget.checked)}
                  />
                  <span>{TOOL_DISPLAY_NAMES[agent]}</span>
                </label>
              {/each}
              {#if trusting.length > 0}
                <p class="notice">
                  {t("harness.support.trust", { agents: trusting.map((agent) => TOOL_DISPLAY_NAMES[agent]).join(", ") })}
                  {#if trusting.includes("codex")}
                    {t("harness.support.codexHooks")}
                  {/if}
                </p>
              {/if}
            </div>
          {/if}
        </section>

        <!-- A command typed in, and what each agent would do with it. -->
        <section>
          {@render sectionHead("tester", t("harness.tester.title"), null)}
          {#if !collapsed.includes("tester")}
            <div class="section-content">
              <p class="hint">{t("harness.tester.hint")}</p>
              <input
                type="text"
                class="mono-input"
                placeholder={t("harness.tester.placeholder")}
                aria-label={t("harness.tester.placeholder")}
                bind:value={testInput}
              />
              {#if testInput.trim()}
                <p class="hint">
                  {testRule ? t("harness.tester.matched", { rule: testRule.prefix }) : t("harness.tester.unmatched")}
                </p>
                <ul class="verdicts">
                  {#each verdicts as { agent, verdict } (agent)}
                    <li>
                      <span class="verdict-agent">{TOOL_DISPLAY_NAMES[agent]}</span>
                      <span class="verdict {verdict}">{t(VERDICT_LABEL[verdict])}</span>
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/if}
        </section>

        <!-- The instruction files already in the repository, edited whole
             in the panel. -->
        <section>
          {@render sectionHead("files", t("harness.files.title"), instructionFiles.length)}
          {#if !collapsed.includes("files")}
            <div class="section-content">
              {#if instructionFiles.length === 0}
                <p class="hint">{t("harness.files.none")}</p>
              {:else}
                <p class="hint">{t("harness.files.hint")}</p>
                <ul class="file-list">
                  {#each instructionFiles as file (file)}
                    <li class="file-row">
                      <code title={file}>{file}</code>
                      <button class="small" onclick={() => harnessStore.editFile(file)}>{t("harness.files.edit")}</button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/if}
        </section>

        <!-- Rules in the agents' files that Termo didn't write. -->
        {#if externalGroups.length > 0}
          <section>
            {@render sectionHead("external", t("harness.external.title"), externalGroups.length)}
            {#if !collapsed.includes("external")}
              <div class="section-content">
                <p class="hint">{t("harness.external.hint")}</p>
                <ul class="external-list">
                  {#each externalGroups as group (group.key)}
                    <li class="external-row">
                      <div class="external-text">
                        {#each group.entries as entry (entry.key + entry.value)}
                          <code title={`${entry.file} · ${entry.key}`}>{entry.value}</code>
                        {/each}
                        {#if group.rule}
                          <span class="external-rule">
                            → {group.rule.kind === "path"
                              ? `${t(ACCESS_LABEL[group.rule.access])}: ${group.rule.pattern}`
                              : `${t(DECISION_LABEL[group.rule.decision])}: ${group.rule.prefix}`}
                          </span>
                        {:else}
                          <span class="external-rule muted">{t("harness.external.notARule")}</span>
                        {/if}
                      </div>
                      {#if group.rule}
                        <button class="small" title={t("harness.external.adoptHint")} onclick={() => harnessStore.adopt(group.entries)}>
                          {t("harness.external.adopt")}
                        </button>
                      {/if}
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
          </section>
        {/if}
      </div>
    {/if}
  </div>
</aside>

<style>
  /* Docked at the window's left edge, like the explorer and the Git panel,
     with a width of its own: it holds editors. */
  .harness-panel {
    position: relative;
    flex-shrink: 0;
    width: var(--panel-width, 380px);
    overflow: hidden;
    border-right: 1px solid var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .harness-inner {
    width: var(--panel-width, 380px);
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .harness-header {
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

  .harness-body {
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

  section {
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .status-block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 9px 10px 10px;
  }

  .status-line {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    flex-shrink: 0;
    border-radius: 50%;
    background: rgba(128, 128, 128, 0.6);
  }

  .status-dot.synced {
    background: #4caf72;
  }

  .status-dot.draft {
    background: var(--pane-active-border, #3a6df0);
  }

  .status-dot.pending {
    background: #d7a72a;
  }

  .status-text {
    font-weight: 600;
  }

  .status-hint,
  .hint {
    margin: 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .status-actions,
  .review-actions {
    display: flex;
    gap: 6px;
  }

  .review-actions {
    justify-content: flex-end;
    padding: 8px 10px 12px;
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

  .section-count {
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

  .section-content {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 10px 10px;
  }

  .check-row {
    display: flex;
    align-items: center;
    gap: 7px;
    cursor: pointer;
  }

  input[type="checkbox"] {
    margin: 0;
    accent-color: var(--pane-active-border, #3a6df0);
  }

  .rule-list,
  .review-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  input[type="text"] {
    box-sizing: border-box;
    padding: 3px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    color: inherit;
    font: inherit;
    font-size: 11.5px;
  }

  input[type="text"]:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .small {
    flex-shrink: 0;
    padding: 3px 9px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .small:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .small:disabled {
    opacity: 0.45;
    cursor: default;
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

  .error-box {
    padding: 5px 7px;
    border-radius: 4px;
    background: rgba(224, 90, 90, 0.14);
  }

  .error-box.padded {
    margin: 0 10px;
  }

  .error-box pre {
    margin: 0;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 10.5px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .review-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 2px;
  }

  .review-title {
    font-weight: 600;
  }

  .link {
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

  .review-file {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 5px 10px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .review-file:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .review-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
  }

  .review-kind {
    flex-shrink: 0;
    padding: 0 5px;
    border-radius: 3px;
    font-size: 10px;
    line-height: 15px;
    background: rgba(128, 128, 128, 0.2);
  }

  .review-kind.create {
    color: #4caf72;
  }

  .review-kind.delete {
    color: #e05a5a;
  }

  .review-diff {
    padding: 2px 10px 8px;
  }

  .engine-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: 11px;
  }

  .engine-row :global(.ai-engine-note) {
    margin: 0;
    font-size: 10.5px;
    opacity: 0.6;
  }

  .assist-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .assist-actions.end {
    justify-content: flex-end;
    align-items: center;
  }

  .working {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 6px 7px;
    border-radius: 4px;
    background: rgba(58, 109, 240, 0.12);
  }

  .working-line {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .working-title {
    flex: 1;
    min-width: 0;
    font-weight: 600;
  }

  .elapsed {
    font-size: 10.5px;
    opacity: 0.6;
    font-variant-numeric: tabular-nums;
  }

  .spinner {
    flex-shrink: 0;
    width: 11px;
    height: 11px;
    border: 2px solid var(--pane-active-border, #3a6df0);
    border-right-color: transparent;
    border-radius: 50%;
    animation: harness-spin 0.8s linear infinite;
  }

  @keyframes harness-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .thought,
  .step {
    margin: 0;
    font-size: 10.5px;
    opacity: 0.75;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .thought {
    white-space: normal;
    font-style: italic;
  }

  .summary {
    margin: 0;
    font-size: 11px;
    line-height: 1.4;
  }

  .section-note.summary {
    opacity: 0.85;
  }

  .proposal-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .proposal {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 6px 7px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
  }

  .proposal-rule {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }

  .proposal-rule code {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    font-weight: 600;
  }

  .proposal-kind {
    flex-shrink: 0;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    opacity: 0.85;
  }

  .proposal-kind.deny,
  .proposal-kind.path {
    color: #e05a5a;
  }

  .proposal-kind.ask {
    color: #d7a72a;
  }

  .proposal-kind.allow,
  .proposal-kind.check {
    color: #4caf72;
  }

  .proposal-reason,
  .proposal-source {
    margin: 0;
    font-size: 11px;
    line-height: 1.35;
  }

  .proposal-source {
    opacity: 0.6;
  }

  .proposal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }

  .chips-label {
    font-size: 10.5px;
    opacity: 0.6;
  }

  .chip {
    padding: 1px 7px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 10px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 10.5px;
    cursor: pointer;
  }

  .chip:hover:not(:disabled) {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .chip:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .chip.mono,
  .mono-input {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  }

  .verdicts {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .verdicts li {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 11px;
  }

  .verdict {
    font-weight: 600;
  }

  .verdict.deny {
    color: #e05a5a;
  }

  .verdict.ask {
    color: #d7a72a;
  }

  .verdict.allow {
    color: #4caf72;
  }

  .verdict.text {
    color: var(--pane-active-border, #3a6df0);
  }

  .verdict.none {
    opacity: 0.55;
    font-weight: normal;
  }

  .more-title {
    margin: 14px 10px 4px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.45;
  }

  .harness-body > .more-title + section {
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .file-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .file-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 24px;
    padding-left: 6px;
  }

  .file-row code {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
  }

  .external-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .external-row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 4px 6px;
    border-radius: 4px;
    background: rgba(128, 128, 128, 0.1);
  }

  .external-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .external-text code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 10.5px;
  }

  .external-rule {
    font-size: 10.5px;
    opacity: 0.8;
  }

  .external-rule.muted {
    opacity: 0.5;
  }

  .text {
    color: var(--pane-active-border, #3a6df0);
    font-size: 10px;
  }

  .none {
    opacity: 0.45;
  }

  .notice {
    margin: 0;
    padding: 5px 7px;
    border-radius: 4px;
    background: rgba(215, 167, 42, 0.16);
    font-size: 11px;
  }
</style>
