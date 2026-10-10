<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { getAgentHooksStatus, installAgentHooks, uninstallAgentHooks } from "$lib/core/agent-hooks-client";
  import {
    installCopilotStatusLine,
    installStatusLine,
    isCopilotStatusLineInstalled,
    isStatusLineInstalled,
    uninstallCopilotStatusLine,
    uninstallStatusLine,
  } from "$lib/core/status-line-client";
  import { SHORTCUT_LABEL_KEYS, type ShortcutAction } from "$lib/core/shortcuts";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import ConsoleIcon from "$lib/agent-launch/ConsoleIcon.svelte";
  import AgentInstallDialog from "$lib/agent-install/AgentInstallDialog.svelte";
  import { marketplaceStatus } from "$lib/core/marketplace-client";
  import { newTabOptions } from "$lib/layout/new-tab-options.svelte";
  import { SOUND_IDS, playSound, type SoundChoice, type SoundEvent } from "$lib/ai-mode/sounds";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import type { ToolId } from "$lib/ai-mode/pattern-table";
  import SettingDropdown from "../SettingDropdown.svelte";
  import ShortcutRecorder from "../ShortcutRecorder.svelte";

  /** Everything Termo sets up for one agent CLI, on its own page. */
  let { tool }: { tool: ToolId } = $props();

  /** What differs from one agent to the next. */
  interface ToolDetails {
    shortcut: ShortcutAction;
    hooksFile: string;
    /** Its status line, for the ones Termo can chain onto. */
    statusLine?: {
      label: MessageKey;
      hint: MessageKey;
      installed: () => Promise<boolean>;
      install: () => Promise<void>;
      uninstall: () => Promise<void>;
    };
    /** What the status bar shows for it without installing anything. */
    statusBarHint?: MessageKey;
    /** For a CLI that can't be handed Termo's agents for one session: how
     * its tabs get them. */
    agentsHint?: MessageKey;
  }

  const DETAILS: Record<ToolId, ToolDetails> = {
    claude: {
      shortcut: "newClaude",
      hooksFile: "~/.claude/settings.json",
      statusLine: {
        label: "preferences.aiMode.statusLine",
        hint: "preferences.aiMode.statusLineHint",
        installed: isStatusLineInstalled,
        install: installStatusLine,
        uninstall: uninstallStatusLine,
      },
    },
    codex: {
      shortcut: "newCodex",
      hooksFile: "~/.codex/hooks.json",
      statusBarHint: "preferences.aiMode.codexStatusBarHint",
    },
    gemini: {
      shortcut: "newGemini",
      hooksFile: "~/.gemini/settings.json",
      statusBarHint: "preferences.aiMode.geminiStatusBarHint",
    },
    copilot: {
      shortcut: "newCopilot",
      hooksFile: "~/.copilot/hooks/termo.json",
      statusLine: {
        label: "preferences.aiMode.copilotStatusLine",
        hint: "preferences.aiMode.copilotStatusLineHint",
        installed: isCopilotStatusLineInstalled,
        install: installCopilotStatusLine,
        uninstall: uninstallCopilotStatusLine,
      },
    },
    grok: {
      shortcut: "newGrok",
      hooksFile: "~/.grok/hooks/termo.json",
      statusBarHint: "preferences.aiMode.grokStatusBarHint",
      agentsHint: "preferences.aiMode.grokAgentsHint",
    },
  };

  const EVENTS: { event: SoundEvent; label: MessageKey }[] = [
    { event: "awaiting", label: "preferences.aiMode.soundAwaiting" },
    { event: "finished", label: "preferences.aiMode.soundFinished" },
  ];

  const details = $derived(DETAILS[tool]);
  const name = $derived(TOOL_DISPLAY_NAMES[tool]);

  let hooks = $state<boolean | null>(null);
  let statusLine = $state<boolean | null>(null);
  let busy = $state<"hooks" | "statusLine" | null>(null);
  let error = $state<string | null>(null);

  /** Where its CLI is, null when it isn't installed; undefined until checked. */
  let cliPath = $state<string | null | undefined>(undefined);
  let installing = $state(false);

  async function refresh() {
    hooks = (await getAgentHooksStatus())[tool];
    statusLine = details.statusLine ? await details.statusLine.installed() : null;
  }

  async function findCli() {
    cliPath = await marketplaceStatus()
      .then((status) => status[`${tool}Cli`])
      .catch(() => undefined);
  }

  onMount(() => {
    void refresh();
    void findCli();
  });

  function installClosed(installed: boolean) {
    installing = false;
    if (!installed) return;
    // It may have set up the hooks on its way out.
    void refresh();
    void findCli();
    // The sidebar's own note of which agents are missing.
    newTabOptions.refresh();
  }

  async function run(what: "hooks" | "statusLine", action: () => Promise<void>) {
    busy = what;
    error = null;
    try {
      await action();
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = null;
    }
  }

  function toggleHooks() {
    void run("hooks", () => (hooks ? uninstallAgentHooks(tool) : installAgentHooks(tool)));
  }

  function toggleStatusLine() {
    const line = details.statusLine;
    if (!line) return;
    void run("statusLine", () => (statusLine ? line.uninstall() : line.install()));
  }

  const soundOptions = $derived(
    (["none", ...SOUND_IDS] as SoundChoice[]).map((id) => ({
      value: id,
      label: t(`preferences.aiMode.soundNames.${id}`),
    })),
  );

  /** Picking a sound plays it, so choosing doesn't need a second click. */
  function pickSound(event: SoundEvent, choice: SoundChoice) {
    appConfigStore.setAiSound(tool, event, choice);
    playSound(choice, appConfigStore.aiSoundVolume);
  }
</script>

