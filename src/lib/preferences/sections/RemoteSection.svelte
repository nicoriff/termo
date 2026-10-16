<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import {
    answerRemotePending,
    disconnectAllRemote,
    remoteOverview,
    setRemoteAllowed,
    setRemoteEnabled,
    tailscaleRunning,
    type RemoteAccess,
    type RemoteAllowed,
    type RemoteOverview,
  } from "$lib/core/remote-client";
  import { t } from "$lib/i18n/i18n.svelte";

  /** How often it looks again while open: who connects, who's waiting. */
  const REFRESH_MS = 3000;

  let overview = $state<RemoteOverview | null>(null);
  let working = $state(false);

  async function refresh() {
    overview = await remoteOverview().catch(() => overview);
  }

  onMount(() => {
    void refresh();
    const timer = setInterval(() => void refresh(), REFRESH_MS);
    return () => clearInterval(timer);
  });

  /** Runs a change, then shows where things stand. */
  async function change(action: () => Promise<unknown>) {
    working = true;
    try {
      await action();
    } finally {
      await refresh();
      working = false;
    }
  }

  const running = $derived(tailscaleRunning(overview));
  const config = $derived(overview?.config ?? null);

  function setAccess(index: number, access: RemoteAccess) {
    if (!config) return;
    const allowed = config.allowed.map((entry, i) => (i === index ? { ...entry, access } : entry));
    void change(() => setRemoteAllowed(allowed));
  }

  function remove(index: number) {
    if (!config) return;
    const allowed: RemoteAllowed[] = config.allowed.filter((_, i) => i !== index);
    void change(() => setRemoteAllowed(allowed));
  }
</script>

<section>
  <h3>{t("preferences.sidebar.remote")}</h3>
  <p class="setting-hint first">{t("preferences.remote.intro")}</p>
  {#if overview && !overview.tailscaleInstalled}
    <p class="notice">{t("preferences.remote.noTailscale")}</p>
  {:else if overview && !running}
    <p class="notice">{t("preferences.remote.tailscaleDown")}</p>
  {/if}
  <label class="toggle-row">
    <span>{t("preferences.remote.enable")}</span>
    <input
      type="checkbox"
      checked={config?.enabled ?? false}
      disabled={working || !overview || (!config?.enabled && !running)}
      onchange={(e) => {
        const enabled = e.currentTarget.checked;
        void change(() => setRemoteEnabled(enabled));
      }}
    />
  </label>
  {#if overview?.tailscale && running}
    <p class="setting-hint">
      {t("preferences.remote.thisComputer", { name: overview.tailscale.me.name, ip: overview.tailscale.me.ip ?? "" })}
    </p>
  {/if}
  {#if config?.enabled && !appConfigStore.persistentSessions}
    <p class="notice">{t("preferences.remote.needsKeepRunning")}</p>
  {/if}
  {#if config?.enabled && overview?.tailscale && running}
    <p class="setting-hint">
      {t("preferences.remote.webClient", {
        url: `http://${overview.tailscale.me.dnsName || overview.tailscale.me.ip}:${config.port}`,
      })}
    </p>
  {/if}
</section>

{#if config?.enabled}
  <section>
    <h3>{t("preferences.remote.allowed")}</h3>
    <ul class="people">
      {#each config.allowed as entry, index (`${entry.login}|${entry.node ?? ""}`)}
        <li>
          <span class="who">
            <span class="login">{entry.login}</span>
            <span class="node">
              {entry.node ? t("preferences.remote.fromComputer", { node: entry.node }) : t("preferences.remote.anyComputer")}
            </span>
          </span>
          <select
            value={entry.access}
            disabled={working}
            aria-label={entry.login}
            onchange={(e) => setAccess(index, e.currentTarget.value as RemoteAccess)}
          >
            <option value="full">{t("preferences.remote.full")}</option>
            <option value="readOnly">{t("preferences.remote.readOnly")}</option>
          </select>
          <button class="small" disabled={working} onclick={() => remove(index)}>{t("preferences.remote.remove")}</button>
        </li>
      {:else}
        <li class="empty">{t("preferences.remote.noOneAllowed")}</li>
      {/each}
    </ul>
  </section>

  {#if overview && overview.pending.length > 0}
    <section>
      <h3>{t("preferences.remote.pending")}</h3>
      <p class="setting-hint first">{t("preferences.remote.pendingHint")}</p>
      <ul class="people">
        {#each overview.pending as entry (`${entry.login}|${entry.node}`)}
          <li>
            <span class="who">
              <span class="login">{entry.login}</span>
              <span class="node">{t("preferences.remote.fromComputer", { node: entry.node })}</span>
            </span>
            <button
              class="small primary"
              disabled={working}
              onclick={() => void change(() => answerRemotePending(entry.login, entry.node, "full"))}
            >
              {t("preferences.remote.allow")}
            </button>
            <button
              class="small"
              disabled={working}
              onclick={() => void change(() => answerRemotePending(entry.login, entry.node, "readOnly"))}
            >
              {t("preferences.remote.allowReadOnly")}
            </button>
            <button
              class="small"
              disabled={working}
              onclick={() => void change(() => answerRemotePending(entry.login, entry.node, null))}
            >
              {t("preferences.remote.deny")}
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <section>
    <h3>{t("preferences.remote.connected")}</h3>
    <ul class="people">
      {#each overview?.viewers ?? [] as viewer (`${viewer.login}|${viewer.node}|${viewer.since}`)}
        <li>
          <span class="who">
            <span class="login">{viewer.login}</span>
            <span class="node">
              {t("preferences.remote.fromComputer", { node: viewer.node })} ·
              {t(viewer.access === "full" ? "preferences.remote.full" : "preferences.remote.readOnly")} ·
              {t("preferences.remote.showing", { count: String(viewer.sessions.length) })}
            </span>
          </span>
        </li>
      {:else}
        <li class="empty">{t("preferences.remote.nobodyConnected")}</li>
      {/each}
    </ul>
    {#if (overview?.viewers.length ?? 0) > 0}
      <button class="small danger" disabled={working} onclick={() => void change(() => disconnectAllRemote())}>
        {t("preferences.remote.disconnectAll")}
      </button>
    {/if}
  </section>
{/if}

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

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
    margin-bottom: 6px;
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

  .setting-hint.first {
    margin: 0 0 8px;
    line-height: 1.5;
  }

  .notice {
    margin: 0 0 8px;
    padding: 6px 8px;
    border-radius: 5px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    font-size: 11px;
    line-height: 1.4;
  }

  .people {
    list-style: none;
    margin: 0 0 8px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .people li {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }

  .people li.empty {
    opacity: 0.6;
    font-size: 11px;
  }

  .who {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .login {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .node {
    font-size: 10px;
    opacity: 0.6;
  }

  select {
    font: inherit;
    font-size: 11px;
    color: inherit;
    background: transparent;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 5px;
    padding: 2px 4px;
  }

  select option {
    background: var(--tabbar-bg, #1e1e1e);
  }

  .small {
    flex-shrink: 0;
    padding: 3px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .small.primary {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .small.danger {
    border-color: #c0504d;
  }

  .small:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
