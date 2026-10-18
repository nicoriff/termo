<script lang="ts">
  import { onMount } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import {
    answerRemotePending,
    disconnectAllRemote,
    remoteOverview,
    setRemoteAgentsEdit,
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

  /** The state the checkbox was just set to, while that's being applied:
   * shown at once, with a spinner, rather than the old one until it's done. */
  let turning = $state<boolean | null>(null);

  async function setEnabled(enabled: boolean) {
    turning = enabled;
    try {
      await setRemoteEnabled(enabled);
      await refresh();
    } catch {
      await refresh();
    } finally {
      turning = null;
    }
  }

  const running = $derived(tailscaleRunning(overview));
  const config = $derived(overview?.config ?? null);

  /** Where the web client is: by this computer's short name on the tailnet,
   * the easiest to type — and its full name too, for a device that doesn't
   * use Tailscale's DNS for short names. Without MagicDNS, only by address.
   * The page answers under all of them (see src-tauri/src/mux/web.rs). */
  const webUrls = $derived.by(() => {
    const tailscale = overview?.tailscale;
    const me = tailscale?.me;
    if (!config?.enabled || !running || !me) return null;
    const at = (host: string) => `http://${host}:${config.port}`;
    if (!tailscale.magicDns || !me.name) return me.ip ? { main: at(me.ip), full: null } : null;
    return { main: at(me.name), full: me.dnsName && me.dnsName !== me.name ? at(me.dnsName) : null };
  });
  const webUrl = $derived(webUrls?.main ?? null);

  let copied = $state(false);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function copyUrl() {
    if (!webUrl) return;
    try {
      await navigator.clipboard.writeText(webUrl);
      copied = true;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copied = false), 1600);
    } catch {
      // Nothing to do: the address is on screen, selectable.
    }
  }

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
    <span class="enable-label">
      {t("preferences.remote.enable")}
      {#if turning !== null}
        <span class="turning" role="status">
          <span class="spinner" aria-hidden="true"></span>
          {turning ? t("preferences.remote.enabling") : t("preferences.remote.disabling")}
        </span>
      {/if}
    </span>
    <input
      type="checkbox"
      checked={turning ?? config?.enabled ?? false}
      disabled={working || turning !== null || !overview || (!config?.enabled && !running)}
      onchange={(e) => void setEnabled(e.currentTarget.checked)}
    />
  </label>
  {#if overview?.tailscale && running}
    <p class="setting-hint">
      {t("preferences.remote.thisComputer", { name: overview.tailscale.me.name, ip: overview.tailscale.me.ip ?? "" })}
    </p>
  {/if}
  {#if config?.enabled && !appConfigStore.persistentSessions}
    <p class="setting-hint">{t("preferences.remote.notKeptHint")}</p>
  {/if}
  {#if webUrl}
    <div class="web-url">
      <span class="web-url-title">{t("preferences.remote.webClientTitle")}</span>
      <div class="web-url-row">
        <code class="web-url-text">{webUrl}</code>
        <button class="copy" class:done={copied} onclick={() => void copyUrl()}>
          {copied ? `✓ ${t("preferences.remote.copiedUrl")}` : t("preferences.remote.copyUrl")}
        </button>
      </div>
      <span class="web-url-hint">{t("preferences.remote.webClientHint")}</span>
      {#if webUrls?.full}
        <span class="web-url-hint">
          {t("preferences.remote.webClientFull")}
          <code class="web-url-full">{webUrls.full}</code>
        </span>
      {/if}
    </div>
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

  <section>
    <h3>{t("preferences.remote.agents")}</h3>
    <p class="setting-hint first">{t("preferences.remote.agentsHint")}</p>
    <label class="toggle-row">
      <span class="enable-label">{t("preferences.remote.agentsEdit")}</span>
      <input
        type="checkbox"
        checked={config.agentsEdit}
        disabled={working}
        onchange={(e) => {
          const enabled = e.currentTarget.checked;
          void change(() => setRemoteAgentsEdit(enabled));
        }}
      />
    </label>
    <p class="setting-hint">{t("preferences.remote.agentsEditHint")}</p>
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

  .enable-label {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .turning {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    opacity: 0.75;
  }

  .spinner {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, currentColor 25%, transparent);
    border-top-color: var(--pane-active-border, #3a6df0);
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 2s;
    }
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

  /* The address to open on a phone: the one thing here people come back
     for, so it stands out. */
  .web-url {
    margin: 12px 0 4px;
    padding: 12px;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, var(--pane-active-border, #3a6df0) 55%, transparent);
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 10%, transparent);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .web-url-title {
    font-size: 12px;
    font-weight: 600;
  }

  .web-url-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .web-url-text {
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    border-radius: 6px;
    background: var(--tabbar-bg, #181818);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    font-family: "Cascadia Code", Consolas, ui-monospace, monospace;
    font-size: 14px;
    overflow-wrap: anywhere;
    user-select: text;
    cursor: text;
  }

  .copy {
    flex-shrink: 0;
    min-width: 84px;
    padding: 7px 12px;
    border: none;
    border-radius: 6px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: filter 120ms ease;
  }

  .copy:hover {
    filter: brightness(1.12);
  }

  .copy.done {
    background: #2ea043;
  }

  .web-url-full {
    font-size: 11px;
    user-select: all;
    word-break: break-all;
  }

  .web-url-hint {
    font-size: 11px;
    line-height: 1.4;
    opacity: 0.7;
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
