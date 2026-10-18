<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { open } from "@tauri-apps/plugin-dialog";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { openLink } from "$lib/core/links";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import type { ToolId } from "$lib/ai-mode/pattern-table";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import { installAgentHooks } from "$lib/core/agent-hooks-client";
  import {
    agentInstallInfo,
    cancelAgentCliInstall,
    installAgentCli,
    installCommand,
    onAgentInstallProgress,
    type AgentInstallInfo,
    type AgentInstallStage,
  } from "$lib/core/agent-install-client";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";

  /**
   * Installs an agent's CLI with npm: where, the install itself, then
   * whether to set up its hooks. Used from Preferences and from the setup,
   * which are different windows, so it covers whichever one it's in.
   */
  let {
    tool,
    onclose,
  }: {
    tool: ToolId;
    /** `installed`: whether the CLI is there now. */
    onclose: (installed: boolean) => void;
  } = $props();

  const STAGE_KEYS: Record<AgentInstallStage, MessageKey> = {
    checking: "agentInstall.stages.checking",
    installing: "agentInstall.stages.installing",
    verifying: "agentInstall.stages.verifying",
  };
  /** How much of npm's output is kept on screen. */
  const MAX_LINES = 200;

  const name = $derived(TOOL_DISPLAY_NAMES[tool]);

  type Screen = "where" | "installing" | "failed" | "hooks" | "done";
  let screen = $state<Screen>("where");
  let info = $state<AgentInstallInfo | null>(null);
  /** The folder chosen; null is npm's own. */
  let customDir = $state<string | null>(null);
  let stage = $state<AgentInstallStage>("checking");
  let lines = $state<string[]>([]);
  let logEl: HTMLPreElement | undefined = $state();
  let error = $state("");
  let installedPath = $state("");
  let hooksDone = $state(false);
  let hooksError = $state("");
  let hooksBusy = $state(false);
  let copied = $state(false);
  let dialogEl: HTMLDivElement | undefined = $state();

  const command = $derived(info ? installCommand(info.package, customDir) : "");

  // The output follows its last line, as a console would.
  $effect(() => {
    void lines.length;
    if (logEl) logEl.scrollTop = logEl.scrollHeight;
  });

  onMount(() => {
    dialogEl?.focus();
    void agentInstallInfo(tool)
      .then((found) => (info = found))
      .catch(() => (info = { npm: null, defaultDir: null, package: "" }));
    const stop = onAgentInstallProgress((progress) => {
      if (progress.tool !== tool) return;
      stage = progress.stage;
      if (progress.line) lines = [...lines.slice(-(MAX_LINES - 1)), progress.line];
    });
    return () => void stop.then((unlisten) => unlisten());
  });

  async function pickDir() {
    const picked = await open({
      directory: true,
      title: t("agentInstall.pickTitle"),
      defaultPath: customDir ?? info?.defaultDir ?? undefined,
    });
    if (typeof picked !== "string") return;
    // Picking npm's own folder is picking the default.
    customDir = picked === info?.defaultDir ? null : picked;
  }

  async function install() {
    screen = "installing";
    stage = "checking";
    lines = [];
    error = "";
    try {
      installedPath = (await installAgentCli(tool, customDir)).path;
      // Remembered so Termo keeps finding it, and its consoles get it on
      // their PATH. npm's own folder needs neither.
      appConfigStore.setAgentInstallDir(tool, customDir);
      screen = "hooks";
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      screen = "failed";
    }
  }

  async function setUpHooks() {
    hooksBusy = true;
    hooksError = "";
    try {
      await installAgentHooks(tool);
      hooksDone = true;
      screen = "done";
    } catch (err) {
      hooksError = err instanceof Error ? err.message : String(err);
    } finally {
      hooksBusy = false;
    }
  }

  async function copyCommand() {
    await writeText(command);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function close() {
    // Mid-install it's cancelled first, with its own button.
    if (screen === "installing") return;
    onclose(screen === "hooks" || screen === "done");
  }

  function onKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  }
</script>

