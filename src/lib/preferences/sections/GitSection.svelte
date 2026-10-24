<script lang="ts">
  import { onMount } from "svelte";
  import {
    appConfigStore,
    type GitEditorBlame,
    type GitReviewBeforePush,
    type GitReviewDefaultScope,
    type GitReviewSeverity,
  } from "$lib/core/app-config.svelte";
  import { listAgents, listProjects, type AgentSpec } from "$lib/core/agents-client";
  import { gitTools, type GitTool, type GitTools } from "$lib/core/git-client";
  import { formatKeybindingLabel } from "$lib/core/shortcuts";
  import { isMac } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "../SettingDropdown.svelte";

  /** Opens the status bar's page, where the repository's item is arranged. */
  let { onstatusbar }: { onstatusbar: () => void } = $props();

  const shortcut = $derived(formatKeybindingLabel(appConfigStore.getEffectiveShortcut("toggleGit"), isMac));
  const off = $derived(!appConfigStore.gitEnabled);

  const fetchOptions = $derived([
    { value: "0", label: t("preferences.git.autoFetchNever") },
    ...[5, 15, 30, 60].map((minutes) => ({ value: String(minutes), label: t("preferences.git.autoFetchEvery", { n: String(minutes) }) })),
  ]);

  const blameOptions = $derived(
    (["line", "gutter", "off"] as const).map((mode) => ({ value: mode, label: t(`preferences.git.blameModes.${mode}`) })),
  );

  /** What's installed of what all this leans on; null while it's asked. */
  let tools = $state<GitTools | null>(null);

  /** The user's own agents, to pick who reviews and who fixes: of them, only
   * the ones made for it in the wizard are offered. The ones an orchestration
   * made for its team never are: they were written for their task in it. */
  let agents = $state<AgentSpec[]>([]);

  function loadAgents() {
    void Promise.all([listAgents(), listProjects()])
      .then(([list, projects]) => {
        const teams = new Set(projects.filter((project) => project.orchestrationId).map((project) => project.id));
        agents = list.filter((agent) => !teams.has(agent.projectId));
      })
      .catch(() => {});
  }

  // This window is preloaded once and reused: asked again on every open.
  onMount(() => {
    void gitTools().then((found) => (tools = found));
    loadAgents();
  });

  /** Reviews need Git, themselves, and Modo IA. */
  const reviewOff = $derived(off || !appConfigStore.gitReviewEnabled || !appConfigStore.aiModeEnabled);
  const agentLabel = (agent: AgentSpec) => `${agent.icon} ${agent.name}`.trim();
  const reviewerOptions = $derived([
    { value: "", label: t("preferences.git.review.builtInNone") },
    ...agents.filter((agent) => agent.gitRole === "reviewer").map((agent) => ({ value: agent.id, label: agentLabel(agent) })),
  ]);
  /** A fixer has to be able to change files, too. */
  const fixerOptions = $derived([
    { value: "", label: t("preferences.git.review.fixerNone") },
    ...agents
      .filter((agent) => agent.gitRole === "fixer" && agent.autonomy !== "read")
      .map((agent) => ({ value: agent.id, label: agentLabel(agent) })),
  ]);
  const scopeOptions = $derived([
    { value: "changes", label: t("git.review.scope.changes") },
    { value: "branch", label: t("git.review.scope.branch") },
  ]);
  const severityOptions = $derived(
    (["suggestion", "warning", "serious"] as const).map((severity) => ({ value: severity, label: t(`git.review.severity.${severity}`) })),
  );
  const timeoutOptions = $derived(
    [5, 10, 20, 30, 60].map((minutes) => ({ value: String(minutes), label: t("preferences.git.review.timeoutMinutes", { n: String(minutes) }) })),
  );

  const TOOLS = [
    { id: "git", name: "git", use: "preferences.git.tools.gitUse" },
    { id: "gh", name: "gh", use: "preferences.git.tools.ghUse" },
    { id: "glab", name: "glab", use: "preferences.git.tools.glabUse" },
  ] as const;

  function toolState(tool: GitTool): string {
    if (!tool.found) return t("preferences.git.tools.missing");
    if (tool.signedIn === false) return t("preferences.git.tools.signedOut");
    return tool.version ?? t("preferences.git.tools.found");
  }
