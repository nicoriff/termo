<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { openLink } from "$lib/core/links";
  import { encode } from "uqr";
  import { t } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { openPreferencesWindow } from "$lib/core/window-client";
  import { collectTiles } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import {
    answerRemotePending,
    connectTailscale,
    disconnectAllRemote,
    disconnectTailscale,
    remoteOverview,
    setRemoteEnabled,
    tailscaleRunning,
    type RemoteAccess,
    type RemoteOverview,
    type RemotePending,
  } from "$lib/core/remote-client";

  /** How often it looks again: rarely while closed (the icon only), often
   * while open, where someone may be waiting for an answer. */
  const CLOSED_MS = 10_000;
  const OPEN_MS = 3000;
  const PANEL_WIDTH = 330;
  const MARGIN = 8;
  /** What to run to let this user control Tailscale, on Linux. */
  const OPERATOR_COMMAND = "sudo tailscale set --operator=$USER";

  let overview = $state<RemoteOverview | null>(null);
  let open = $state(false);
  let buttonEl = $state<HTMLButtonElement | undefined>(undefined);
  /** An action in flight: its buttons wait. */
  let busy = $state(false);
  let message = $state<{ text: string; error: boolean } | null>(null);
  /** Tailscale was asked to connect or disconnect, and hasn't got there yet. */
  let switching = $state<"connecting" | "disconnecting" | null>(null);
  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function refresh() {
    overview = await remoteOverview().catch(() => overview);
  }

  function schedule() {
    clearTimeout(timer);
    timer = setTimeout(
      () => {
        void refresh().then(schedule);
      },
      open ? OPEN_MS : CLOSED_MS,
    );
  }

  onMount(() => {
    void refresh();
    schedule();
  });

  onDestroy(() => {
    clearTimeout(timer);
    clearTimeout(copiedTimer);
  });

  type TailscaleState = "missing" | "notAnswering" | "stopped" | "needsLogin" | "starting" | "connected";

  const tailscale = $derived.by<TailscaleState | null>(() => {
    if (!overview) return null;
    if (!overview.tailscaleInstalled) return "missing";
    const status = overview.tailscale;
    if (!status) return "notAnswering";
    if (tailscaleRunning(overview)) return "connected";
    if (status.backend === "NeedsLogin" || status.backend === "NeedsMachineAuth") return "needsLogin";
    if (status.backend === "Starting") return "starting";
    return "stopped";
  });
  const connected = $derived(tailscale === "connected");
  const enabled = $derived(overview?.config.enabled ?? false);
  const viewers = $derived(enabled ? (overview?.viewers ?? []) : []);
  const pending = $derived(enabled ? (overview?.pending ?? []) : []);

  /** Where the web client is, as Preferences shows it (see RemoteSection):
   * by this computer's short name, and its full one. */
  const urls = $derived.by(() => {
    const status = overview?.tailscale;
    const me = status?.me;
    if (!enabled || !connected || !me || !overview) return null;
    const at = (host: string) => `http://${host}:${overview!.config.port}`;
    if (!status.magicDns || !me.name) return me.ip ? { main: at(me.ip), full: null } : null;
    return { main: at(me.name), full: me.dnsName && me.dnsName !== me.name ? at(me.dnsName) : null };
  });

  /** The QR has the full name: a phone that doesn't use Tailscale's DNS for
   * short names still finds it. Drawn here, offline: it's just the address. */
  const qr = $derived.by(() => {
    const url = urls ? (urls.full ?? urls.main) : null;
    if (!url) return null;
    const code = encode(url, { ecc: "M", border: 2 });
    let path = "";
    code.data.forEach((row, y) =>
      row.forEach((dark, x) => {
        if (dark) path += `M${x},${y}h1v1h-1z`;
      }),
    );
    return { size: code.size, path };
  });

  const tailscaleLine = $derived(
    tailscale === null
      ? ""
      : t(
          {
            missing: "titlebar.remote.tsMissing",
            notAnswering: "titlebar.remote.tsNotAnswering",
            stopped: "titlebar.remote.tsStopped",
            needsLogin: "titlebar.remote.tsNeedsLogin",
            starting: "titlebar.remote.tsStarting",
            connected: "titlebar.remote.tsConnected",
          }[tailscale] as Parameters<typeof t>[0],
        ),
  );

  const tooltip = $derived(
    [
      t("titlebar.remote.label"),
      `${t("titlebar.remote.tailscale")}: ${tailscaleLine}`,
      t(enabled ? "titlebar.remote.accessOn" : "titlebar.remote.accessOff"),
      ...(viewers.length ? [t("remote.viewers", { count: String(viewers.length) })] : []),
      ...(pending.length ? [t("titlebar.remote.pendingCount", { count: String(pending.length) })] : []),
    ].join(" · "),
  );

  const pos = $derived.by(() => {
    if (!buttonEl || !open) return { top: 0, left: 0 };
    const box = buttonEl.getBoundingClientRect();
    return {
      top: box.bottom + 2,
      left: Math.max(MARGIN, Math.min(box.right - PANEL_WIDTH, window.innerWidth - PANEL_WIDTH - MARGIN)),
    };
  });

  function toggle() {
    open = !open;
    if (open) {
      message = null;
      void refresh();
    }
    schedule();
  }

  function close() {
    open = false;
    schedule();
  }

  function errorText(err: unknown): string {
    const reason = String(err);
    if (reason === "permission") return t("titlebar.remote.errPermission", { command: OPERATOR_COMMAND });
    if (reason === "not-installed") return t("titlebar.remote.tsMissing");
    return t("titlebar.remote.errFailed", { error: reason });
  }

  /** Runs an action, then shows where things stand. */
  async function act(action: () => Promise<unknown>) {
    busy = true;
    try {
      await action();
    } catch (err) {
      message = { text: errorText(err), error: true };
    } finally {
      await refresh();
      busy = false;
    }
  }

  /** Asks again until Tailscale shows `done`, for a few seconds: it can take
   * a moment after the command returns. */
  async function settle(done: () => boolean) {
    for (let i = 0; i < 16 && !done(); i++) {
      await new Promise((resolve) => setTimeout(resolve, 500));
      await refresh();
    }
  }

  function connectNow() {
    message = null;
    switching = "connecting";
    void act(async () => {
      const result = await connectTailscale();
      if (result.kind === "login") {
        await openLink(result.url);
        message = { text: t("titlebar.remote.loginOpened"), error: false };
        return;
      }
      await refresh();
      await settle(() => tailscale === "connected");
    }).finally(() => (switching = null));
  }

  function disconnectNow() {
    message = null;
    switching = "disconnecting";
    void act(async () => {
      await disconnectTailscale();
      await refresh();
      await settle(() => tailscale !== "connected");
    }).finally(() => (switching = null));
  }

  function answer(entry: RemotePending, access: RemoteAccess | null) {
    void act(() => answerRemotePending(entry.login, entry.node, access));
  }

  async function copyUrl() {
    if (!urls) return;
    try {
      await navigator.clipboard.writeText(urls.main);
      copied = true;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copied = false), 1600);
    } catch {
      // The address is on screen, selectable.
    }
  }

  function connectOther() {
    close();
    const root = workspaceStore.root;
    const tileId = workspaceStore.focusedTileId ?? (root ? collectTiles(root)[0]?.id : undefined);
    if (tileId) uiState.openRemoteConnect(tileId);
  }

  function settings() {
    close();
    void openPreferencesWindow("remote");
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.stopPropagation();
      close();
      buttonEl?.focus();
    }
  }