<div class="tool-title">
  <ConsoleIcon kind={tool} size={18} />
  <h2>{name}</h2>
</div>

{#if cliPath === null}
  <div class="missing">
    <span>
      {t("agentInstall.notInstalled", { tool: name })}
      <small>{t("agentInstall.notInstalledHint")}</small>
    </span>
    <button class="action-btn primary" onclick={() => (installing = true)}>{t("agentInstall.install")}</button>
  </div>
{:else if cliPath}
  <p class="cli-path">{t("agentInstall.installedAt", { path: cliPath })}</p>
{/if}
{#if installing}
  <AgentInstallDialog {tool} onclose={installClosed} />
{/if}

<!-- Nothing here does anything for a CLI that isn't there. -->
<div class="settings" class:off={cliPath === null} inert={cliPath === null}>
<section>
  <h3>{t("preferences.aiMode.tabs")}</h3>
  <div class="row">
    <span>{t(SHORTCUT_LABEL_KEYS[details.shortcut])}</span>
    <ShortcutRecorder action={details.shortcut} />
  </div>
</section>

<section>
  <h3>{t("preferences.aiMode.hooks")}</h3>
  <div class="row">
    <span>{t("preferences.aiMode.hooksFor", { tool: name })}</span>
    <span class="state" class:on={hooks}>
      {t(hooks ? "preferences.aiMode.installed" : "preferences.aiMode.notInstalled")}
    </span>
    <button class="action-btn" disabled={hooks === null || busy !== null} onclick={toggleHooks}>
      {busy === "hooks" ? "..." : t(hooks ? "preferences.aiMode.remove" : "preferences.aiMode.install")}
    </button>
  </div>
  <p class="setting-hint">{t("preferences.aiMode.hooksHint", { files: details.hooksFile })}</p>
</section>

<section>
  <h3>{t("preferences.aiMode.statusBar")}</h3>
  {#if details.statusLine}
    <div class="row">
      <span>{t(details.statusLine.label)}</span>
      <span class="state" class:on={statusLine}>
        {t(statusLine ? "preferences.aiMode.installed" : "preferences.aiMode.notInstalled")}
      </span>
      <button class="action-btn" disabled={statusLine === null || busy !== null} onclick={toggleStatusLine}>
        {busy === "statusLine" ? "..." : t(statusLine ? "preferences.aiMode.remove" : "preferences.aiMode.install")}
      </button>
    </div>
    <p class="setting-hint">{t(details.statusLine.hint)}</p>
  {:else if details.statusBarHint}
    <p class="setting-hint first">{t(details.statusBarHint)}</p>
  {/if}
  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

{#if details.agentsHint}
  <section>
    <h3>{t("preferences.agents.consoles")}</h3>
    <p class="setting-hint first">{t(details.agentsHint)}</p>
  </section>
{/if}

<section>
  <h3>{t("preferences.aiMode.sounds")}</h3>
  <label class="row toggle-row">
    <span>{t("preferences.aiMode.soundsOnFor", { tool: name })}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiSoundsEnabled[tool]}
      onchange={(e) => appConfigStore.setAiSoundsEnabled(tool, e.currentTarget.checked)}
    />
  </label>
  <!-- Off keeps the picks, just dimmed and out of reach until it's back on. -->
  <div class="sounds" class:off={!appConfigStore.aiSoundsEnabled[tool]} inert={!appConfigStore.aiSoundsEnabled[tool]}>
    {#each EVENTS as { event, label } (event)}
      {@const choice = appConfigStore.aiSounds[tool][event]}
      <div class="row">
        <span>{t(label)}</span>
        <div class="sound-controls">
          <SettingDropdown options={soundOptions} value={choice} onpick={(value) => pickSound(event, value as SoundChoice)} />
          <button
            class="preview-btn"
            title={t("preferences.aiMode.soundPreview")}
            aria-label={t("preferences.aiMode.soundPreview")}
            disabled={choice === "none" || appConfigStore.aiSoundVolume === 0}
            onclick={() => playSound(choice, appConfigStore.aiSoundVolume)}
          >
            <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
              <path d="M3 1.5v9l7.5-4.5z" fill="currentColor" />
            </svg>
          </button>
        </div>
      </div>
    {/each}
  </div>
  <p class="setting-hint">{t("preferences.aiMode.soundsToolHint")}</p>
</section>
</div>

<style>
  .tool-title {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
  }

  .missing {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 18px;
    padding: 10px 12px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    font-size: 12px;
  }

  .missing small {
    display: block;
    margin-top: 2px;
    font-size: 10px;
    opacity: 0.65;
  }

  .action-btn.primary {
    flex-shrink: 0;
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .cli-path {
    margin: -8px 0 16px;
    font-size: 10px;
    opacity: 0.6;
    overflow-wrap: anywhere;
  }

  .settings {
    transition: opacity 120ms ease;
  }

  .settings.off {
    opacity: 0.4;
  }

  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  section {
    margin-bottom: 20px;
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
    margin-bottom: 6px;
  }

  .row > span:first-child {
    flex: 1;
  }

  .toggle-row {
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .state {
    font-size: 11px;
    opacity: 0.6;
  }

  .state.on {
    opacity: 1;
    color: var(--pane-active-border, #3a6df0);
  }

  .action-btn {
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .action-btn:hover:not(:disabled) {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .sounds {
    transition: opacity 120ms ease;
  }

  .sounds.off {
    opacity: 0.4;
  }

  .sounds .row {
    margin-bottom: 8px;
  }

  .sound-controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .preview-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    padding: 0;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .preview-btn:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .preview-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .setting-hint.first {
    margin-top: 0;
  }

  .error {
    font-size: 11px;
    color: #f14c4c;
  }
</style>