</script>

<section>
  <h3>{t("preferences.git.title")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.git.enabled")}</span>
    <input type="checkbox" checked={appConfigStore.gitEnabled} onchange={(e) => appConfigStore.setGitEnabled(e.currentTarget.checked)} />
  </label>
  <p class="setting-hint">{t("preferences.git.enabledHint")}</p>
</section>

<section class:off>
  <h3>{t("preferences.git.panel")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.git.inActivityBar")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitInActivityBar}
      disabled={off}
      onchange={(e) => appConfigStore.setGitInActivityBar(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.git.inActivityBarHint", { shortcut })}</p>
</section>

<section class:off>
  <h3>{t("preferences.git.statusBar")}</h3>
  <div class="row">
    <span class="status-bar-moved">{t("preferences.git.statusBarMoved")}</span>
    <button class="status-bar-link" onclick={onstatusbar}>{t("preferences.git.statusBarOpen")}</button>
  </div>
</section>

<section class:off>
  <h3>{t("preferences.git.editor")}</h3>
  <div class="row spaced">
    <span>{t("preferences.git.blame")}</span>
    <SettingDropdown
      options={blameOptions}
      value={appConfigStore.gitEditorBlame}
      disabled={off}
      onpick={(value) => appConfigStore.setGitEditorBlame(value as GitEditorBlame)}
    />
  </div>
  <p class="setting-hint">{t("preferences.git.blameHint")}</p>
  <label class="row toggle-row sub">
    <span>{t("preferences.git.blameMessage")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitEditorBlameMessage}
      disabled={off || appConfigStore.gitEditorBlame === "off"}
      onchange={(e) => appConfigStore.setGitEditorBlameMessage(e.currentTarget.checked)}
    />
  </label>
</section>

<section class:off>
  <h3>{t("preferences.git.remote")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.git.pr")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitPrEnabled}
      disabled={off}
      onchange={(e) => appConfigStore.setGitPrEnabled(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.git.prHint")}</p>
  <div class="row spaced">
    <span>{t("preferences.git.autoFetch")}</span>
    <SettingDropdown
      options={fetchOptions}
      value={String(appConfigStore.gitAutoFetchMinutes)}
      disabled={off}
      onpick={(value) => appConfigStore.setGitAutoFetchMinutes(Number(value))}
    />
  </div>
  <p class="setting-hint">{t("preferences.git.autoFetchHint")}</p>
</section>

<section class:off>
  <h3>{t("preferences.git.review.title")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.git.review.enabled")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitReviewEnabled}
      disabled={off}
      onchange={(e) => appConfigStore.setGitReview({ enabled: e.currentTarget.checked })}
    />
  </label>
  <p class="setting-hint">{t("preferences.git.review.enabledHint")}</p>
  <div class="row spaced">
    <span>{t("preferences.git.review.reviewer")}</span>
    <SettingDropdown
      options={reviewerOptions}
      value={reviewerOptions.some((option) => option.value === appConfigStore.gitReviewAgentId) ? appConfigStore.gitReviewAgentId : ""}
      disabled={reviewOff}
      onopen={loadAgents}
      onpick={(value) => appConfigStore.setGitReview({ agentId: value })}
    />
  </div>
  <p class="setting-hint">{t("preferences.git.review.reviewerHint")}</p>
  <div class="row spaced">
    <span>{t("preferences.git.review.fixer")}</span>
    <SettingDropdown
      options={fixerOptions}
      value={fixerOptions.some((option) => option.value === appConfigStore.gitReviewFixAgentId) ? appConfigStore.gitReviewFixAgentId : ""}
      disabled={reviewOff}
      onopen={loadAgents}
      onpick={(value) => appConfigStore.setGitReview({ fixAgentId: value })}
    />
  </div>
  <p class="setting-hint">{t("preferences.git.review.fixerHint")}</p>
  <div class="row spaced">
    <span>{t("preferences.git.review.scope")}</span>
    <SettingDropdown
      options={scopeOptions}
      value={appConfigStore.gitReviewScope}
      disabled={reviewOff}
      onpick={(value) => appConfigStore.setGitReview({ scope: value as GitReviewDefaultScope })}
    />
  </div>
  <div class="row spaced">
    <span>{t("preferences.git.review.minSeverity")}</span>
    <SettingDropdown
      options={severityOptions}
      value={appConfigStore.gitReviewMinSeverity}
      disabled={reviewOff}
      onpick={(value) => appConfigStore.setGitReview({ minSeverity: value as GitReviewSeverity })}
    />
  </div>
  <div class="row spaced">
    <span>{t("preferences.git.review.timeout")}</span>
    <SettingDropdown
      options={timeoutOptions}
      value={String(appConfigStore.gitReviewTimeoutMin)}
      disabled={reviewOff}
      onpick={(value) => appConfigStore.setGitReview({ timeoutMin: Number(value) })}
    />
  </div>
  <label class="row toggle-row spaced">
    <span>{t("preferences.git.review.beforePush")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitReviewBeforePush === "ask"}
      disabled={reviewOff}
      onchange={(e) => appConfigStore.setGitReview({ beforePush: (e.currentTarget.checked ? "ask" : "off") satisfies GitReviewBeforePush })}
    />
  </label>
  <p class="setting-hint">{t("preferences.git.review.beforePushHint")}</p>
  <label class="row toggle-row spaced">
    <span>{t("preferences.git.review.onPrCreate")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.gitReviewOnPrCreate}
      disabled={reviewOff || !appConfigStore.gitPrEnabled}
      onchange={(e) => appConfigStore.setGitReview({ onPrCreate: e.currentTarget.checked })}
    />
  </label>
  <label class="field spaced">
    <span>{t("preferences.git.review.instructions")}</span>
    <textarea
      rows="3"
      placeholder={t("preferences.git.review.instructionsPlaceholder")}
      value={appConfigStore.gitReviewInstructions}
      disabled={reviewOff}
      onchange={(e) => appConfigStore.setGitReview({ instructions: e.currentTarget.value })}
    ></textarea>
  </label>
  <p class="setting-hint">{t("preferences.git.review.instructionsHint")}</p>
</section>

<section>
  <h3>{t("preferences.git.tools.title")}</h3>
  {#each TOOLS as tool (tool.id)}
    {@const found = tools?.[tool.id]}
    <div class="row tool">
      <span class="tool-dot" class:ok={found?.found && found.signedIn !== false} class:warn={found?.found && found.signedIn === false}></span>
      <span class="tool-name">{tool.name}</span>
      <span class="tool-use">{t(tool.use)}</span>
      <span class="tool-state">{found ? toolState(found) : t("preferences.git.tools.checking")}</span>
    </div>
  {/each}
  <p class="setting-hint">{t("preferences.git.tools.hint")}</p>
</section>

<style>
  section {
    margin-bottom: 20px;
  }

  /* Git as a whole is off: its options stay in view, out of reach. */
  section.off {
    opacity: 0.55;
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    margin: 0 0 8px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
  }

  .status-bar-moved {
    opacity: 0.75;
  }

  .status-bar-link {
    flex-shrink: 0;
    padding: 4px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .status-bar-link:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .row.spaced {
    margin-top: 12px;
  }

  .row.sub {
    margin-top: 7px;
    padding-left: 14px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
  }

  .field.spaced {
    margin-top: 12px;
  }

  .field textarea {
    box-sizing: border-box;
    width: 100%;
    padding: 5px 7px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    color: inherit;
    font: inherit;
    resize: vertical;
  }

  .field textarea:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .toggle-row {
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .row.tool {
    justify-content: flex-start;
    margin-bottom: 6px;
  }

  .tool-dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(128, 128, 128, 0.6);
  }

  .tool-dot.ok {
    background: #3fa04a;
  }

  .tool-dot.warn {
    background: #d7a72a;
  }

  .tool-name {
    flex-shrink: 0;
    width: 34px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-weight: 600;
  }

  .tool-use {
    flex: 1;
    min-width: 0;
    opacity: 0.65;
  }

  .tool-state {
    flex-shrink: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
    opacity: 0.8;
  }
</style>
