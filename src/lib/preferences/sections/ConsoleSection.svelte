<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { listShells } from "$lib/core/pty-client";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { ShellInfo } from "$lib/core/types";
  import { t } from "$lib/i18n/i18n.svelte";
  import { sessionsDaemon } from "$lib/core/sessions-daemon.svelte";
  import SettingDropdown from "../SettingDropdown.svelte";

  let shells = $state<ShellInfo[]>([]);
  const shellOptions = $derived(shells.map((shell) => ({ value: shell.id, label: shell.display_name })));

  onMount(() => {
    void listShells().then((list) => (shells = list));
    return sessionsDaemon.start();
  });

  // themeStore setters never persist themselves (kept decoupled from
  // appConfigStore) — each Preferences section that mutates a themeStore
  // field owns persisting the fields it actually touches, same pattern as
  // AppearanceSection.
  $effect(() => {
    void themeStore.outputHighlightingEnabled;
    // untrack: persist() reads every config field synchronously, which would
    // otherwise make this effect re-run (and re-save) on any config change.
    if (appConfigStore.loaded) untrack(() => void appConfigStore.persist());
  });
</script>

<section>
  <h3>{t("preferences.console.defaultShell")}</h3>
  {#if shells.length === 0}
    <p class="setting-hint">{t("preferences.console.searching")}</p>
  {:else}
    <!-- A combo, not a list: some systems have a dozen consoles. -->
    <div class="row">
      <span>{t("preferences.console.defaultShellLabel")}</span>
      <SettingDropdown
        options={shellOptions}
        value={appConfigStore.defaultShellId ?? shells[0].id}
        onpick={(id) => appConfigStore.setDefaultShell(id)}
      />
    </div>
    <p class="setting-hint">{t("preferences.console.defaultShellHint")}</p>
  {/if}
</section>

<section>
  <h3>{t("preferences.console.output")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.console.highlight")}</span>
    <input
      type="checkbox"
      checked={themeStore.outputHighlightingEnabled}
      onchange={(e) => themeStore.setOutputHighlighting(e.currentTarget.checked)}
    />
  </label>
</section>

<section>
  <h3>{t("preferences.console.sessions")}</h3>
  <label class="toggle-row">
    <span>{t("preferences.console.keepRunning")}</span>
    <input
      type="checkbox"
      checked={appConfigStore.persistentSessions}
      onchange={(e) => appConfigStore.setPersistentSessions(e.currentTarget.checked)}
    />
  </label>
  <p class="setting-hint">{t("preferences.console.keepRunningHint")}</p>
  {#if !sessionsDaemon.answering && appConfigStore.persistentSessions}
    <div class="daemon-down" role="status">
      <strong>{t("preferences.console.daemonDown")}</strong>
      <p>{t("preferences.console.daemonDownHint")}</p>
      <button class="restart" disabled={sessionsDaemon.restarting} onclick={() => void sessionsDaemon.restart()}>
        {sessionsDaemon.restarting ? t("preferences.console.restartingDaemon") : t("preferences.console.restartDaemon")}
      </button>
      {#if sessionsDaemon.restartError}
        <p class="error">{t("preferences.console.restartFailed", { error: sessionsDaemon.restartError })}</p>
      {/if}
    </div>
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

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
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

  .daemon-down {
    margin-top: 12px;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, #e3b341 55%, transparent);
    background: color-mix(in srgb, #e3b341 10%, transparent);
    font-size: 12px;
  }

  .daemon-down strong {
    color: #e3b341;
  }

  .daemon-down p {
    margin: 4px 0 10px;
    font-size: 11px;
    line-height: 1.4;
    opacity: 0.8;
  }

  .daemon-down .error {
    margin: 8px 0 0;
    color: #f85149;
    opacity: 1;
  }

  .restart {
    padding: 6px 12px;
    border: none;
    border-radius: 6px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .restart:hover:not(:disabled) {
    filter: brightness(1.12);
  }

  .restart:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
