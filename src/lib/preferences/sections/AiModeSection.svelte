<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import {
    getAgentHooksStatus,
    installAgentHooks,
    uninstallAgentHooks,
    type AgentHooksStatus,
    type AgentTool,
  } from "$lib/core/agent-hooks-client";
  import { t } from "$lib/i18n/i18n.svelte";

  const TOOLS: { tool: AgentTool; label: string; file: string }[] = [
    { tool: "claude", label: "Claude Code", file: "~/.claude/settings.json" },
    { tool: "codex", label: "Codex", file: "~/.codex/hooks.json" },
  ];

  let hooks = $state<AgentHooksStatus | null>(null);
  let busy = $state<AgentTool | null>(null);
  let error = $state<string | null>(null);

  async function refresh() {
    hooks = await getAgentHooksStatus();
  }

  onMount(() => {
    void refresh();
  });

  async function toggleHooks(tool: AgentTool, installed: boolean) {
    busy = tool;
    error = null;
    try {
      await (installed ? uninstallAgentHooks(tool) : installAgentHooks(tool));
      await refresh();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = null;
    }
  }
</script>

<section>
  <h3>{t("preferences.aiMode.title")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.detect")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiModeEnabled}
      onchange={(e) => appConfigStore.setAiModeEnabled(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.aiMode.detectHint")}</p>
</section>

<section>
  <h3>{t("preferences.aiMode.notices")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.notifyAwaiting")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiNotifyAwaiting}
      onchange={(e) => appConfigStore.setAiNotifyAwaiting(e.currentTarget.checked)}
    />
  </label>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.notifyFinished")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiNotifyFinished}
      onchange={(e) => appConfigStore.setAiNotifyFinished(e.currentTarget.checked)}
    />
  </label>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.notifyError")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiNotifyError}
      onchange={(e) => appConfigStore.setAiNotifyError(e.currentTarget.checked)}
    />
  </label>
  <label class="toggle-row">
    <span>{t("preferences.aiMode.dockBadge")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.aiDockBadge}
      onchange={(e) => appConfigStore.setAiDockBadge(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.aiMode.noticesHint")}</p>
</section>

<section>
  <h3>{t("preferences.aiMode.hooks")}</h3>
  {#each TOOLS as { tool, label } (tool)}
      {@const installed = hooks?.[tool] ?? false}
      <div class="hook-row">
        <span>{label}</span>
        <span class="hook-state" class:on={installed}>
          {t(installed ? "preferences.aiMode.installed" : "preferences.aiMode.notInstalled")}
        </span>
        <button class="action-btn" disabled={!hooks || busy !== null} onclick={() => toggleHooks(tool, installed)}>
          {busy === tool ? "..." : t(installed ? "preferences.aiMode.remove" : "preferences.aiMode.install")}
        </button>
      </div>
    {/each}
    <p class="setting-hint">
      {t("preferences.aiMode.hooksHint", { claudeFile: TOOLS[0].file, codexFile: TOOLS[1].file })}
    </p>
  {#if error}
    <p class="error">{error}</p>
  {/if}
</section>

<style>
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

  .toggle-row,
  .hook-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 6px;
  }

  .toggle-row {
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .hook-row span:first-child {
    flex: 1;
  }

  .hook-state {
    font-size: 11px;
    opacity: 0.6;
  }

  .hook-state.on {
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

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .error {
    font-size: 11px;
    color: #f14c4c;
  }
</style>