<div class="backdrop" transition:fade={{ duration: 120 }}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={t("agentInstall.title", { tool: name })}
    bind:this={dialogEl}
    onkeydown={onKeydown}
    transition:fly={{ y: 12, duration: 160 }}
  >
    <header>
      <ConsoleIcon kind={tool} size={20} />
      <h2>{t("agentInstall.title", { tool: name })}</h2>
    </header>

    {#if info === null}
      <p class="text">{t("agentInstall.loading")}</p>
    {:else if !info.npm}
      <p class="text">{t("agentInstall.needsNode")}</p>
      <div class="actions">
        <button class="button" onclick={() => void openLink("https://nodejs.org/")}>{t("agentInstall.getNode")}</button>
        <button class="button primary" onclick={close}>{t("agentInstall.close")}</button>
      </div>
    {:else if screen === "where"}
      <h3>{t("agentInstall.where")}</h3>
      <div class="folder">
        <span class="folder-path">{customDir ?? info.defaultDir ?? "npm"}</span>
        <span class="folder-kind">{t(customDir ? "agentInstall.customDir" : "agentInstall.defaultDir")}</span>
      </div>
      <div class="folder-actions">
        <button class="button" onclick={() => void pickDir()}>{t("agentInstall.change")}</button>
        {#if customDir}
          <button class="button quiet" onclick={() => (customDir = null)}>{t("agentInstall.useDefault")}</button>
        {/if}
      </div>
      {#if customDir}
        <p class="warning">{t("agentInstall.customWarning", { tool: name, dir: customDir })}</p>
      {/if}
      <h3>{t("agentInstall.command")}</h3>
      <code class="command">{command}</code>
      <div class="actions">
        <button class="button" onclick={close}>{t("agentInstall.cancel")}</button>
        <button class="button primary" onclick={() => void install()}>{t("agentInstall.start")}</button>
      </div>
    {:else if screen === "installing"}
      <!-- What it's doing and what npm says: npm doesn't tell how far along
           it is, so there's no bar to fill. -->
      <p class="working" role="status">
        <span class="spinner" aria-hidden="true"></span>
        {t(STAGE_KEYS[stage])}
      </p>
      <pre class="log live" bind:this={logEl}>{lines.join("\n")}</pre>
      <div class="actions">
        <button class="button" onclick={() => void cancelAgentCliInstall()}>{t("agentInstall.cancel")}</button>
      </div>
    {:else if screen === "failed"}
      <h3>{t("agentInstall.failed")}</h3>
      <pre class="log error" role="alert">{error}</pre>
      <code class="command">{command}</code>
      <div class="actions">
        <button class="button quiet" onclick={() => void copyCommand()}>
          {t(copied ? "agentInstall.copied" : "agentInstall.copy")}
        </button>
        <span class="spacer"></span>
        <button class="button" onclick={close}>{t("agentInstall.close")}</button>
        <button class="button primary" onclick={() => (screen = "where")}>{t("agentInstall.retry")}</button>
      </div>
    {:else if screen === "hooks"}
      <h3>{t("agentInstall.installed", { tool: name })}</h3>
      <p class="path">{installedPath}</p>
      <p class="text">{t("agentInstall.hooksQuestion")}</p>
      <p class="hint">{t("agentInstall.hooksHint", { tool: name })}</p>
      {#if hooksError}
        <p class="warning" role="alert">{t("agentInstall.hooksFailed", { error: hooksError })}</p>
      {/if}
      <div class="actions">
        <button class="button" disabled={hooksBusy} onclick={() => (screen = "done")}>{t("agentInstall.hooksNo")}</button>
        <button class="button primary" disabled={hooksBusy} onclick={() => void setUpHooks()}>
          {t("agentInstall.hooksYes")}
        </button>
      </div>
    {:else}
      <h3>{t("agentInstall.installed", { tool: name })}</h3>
      <p class="path">{installedPath}</p>
      {#if hooksDone}
        <p class="text">{t("agentInstall.doneHooks")}</p>
      {/if}
      <p class="text">{t("agentInstall.doneSignIn", { tool: name })}</p>
      <div class="actions">
        <button class="button primary" onclick={close}>{t("agentInstall.close")}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 500;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
  }

  .dialog {
    width: min(480px, 92%);
    max-height: 88%;
    display: flex;
    flex-direction: column;
    padding: 18px 20px 16px;
    background: var(--tabbar-active-bg-solid, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    overflow-y: auto;
  }

  .dialog:focus {
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  h3 {
    margin: 16px 0 6px;
    font-size: 12px;
    font-weight: 600;
  }

  .text {
    margin: 12px 0 0;
    line-height: 1.5;
  }

  .hint {
    margin: 6px 0 0;
    font-size: 11px;
    line-height: 1.5;
    opacity: 0.65;
  }

  .folder {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
  }

  .folder-path,
  .path {
    font-family: var(--terminal-font, ui-monospace, Consolas, monospace);
    overflow-wrap: anywhere;
  }

  .folder-kind {
    font-size: 11px;
    opacity: 0.6;
  }

  .path {
    margin: 0;
    font-size: 11px;
    opacity: 0.7;
  }

  .folder-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  .warning {
    margin: 10px 0 0;
    padding: 8px 10px;
    border-left: 3px solid #d9a441;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
    line-height: 1.5;
    overflow-wrap: anywhere;
  }

  .command {
    display: block;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.3));
    font-family: var(--terminal-font, ui-monospace, Consolas, monospace);
    font-size: 11px;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .working {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 18px 0 0;
    font-weight: 600;
  }

  .spinner {
    flex-shrink: 0;
    width: 14px;
    height: 14px;
    box-sizing: border-box;
    border: 2px solid var(--tabbar-border, #2a2a2a);
    border-top-color: var(--pane-active-border, #3a6df0);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .log {
    max-height: 180px;
    margin: 8px 0 0;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.3));
    font-family: var(--terminal-font, ui-monospace, Consolas, monospace);
    font-size: 10px;
    line-height: 1.5;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    overflow-y: auto;
    user-select: text;
  }

  /* The same height from the first line to the last, so the dialog doesn't
     grow as npm talks. */
  .log.live {
    height: 150px;
    max-height: none;
    margin-top: 10px;
    opacity: 0.85;
  }

  .log.error {
    margin-bottom: 8px;
    color: #f14c4c;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
    margin-top: 18px;
  }

  .spacer {
    flex: 1;
  }

  .button {
    padding: 6px 14px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }

  .button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .button:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 2px;
  }

  .button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .button.quiet {
    border-color: transparent;
    opacity: 0.75;
  }

  .button.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  /* Slow enough not to pull the eye, still showing it's alive. */
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 4s;
    }
  }
</style>
