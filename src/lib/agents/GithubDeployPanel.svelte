<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { t } from "$lib/i18n/i18n.svelte";
  import { githubPreview, githubWrite, type AgentSpec, type GithubPreview } from "$lib/core/agents-client";

  /** The wizard's agent; the deploy record is written into it. */
  let { spec }: { spec: AgentSpec } = $props();

  let preview = $state<GithubPreview | null>(null);
  let error = $state("");
  let showYaml = $state(false);
  let writing = $state(false);
  let written = $state<string | null>(null);
  let copied = $state<string | null>(null);

  const commitCommand = $derived(
    preview ? `git add ${preview.workflowFile} && git commit -m "Add agent: ${spec.name}" && git push` : "",
  );

  onMount(() => {
    void githubPreview($state.snapshot(spec))
      .then((result) => (preview = result))
      .catch((reason) => (error = String(reason)));
  });

  async function copy(text: string) {
    await writeText(text).catch(() => {});
    copied = text;
    setTimeout(() => (copied = copied === text ? null : copied), 1500);
  }

  async function write() {
    if (!preview) return;
    writing = true;
    error = "";
    try {
      written = await githubWrite($state.snapshot(spec));
      spec.deploy.github = {
        repo: preview.repo ?? "",
        repoPath: spec.workDir,
        workflowFile: preview.workflowFile,
        deployedAt: Math.floor(Date.now() / 1000),
      };
    } catch (reason) {
      error = String(reason);
    } finally {
      writing = false;
    }
  }
</script>

<div class="github">
  {#if error && !preview}
    <p class="error">{error}</p>
  {:else if !preview}
    <p class="hint">{t("agents.github.loading")}</p>
  {:else}
    <p class="hint">
      {t("agents.github.intro", { file: preview.workflowFile })}
      {#if preview.repo}<b>{preview.repo}</b>{:else}{t("agents.github.noRemote")}{/if}
    </p>
    {#if preview.approximated}<p class="note">⚠ {t("agents.github.approximated")}</p>{/if}
    {#if preview.dropsWatch}<p class="note">⚠ {t("agents.github.dropsWatch")}</p>{/if}

    <ol class="steps">
      <li>
        <span class="step-title">{t("agents.github.tokenTitle")}</span>
        <span class="hint">{t("agents.github.tokenText")}</span>
        <div class="command">
          <code>claude setup-token</code>
          <button class="small" onclick={() => void copy("claude setup-token")}>
            {copied === "claude setup-token" ? t("agents.github.copied") : t("agents.github.copy")}
          </button>
        </div>
      </li>
      <li>
        <span class="step-title">{t("agents.github.secretsTitle")}</span>
        <span class="hint">{t("agents.github.secretsText")}</span>
        <div class="secrets">
          {#each preview.secrets as name (name)}<code>{name}</code>{/each}
        </div>
        {#if preview.repo}
          <button class="small" onclick={() => void openUrl(`https://github.com/${preview!.repo}/settings/secrets/actions/new`)}>
            {t("agents.github.openSecrets")} ↗
          </button>
        {/if}
      </li>
      <li>
        <span class="step-title">{t("agents.github.writeTitle")}</span>
        <div class="row">
          <button class="small primary" disabled={writing} onclick={() => void write()}>
            {written ? t("agents.github.rewrite") : t("agents.github.write")}
          </button>
          <button class="small" onclick={() => (showYaml = !showYaml)}>
            {showYaml ? t("agents.github.hideYaml") : t("agents.github.showYaml")}
          </button>
        </div>
        {#if written}<span class="ok">✓ {written}</span>{/if}
        {#if showYaml}<pre class="yaml">{preview.yaml}</pre>{/if}
      </li>
      <li>
        <span class="step-title">{t("agents.github.pushTitle")}</span>
        <div class="command">
          <code>{commitCommand}</code>
          <button class="small" onclick={() => void copy(commitCommand)}>
            {copied === commitCommand ? t("agents.github.copied") : t("agents.github.copy")}
          </button>
        </div>
      </li>
      <li>
        <span class="step-title">{t("agents.github.runTitle")}</span>
        <span class="hint">{t("agents.github.runText")}</span>
        {#if preview.repo}
          <button class="small" onclick={() => void openUrl(`https://github.com/${preview!.repo}/actions`)}>
            {t("agents.github.openActions")} ↗
          </button>
        {/if}
      </li>
    </ol>
    {#if error}<p class="error">{error}</p>{/if}
  {/if}
</div>

<style>
  .github {
    margin-top: 12px;
    padding: 12px 14px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
  }

  .hint {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.5;
    opacity: 0.7;
  }

  .note {
    margin: 6px 0 0;
    font-size: 11.5px;
    color: var(--warning-fg, #d7ba7d);
  }

  /* Numbered by hand: a flex <li> loses its list marker. */
  .steps {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 12px 0 0;
    padding-left: 0;
    list-style: none;
    counter-reset: step;
  }

  .step-title::before {
    counter-increment: step;
    content: counter(step) ". ";
    color: var(--pane-active-border, #3a6df0);
  }

  .steps li {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 5px;
  }

  .step-title {
    font-weight: 600;
  }

  .command {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 100%;
  }

  code {
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.3));
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }

  .secrets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  .small {
    flex-shrink: 0;
    padding: 3px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .small:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .small.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .ok {
    font-size: 11.5px;
    color: var(--success-fg, #89d185);
    overflow-wrap: anywhere;
  }

  .yaml {
    box-sizing: border-box;
    width: 100%;
    max-height: 220px;
    margin: 0;
    padding: 8px 10px;
    overflow: auto;
    border-radius: 6px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.3));
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    line-height: 1.45;
    scrollbar-width: thin;
  }

  .error {
    margin: 6px 0 0;
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }
</style>
