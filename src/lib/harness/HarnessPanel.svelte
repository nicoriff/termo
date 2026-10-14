<script lang="ts">
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/i18n.svelte";
  import SidebarResizeHandle from "$lib/app/SidebarResizeHandle.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import type { CommandDecision, PathAccess } from "$lib/core/harness-client";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { fileName } from "$lib/layout/split-tree";
  import HarnessDiff from "./HarnessDiff.svelte";
  import HarnessTextEditor from "./HarnessTextEditor.svelte";
  import {
    ACCESS_LEVELS,
    AGENTS,
    CHANGE_KIND_LABEL,
    DECISIONS,
    NEEDS_TRUST,
    SUPPORT,
    changeKind,
    commandProblem,
    normalizePattern,
    normalizePrefix,
    pathProblem,
    relativeToRoot,
    ruleKinds,
    type Support,
  } from "./harness-model";
  import { HARNESS_WIDTH, harnessStore } from "./harness-store.svelte";

  const COLLAPSED_KEY = "termo.harness.collapsedSections";

  type SectionId = "agents" | "paths" | "commands" | "checks" | "instructions" | "support";

  function loadCollapsed(): SectionId[] {
    try {
      const saved = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
      return Array.isArray(saved) ? saved : [];
    } catch {
      return [];
    }
  }

  const root = $derived(harnessStore.root);
  const draft = $derived(harnessStore.draft);
  const review = $derived(harnessStore.review);
  const busy = $derived(harnessStore.busy);
  const repoName = $derived(root ? fileName(root) : t("harness.title"));
  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleHarness"), isMac));
  const pendingCount = $derived(harnessStore.saved?.pending.length ?? 0);
  const kinds = $derived(draft ? ruleKinds(draft) : []);
  const targeted = $derived(draft ? AGENTS.filter((agent) => draft.agents.includes(agent)) : []);
  const trusting = $derived(targeted.filter((agent) => NEEDS_TRUST.includes(agent)));

  const SUPPORT_MARK: Record<Support, string> = { enforced: "✓", partial: "◐", text: "Aa", none: "–" };
  const SUPPORT_LABEL = {
    enforced: "harness.support.enforced",
    partial: "harness.support.partial",
    text: "harness.support.text",
    none: "harness.support.none",
  } as const;
  const KIND_LABEL = {
    noEdit: "harness.support.kinds.noEdit",
    noRead: "harness.support.kinds.noRead",
    deny: "harness.support.kinds.deny",
    ask: "harness.support.kinds.ask",
    allow: "harness.support.kinds.allow",
    checks: "harness.support.kinds.checks",
  } as const;
  /** Short enough for a column; the full name is its tooltip. */
  const SHORT_NAME = { claude: "Claude", codex: "Codex", gemini: "Gemini", copilot: "Copilot", grok: "Grok" } as const;

  let collapsed = $state<SectionId[]>(loadCollapsed());
  /** The files on review shown open, by path. */
  let expanded = $state<string[]>([]);

  let newPath = $state("");
  let newAccess = $state<PathAccess>("noEdit");
  let pathNote = $state<string | null>(null);
  let newCommand = $state("");
  let newDecision = $state<CommandDecision>("deny");
  let newCheck = $state("");

  const pathError = $derived(
    draft && newPath.trim() ? pathProblem(newPath, draft.paths.map((rule) => rule.pattern)) : null,
  );
  const commandError = $derived(
    draft && newCommand.trim() ? commandProblem(newCommand, draft.commands.map((rule) => rule.prefix)) : null,
  );

  const ACCESS_LABEL = { noEdit: "harness.paths.noEdit", noRead: "harness.paths.noRead" } as const;
  const DECISION_LABEL = {
    deny: "harness.commands.deny",
    ask: "harness.commands.ask",
    allow: "harness.commands.allow",
  } as const;
  const DECISION_HINT = {
    deny: "harness.commands.denyHint",
    ask: "harness.commands.askHint",
    allow: "harness.commands.allowHint",
  } as const;

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

  function addPath(event: SubmitEvent) {
    event.preventDefault();
    if (!newPath.trim() || pathError) return;
    const pattern = normalizePattern(newPath);
    const access = newAccess;
    harnessStore.edit((spec) => spec.paths.push({ pattern, access }));
    newPath = "";
    pathNote = null;
  }

  async function browse() {
    if (!root) return;
    const picked = await openDialog({ defaultPath: root, multiple: false, directory: false });
    if (typeof picked !== "string") return;
    const relative = relativeToRoot(root, picked);
    if (relative === null) {
      pathNote = t("harness.paths.outside");
      return;
    }
    pathNote = null;
    newPath = relative;
  }

  function addCommand(event: SubmitEvent) {
    event.preventDefault();
    if (!newCommand.trim() || commandError) return;
    const prefix = normalizePrefix(newCommand);
    const decision = newDecision;
    harnessStore.edit((spec) => spec.commands.push({ prefix, decision }));
    newCommand = "";
  }

  function addCheck(event: SubmitEvent) {
    event.preventDefault();
    const command = newCheck.trim();
    if (!command || draft?.checks.some((check) => check.command.trim() === command)) return;
    harnessStore.edit((spec) => spec.checks.push({ command }));
    newCheck = "";
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
    // Escape inside the instructions' editor is the editor's.
    if ((event.target as HTMLElement | null)?.closest(".cm-editor")) return;
    if (review !== null) harnessStore.cancelReview();
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

{#snippet removeButton(label: string, onclick: () => void)}
  <button class="icon-button remove" title={label} aria-label={label} {onclick}>
    <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.1" /></svg>
  </button>
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
          <ul class="review-list">
            {#each review as change (change.path)}
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
            </div>
          {/if}
        </section>

        <section>
          {@render sectionHead("paths", t("harness.paths.title"), draft.paths.length)}
          {#if !collapsed.includes("paths")}
            <div class="section-content">
              <p class="hint">{t("harness.paths.hint")}</p>
              {#if draft.paths.length > 0}
                <ul class="rule-list">
                  {#each draft.paths as rule, index (rule.pattern)}
                    <li class="rule-row">
                      <code class="rule-text" title={rule.pattern}>{rule.pattern}</code>
                      <select
                        aria-label={t("harness.paths.access")}
                        value={rule.access}
                        onchange={(event) => {
                          const access = event.currentTarget.value as PathAccess;
                          harnessStore.edit((spec) => (spec.paths[index].access = access));
                        }}
                      >
                        {#each ACCESS_LEVELS as level (level)}
                          <option value={level}>{t(ACCESS_LABEL[level])}</option>
                        {/each}
                      </select>
                      {@render removeButton(t("harness.remove"), () => harnessStore.edit((spec) => spec.paths.splice(index, 1)))}
                    </li>
                  {/each}
                </ul>
              {/if}
              <form class="add-form" onsubmit={addPath}>
                <input
                  type="text"
                  placeholder={t("harness.paths.placeholder")}
                  aria-label={t("harness.paths.placeholder")}
                  bind:value={newPath}
                  oninput={() => (pathNote = null)}
                />
                <select aria-label={t("harness.paths.access")} bind:value={newAccess}>
                  {#each ACCESS_LEVELS as level (level)}
                    <option value={level}>{t(ACCESS_LABEL[level])}</option>
                  {/each}
                </select>
                <button type="button" class="small" title={t("harness.paths.browse")} onclick={() => void browse()}>…</button>
                <button type="submit" class="small" disabled={!newPath.trim() || !!pathError}>{t("harness.add")}</button>
              </form>
              {#if pathError || pathNote}
                <p class="field-error">{pathNote ?? (pathError ? t(pathError) : "")}</p>
              {/if}
            </div>
          {/if}
        </section>

        <section>
          {@render sectionHead("commands", t("harness.commands.title"), draft.commands.length)}
          {#if !collapsed.includes("commands")}
            <div class="section-content">
              <p class="hint">{t("harness.commands.hint")}</p>
              {#each DECISIONS as decision (decision)}
                {@const rules = draft.commands.map((rule, index) => ({ rule, index })).filter(({ rule }) => rule.decision === decision)}
                {#if rules.length > 0}
                  <div class="decision-group">
                    <span class="decision-title {decision}" title={t(DECISION_HINT[decision])}>{t(DECISION_LABEL[decision])}</span>
                    <ul class="rule-list">
                      {#each rules as { rule, index } (rule.prefix)}
                        <li class="rule-row">
                          <code class="rule-text" title={rule.prefix}>{rule.prefix}</code>
                          <select
                            aria-label={t("harness.commands.decision")}
                            value={rule.decision}
                            onchange={(event) => {
                              const next = event.currentTarget.value as CommandDecision;
                              harnessStore.edit((spec) => (spec.commands[index].decision = next));
                            }}
                          >
                            {#each DECISIONS as option (option)}
                              <option value={option}>{t(DECISION_LABEL[option])}</option>
                            {/each}
                          </select>
                          {@render removeButton(t("harness.remove"), () => harnessStore.edit((spec) => spec.commands.splice(index, 1)))}
                        </li>
                      {/each}
                    </ul>
                  </div>
                {/if}
              {/each}
              <form class="add-form" onsubmit={addCommand}>
                <input
                  type="text"
                  placeholder={t("harness.commands.placeholder")}
                  aria-label={t("harness.commands.placeholder")}
                  bind:value={newCommand}
                />
                <select aria-label={t("harness.commands.decision")} bind:value={newDecision}>
                  {#each DECISIONS as option (option)}
                    <option value={option}>{t(DECISION_LABEL[option])}</option>
                  {/each}
                </select>
                <button type="submit" class="small" disabled={!newCommand.trim() || !!commandError}>{t("harness.add")}</button>
              </form>
              {#if commandError}
                <p class="field-error">{t(commandError)}</p>
              {/if}
            </div>
          {/if}
        </section>

        <section>
          {@render sectionHead("checks", t("harness.checks.title"), draft.checks.length)}
          {#if !collapsed.includes("checks")}
            <div class="section-content">
              <p class="hint">{t("harness.checks.hint")}</p>
              {#if draft.checks.length > 0}
                <ul class="rule-list">
                  {#each draft.checks as check, index (check.command)}
                    <li class="rule-row">
                      <code class="rule-text" title={check.command}>{check.command}</code>
                      {@render removeButton(t("harness.remove"), () => harnessStore.edit((spec) => spec.checks.splice(index, 1)))}
                    </li>
                  {/each}
                </ul>
              {/if}
              <form class="add-form" onsubmit={addCheck}>
                <input
                  type="text"
                  placeholder={t("harness.checks.placeholder")}
                  aria-label={t("harness.checks.placeholder")}
                  bind:value={newCheck}
                />
                <button type="submit" class="small" disabled={!newCheck.trim()}>{t("harness.add")}</button>
              </form>
            </div>
          {/if}
        </section>

        <section>
          {@render sectionHead("instructions", t("harness.instructions.title"), null)}
          {#if !collapsed.includes("instructions")}
            <div class="section-content">
              <p class="hint">{t("harness.instructions.hint")}</p>
              <HarnessTextEditor
                value={draft.instructions}
                label={t("harness.instructions.title")}
                placeholder={t("harness.instructions.placeholder")}
                onchange={(text) => harnessStore.edit((spec) => (spec.instructions = text))}
              />
            </div>
          {/if}
        </section>

        <!-- Which agent holds which kind of rule by itself, and which only
             hears of it in AGENTS.md. -->
        <section>
          {@render sectionHead("support", t("harness.support.title"), null)}
          {#if !collapsed.includes("support")}
            <div class="section-content">
              <p class="hint">{t("harness.support.hint")}</p>
              {#if kinds.length === 0 || targeted.length === 0}
                <p class="hint">{t("harness.support.empty")}</p>
              {:else}
                <table class="support-table">
                  <thead>
                    <tr>
                      <th></th>
                      {#each targeted as agent (agent)}
                        <th scope="col" title={TOOL_DISPLAY_NAMES[agent]}>{SHORT_NAME[agent]}</th>
                      {/each}
                    </tr>
                  </thead>
                  <tbody>
                    {#each kinds as kind (kind)}
                      <tr>
                        <th scope="row">{t(KIND_LABEL[kind])}</th>
                        {#each targeted as agent (agent)}
                          {@const support = SUPPORT[agent][kind]}
                          <td class={support} title={t(SUPPORT_LABEL[support])}>
                            <span aria-hidden="true">{SUPPORT_MARK[support]}</span>
                            <span class="visually-hidden">{t(SUPPORT_LABEL[support])}</span>
                          </td>
                        {/each}
                      </tr>
                    {/each}
                  </tbody>
                </table>
                <p class="legend">
                  {#each ["enforced", "partial", "text", "none"] as const as support (support)}
                    <span class="legend-item"><span class="mark {support}">{SUPPORT_MARK[support]}</span> {t(SUPPORT_LABEL[support])}</span>
                  {/each}
                </p>
              {/if}
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

  .icon-button.remove svg {
    width: 9px;
    height: 9px;
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

  .rule-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rule-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 24px;
    padding: 0 0 0 6px;
    border-radius: 4px;
  }

  .rule-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .rule-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
  }

  .decision-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .decision-title {
    font-size: 10.5px;
    font-weight: 600;
    opacity: 0.75;
  }

  .decision-title.deny {
    color: #e05a5a;
  }

  .decision-title.ask {
    color: #d7a72a;
  }

  .decision-title.allow {
    color: #4caf72;
  }

  .add-form {
    display: flex;
    gap: 4px;
  }

  .add-form input {
    flex: 1;
    min-width: 0;
  }

  input[type="text"],
  select {
    box-sizing: border-box;
    padding: 3px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    color: inherit;
    font: inherit;
    font-size: 11.5px;
  }

  input[type="text"]:focus,
  select:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .field-error {
    margin: 0;
    font-size: 11px;
    color: #e05a5a;
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

  .support-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 11px;
  }

  .support-table th,
  .support-table td {
    padding: 3px 4px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .support-table thead th {
    font-size: 10px;
    font-weight: 600;
    opacity: 0.7;
    text-align: center;
  }

  .support-table tbody th {
    font-weight: normal;
    text-align: left;
  }

  .support-table td {
    text-align: center;
    font-weight: 700;
  }

  .enforced {
    color: #4caf72;
  }

  .partial {
    color: #d7a72a;
  }

  .text {
    color: var(--pane-active-border, #3a6df0);
    font-size: 10px;
  }

  .none {
    opacity: 0.45;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    margin: 0;
    font-size: 10.5px;
    opacity: 0.8;
  }

  .legend-item {
    white-space: nowrap;
  }

  .mark {
    font-weight: 700;
  }

  .notice {
    margin: 0;
    padding: 5px 7px;
    border-radius: 4px;
    background: rgba(215, 167, 42, 0.16);
    font-size: 11px;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }
</style>