</script>

<button
  class="remote"
  class:dim={!enabled || !connected}
  aria-label={t("titlebar.remote.label")}
  aria-haspopup="dialog"
  aria-expanded={open}
  title={`${tooltip}\n${t("titlebar.remote.hint")}`}
  bind:this={buttonEl}
  onclick={toggle}
>
  <!-- An antenna sending: its waves, and a slash through it while Tailscale
       is down. Centered on (8, 8) like the other buttons' glyphs. -->
  <svg viewBox="0 0 16 16" width="17" height="17" aria-hidden="true">
    <circle cx="8" cy="8" r="1.4" fill="currentColor" />
    <path d="M5.4 5.4a3.7 3.7 0 0 0 0 5.2M10.6 5.4a3.7 3.7 0 0 1 0 5.2" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" />
    <path d="M3.3 3.3a6.6 6.6 0 0 0 0 9.4M12.7 3.3a6.6 6.6 0 0 1 0 9.4" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" />
    {#if tailscale !== null && !connected}
      <path d="M2.5 13.5L13.5 2.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
    {/if}
  </svg>
  {#if pending.length > 0}
    <span class="badge pending" aria-hidden="true"></span>
  {:else if viewers.length > 0}
    <span class="badge count" aria-hidden="true">{viewers.length}</span>
  {:else if enabled && connected}
    <span class="badge on" aria-hidden="true"></span>
  {/if}
</button>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="backdrop" onpointerdown={close}></div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="panel"
    style="top:{pos.top}px; left:{pos.left}px; width:{PANEL_WIDTH}px"
    role="dialog"
    aria-label={t("titlebar.remote.label")}
    tabindex="-1"
    onkeydown={onKeydown}
  >
    <div class="panel-title">{t("titlebar.remote.label")}</div>

    <!-- Tailscale: whether this computer is on the tailnet, and the way to
         put it on or take it off. -->
    <div class="block">
      <div class="row">
        <span class="row-title">{t("titlebar.remote.tailscale")}</span>
        <!-- Its state as a pill: green connected, amber on its way or
             waiting for a sign-in, red off, grey not there at all. -->
        {#if tailscale !== null}
          <span
            class="pill"
            class:ok={connected && !switching}
            class:warn={switching !== null || tailscale === "needsLogin" || tailscale === "starting"}
            class:off={!switching && (tailscale === "stopped" || tailscale === "notAnswering")}
          >
            <span class="pill-dot" aria-hidden="true"></span>
            {switching === "connecting"
              ? t("titlebar.remote.tsStarting")
              : switching === "disconnecting"
                ? t("titlebar.remote.tsStopping")
                : tailscale === "missing"
                  ? t("titlebar.remote.tsMissingShort")
                  : tailscale === "notAnswering"
                    ? t("titlebar.remote.tsNotAnsweringShort")
                    : tailscaleLine}
          </span>
        {/if}
        <span class="spacer"></span>
        {#if tailscale === "missing"}
          <button
            class="icon-button"
            title={t("titlebar.remote.download")}
            aria-label={t("titlebar.remote.download")}
            onclick={() => void openLink("https://tailscale.com/download")}
          >
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
              <path d="M8 2.5v8M4.8 7.5L8 10.7l3.2-3.2M3 13.5h10" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
        {:else if tailscale !== null}
          {@const label = switching
            ? t(switching === "connecting" ? "titlebar.remote.tsStarting" : "titlebar.remote.tsStopping")
            : connected
              ? t("titlebar.remote.disconnect")
              : tailscale === "needsLogin"
                ? t("titlebar.remote.signIn")
                : t("titlebar.remote.connect")}
          <!-- Power: connects when off, disconnects when on; a spinner while
               Tailscale gets there. -->
          <button
            class="icon-button power"
            class:on={connected}
            class:switching={switching !== null}
            disabled={busy}
            title={label}
            aria-label={label}
            aria-busy={switching !== null}
            onclick={connected ? disconnectNow : connectNow}
          >
            {#if switching}
              <svg class="spinner" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
                <circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" stroke-opacity="0.25" stroke-width="1.6" />
                <path d="M8 2.5a5.5 5.5 0 0 1 5.5 5.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
              </svg>
            {:else}
              <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
                <path d="M8 2v5.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
                <path d="M5 4.2a5 5 0 1 0 6 0" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
              </svg>
            {/if}
          </button>
        {/if}
      </div>
      {#if connected && overview?.tailscale}
        <span class="row-sub">
          {t("titlebar.remote.tsAs", {
            login: overview.tailscale.me.login ?? "",
            name: overview.tailscale.me.name,
            ip: overview.tailscale.me.ip ?? "",
          })}
        </span>
      {:else if tailscale === "missing" || tailscale === "notAnswering"}
        <span class="row-sub">{tailscaleLine}</span>
      {/if}
      {#if message}
        <p class="note" class:error={message.error}>{message.text}</p>
      {/if}
    </div>

    <!-- Remote access itself. -->
    <label class="block row toggle">
      <span class="row-main">
        <span class="row-title">{t("preferences.remote.enable")}</span>
        {#if !connected && !enabled}
          <span class="row-sub">{t("titlebar.remote.accessNeedsTailscale")}</span>
        {/if}
      </span>
      <input
        type="checkbox"
        checked={enabled}
        disabled={busy || !overview || (!enabled && !connected)}
        onchange={(e) => {
          const on = e.currentTarget.checked;
          void act(() => setRemoteEnabled(on));
        }}
      />
    </label>

    {#if urls}
      <div class="block">
        <span class="section">{t("preferences.remote.webClientTitle")}</span>
        <div class="url-row">
          <code class="url">{urls.main}</code>
          <button class="small" class:done={copied} onclick={() => void copyUrl()}>
            {copied ? `✓ ${t("preferences.remote.copiedUrl")}` : t("preferences.remote.copyUrl")}
          </button>
        </div>
        {#if qr}
          <div class="qr-row">
            <svg class="qr" viewBox="0 0 {qr.size} {qr.size}" shape-rendering="crispEdges" role="img" aria-label={urls.full ?? urls.main}>
              <rect width={qr.size} height={qr.size} fill="#fff" />
              <path d={qr.path} fill="#000" />
            </svg>
            <span class="note">{t("titlebar.remote.qrHint")}</span>
          </div>
        {/if}
      </div>
    {/if}

    {#if pending.length > 0}
      <div class="block">
        <span class="section">{t("preferences.remote.pending")}</span>
        {#each pending as entry (`${entry.login}|${entry.node}`)}
          <div class="person">
            <span class="row-main">
              <span class="row-title">{entry.login}</span>
              <span class="row-sub">{t("preferences.remote.fromComputer", { node: entry.node })}</span>
            </span>
            <div class="person-actions">
              <button class="small primary" disabled={busy} onclick={() => answer(entry, "full")}>
                {t("preferences.remote.allow")}
              </button>
              <button class="small" disabled={busy} onclick={() => answer(entry, "readOnly")}>
                {t("preferences.remote.allowReadOnly")}
              </button>
              <button class="small" disabled={busy} onclick={() => answer(entry, null)}>
                {t("preferences.remote.deny")}
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    {#if enabled}
      <div class="block">
        <span class="section">{t("preferences.remote.connected")}</span>
        {#each viewers as viewer (`${viewer.login}|${viewer.node}|${viewer.since}`)}
          <div class="person">
            <span class="row-main">
              <span class="row-title">{viewer.login}</span>
              <span class="row-sub">
                {t("preferences.remote.fromComputer", { node: viewer.node })} ·
                {t(viewer.access === "full" ? "preferences.remote.full" : "preferences.remote.readOnly")} ·
                {t("preferences.remote.showing", { count: String(viewer.sessions.length) })}
              </span>
            </span>
          </div>
        {:else}
          <p class="note">{t("preferences.remote.nobodyConnected")}</p>
        {/each}
        {#if viewers.length > 0}
          <button class="small danger wide" disabled={busy} onclick={() => void act(() => disconnectAllRemote())}>
            {t("preferences.remote.disconnectAll")}
          </button>
        {/if}
      </div>
    {/if}

    <div class="divider"></div>
    <button class="item" disabled={!connected} onclick={connectOther}>{t("titlebar.remote.connectOther")}</button>
    <button class="item" onclick={settings}>{t("titlebar.remote.settings")}</button>
  </div>
{/if}

<style>
  .remote {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    /* The width of the bar's other buttons. */
    min-width: 44px;
    padding: 0 10px;
    box-sizing: border-box;
    border: none;
    background: transparent;
    color: inherit;
    opacity: 1;
    cursor: pointer;
  }

  .remote.dim {
    opacity: 0.6;
  }

  .remote:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .remote svg {
    display: block;
    flex-shrink: 0;
  }

  /* The rounded, compact buttons of the macOS and GNOME title bars. */
  :global(.titlebar.mac) .remote,
  :global(.titlebar.linux) .remote {
    align-self: center;
    height: 24px;
    min-width: 28px;
    border-radius: 6px;
  }

  .badge {
    position: absolute;
    top: 50%;
    left: 50%;
    pointer-events: none;
  }

  .badge.on,
  .badge.pending {
    width: 6px;
    height: 6px;
    margin: -9px 0 0 5px;
    border-radius: 50%;
  }

  .badge.on {
    background: var(--success-fg, #89d185);
  }

  .badge.pending {
    background: var(--warning-fg, #d7ba7d);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--warning-fg, #d7ba7d) 30%, transparent);
  }

  .badge.count {
    min-width: 13px;
    height: 13px;
    margin: -12px 0 0 3px;
    padding: 0 3px;
    box-sizing: border-box;
    border-radius: 7px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 9px;
    font-weight: 700;
    line-height: 13px;
    text-align: center;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 601;
  }

  .panel {
    position: fixed;
    z-index: 602;
    max-height: calc(100vh - 60px);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: 4px;
    box-sizing: border-box;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .panel:focus {
    outline: none;
  }

  .panel-title {
    padding: 6px 10px 4px;
    font-weight: 600;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px 10px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .toggle {
    flex-direction: row;
    cursor: pointer;
  }

  .toggle input {
    margin: 0;
  }

  .row-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .row-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-sub {
    font-size: 11px;
    opacity: 0.6;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  /* Tailscale's state. Grey unless one of the states below says otherwise. */
  .pill {
    --pill: var(--titlebar-fg, #d4d4d4);
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 9px 2px 7px;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--pill) 40%, transparent);
    background: color-mix(in srgb, var(--pill) 14%, transparent);
    color: var(--pill);
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
  }

  .pill.ok {
    --pill: var(--success-fg, #89d185);
  }

  .pill.warn {
    --pill: var(--warning-fg, #d7ba7d);
  }

  .pill.off {
    --pill: var(--error-fg, #f48771);
  }

  .pill-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }

  .icon-button {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 50%;
    background: transparent;
    color: inherit;
    cursor: pointer;
    transition:
      background 120ms ease,
      color 120ms ease,
      border-color 120ms ease;
  }

  .icon-button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .icon-button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* Off: the power button in the accent, to connect. On: quiet, to
     disconnect. */
  .power:not(.on) {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .power:not(.on):hover:not(:disabled) {
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 85%, #fff);
  }

  .power.on {
    color: var(--success-fg, #89d185);
  }

  /* While it's on its way: not dimmed like a disabled button, it's working. */
  .power.switching:disabled {
    opacity: 1;
    cursor: progress;
  }

  .spinner {
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .section {
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.5;
  }

  .url-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .url {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
    font-size: 11.5px;
  }

  .qr-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .qr {
    flex-shrink: 0;
    width: 132px;
    height: 132px;
    border-radius: 6px;
  }

  .person {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 4px 0;
  }

  .person-actions {
    display: flex;
    gap: 6px;
  }

  .note {
    margin: 0;
    font-size: 11px;
    line-height: 1.4;
    opacity: 0.7;
    overflow-wrap: anywhere;
  }

  .note.error {
    opacity: 1;
    color: var(--error-fg, #f48771);
  }

  .small {
    flex-shrink: 0;
    padding: 3px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
    white-space: nowrap;
  }

  .small:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .small:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .small.primary {
    background: var(--pane-active-border, #3a6df0);
    border-color: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .small.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 85%, #fff);
  }

  .small.danger {
    color: #ff6b6b;
    border-color: rgba(255, 107, 107, 0.4);
  }

  .small.done {
    color: var(--success-fg, #89d185);
  }

  .small.wide {
    align-self: flex-start;
  }

  .divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--tabbar-border, #2a2a2a);
  }

  .item {
    display: flex;
    align-items: center;
    padding: 6px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .item:hover:not(:disabled),
  .item:focus-visible {
    outline: none;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .item:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
