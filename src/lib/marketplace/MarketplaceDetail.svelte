<script lang="ts">
  import { fly } from "svelte/transition";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import type { AgentTarget, CatalogItem, ComponentClass } from "$lib/core/marketplace-client";
  import { openUrl } from "@tauri-apps/plugin-opener";

  import InstallTargetPicker from "./InstallTargetPicker.svelte";
  import ItemIcon from "./ItemIcon.svelte";
  import ItemStats from "./ItemStats.svelte";
  import { catalogStore } from "./catalog-store.svelte";
  import { initialParamValues, paramsFor, paramsSatisfied, previewLines } from "./mcp-params";

  interface Props {
    item: CatalogItem;
    canInstall: boolean;
    onback: () => void;
  }

  const { item, canInstall, onback }: Props = $props();

  /** Installing is a two-step: the first click shows the exact command
   * Termo is about to run, and only the second one runs it. Installing
   * means running someone else's code on this machine, so it never happens
   * on a single click. */
  let confirming = $state(false);
  /** Null until the user picks one, so the default follows the item rather
   * than sticking to whatever the previously opened one supported. Codex is
   * a real target for an MCP server now (`codex mcp add`), so this just
   * follows what the model reports — a remote that needs a header Codex
   * can't take shows that as an install error instead, same as any other
   * command failure. */
  const installableTargets = $derived<AgentTarget[]>(item.targets);

  let chosenTarget = $state<AgentTarget | null>(null);
  const target = $derived(
    chosenTarget && installableTargets.includes(chosenTarget) ? chosenTarget : (installableTargets[0] ?? "claude"),
  );

  const busy = $derived(catalogStore.busyId === item.id);
  const approval = $derived(
    catalogStore.pendingApproval?.item.id === item.id ? catalogStore.pendingApproval : null,
  );
  const availableTargets = $derived<AgentTarget[]>(
    [
      catalogStore.status?.claudeCli ? "claude" : null,
      catalogStore.status?.codexCli ? "codex" : null,
      catalogStore.status?.geminiCli ? "gemini" : null,
      catalogStore.status?.copilotCli ? "copilot" : null,
    ].filter((value): value is AgentTarget => value !== null),
  );
  const commandPreview = $derived(
    item.detail.kind === "plugin" ? `claude plugin install ${item.detail.plugin_id}` : "",
  );

  // What the MCP registry says has to be filled in before installing this
  // entry — a remote's header values, or a stdio package's environment
  // variables. Read once: the component remounts fresh per item (see the
  // {#key selected.id} wrapper in MarketplacePanel), so `item` never
  // actually changes under it — this is a form's initial values, not a
  // value that should follow later prop updates.
  const paramSpecs = $derived(item.detail.kind === "mcp" ? paramsFor(item.detail) : []);
  // svelte-ignore state_referenced_locally
  let paramValues = $state<Record<string, string>>(item.detail.kind === "mcp" ? initialParamValues(paramsFor(item.detail)) : {});
  const paramsOk = $derived(paramsSatisfied(paramSpecs, paramValues));
  const mcpPreview = $derived(item.detail.kind === "mcp" ? previewLines(item.detail, paramValues) : []);

  async function confirmInstall(acceptCommand?: string) {
    const ok = await catalogStore.install(item, target, acceptCommand);
    if (ok) confirming = false;
  }

  async function confirmInstallMcp() {
    const ok = await catalogStore.installMcp(item, target, paramValues);
    if (ok) confirming = false;
  }

  const CLASS_LABEL_KEYS: Record<ComponentClass, MessageKey> = {
    plugin: "marketplace.classes.plugin",
    mcp: "marketplace.classes.mcp",
    skill: "marketplace.classes.skill",
  };

  const TARGET_LABEL_KEYS: Record<AgentTarget, MessageKey> = {
    claude: "marketplace.targets.claude",
    codex: "marketplace.targets.codex",
    gemini: "marketplace.targets.gemini",
    copilot: "marketplace.targets.copilot",
  };

  /** Where the code actually comes from — the single most important thing to
   * show before someone installs it. The catalog's `source` is a free-form
   * object whose useful field differs by source kind. */
  const origin = $derived.by(() => {
    if (item.detail.kind !== "plugin") return item.homepage;
    const source = item.detail.source as Record<string, unknown> | null;
    const url = source?.url ?? source?.repo ?? source?.path;
    return typeof url === "string" ? url : item.homepage;
  });
</script>

<div class="detail" transition:fly={{ x: 24, duration: 140 }}>
  <button class="back" onclick={onback}>{t("marketplace.back")}</button>

  <div class="heading">
    <ItemIcon {item} size={40} />
    <div>
      <h2>{item.displayName}</h2>
      <ItemStats {item} />
    </div>
  </div>
  <p class="meta">
    <span class="pill">{t(CLASS_LABEL_KEYS[item.class])}</span>
    {#if item.category}<span class="pill">{item.category}</span>{/if}
    {#if item.declaresSkills.length > 0}<span class="pill">{t("marketplace.hasSkills")}</span>{/if}
  </p>

  <p class="description">{item.description}</p>

  <dl class="facts">
    <dt>{t("marketplace.source")}</dt>
    <dd>{item.sourceLabel}</dd>
    {#if item.author}
      <dt>{t("marketplace.author")}</dt>
      <dd>{item.author}</dd>
    {/if}
    {#if item.version}
      <dt>{t("marketplace.version")}</dt>
      <dd>{item.version}</dd>
    {/if}
    {#if origin}
      <dt>{t("marketplace.origin")}</dt>
      <dd><button class="link" onclick={() => void openUrl(origin)}>{origin}</button></dd>
    {/if}
  </dl>

  <p class="label">{t("marketplace.installsInto")}</p>
  <InstallTargetPicker
    supported={installableTargets}
    available={availableTargets}
    value={target}
    onpick={(picked) => (chosenTarget = picked)}
  />
  {#if item.class === "plugin"}
    <p class="hint">{t("marketplace.codexHasNoPlugins")}</p>
  {/if}

  {#if item.class === "mcp" && paramSpecs.length > 0 && !approval}
    <div class="params">
      <p class="label">{t("marketplace.paramsTitle")}</p>
      {#each paramSpecs as spec (spec.name)}
        <label class="param">
          <span class="param-name">
            {spec.name}
            {#if spec.required}<span class="required">{t("marketplace.paramRequired")}</span>{/if}
          </span>
          <input
            type={spec.secret ? "password" : "text"}
            bind:value={paramValues[spec.name]}
            placeholder={spec.description ?? ""}
            autocomplete="off"
          />
        </label>
      {/each}
    </div>
  {/if}

  {#if approval}
    <!-- The marketplace wants to run a command on this machine as part of
         installing. It is shown in full, with its checksum, and only runs if
         the user says yes to this exact text. -->
    <div class="approval">
      <p class="approval-title">{t("marketplace.approvalTitle")}</p>
      {#each approval.commands as command (command.sha256)}
        <pre>{command.text}</pre>
        <p class="hint">sha256 {command.sha256}</p>
      {/each}
      <p class="hint">{t("marketplace.approvalWarning")}</p>
      <div class="row">
        <button class="install" disabled={busy} onclick={() => void confirmInstall(approval.commands[0].sha256)}>
          {busy ? t("marketplace.installing") : t("marketplace.approveAndInstall")}
        </button>
        <button class="ghost" onclick={() => (catalogStore.pendingApproval = null)}>
          {t("marketplace.cancel")}
        </button>
      </div>
    </div>
  {:else if item.installedIn.length > 0}
    <p class="installed">{t("marketplace.installed")}</p>
    <button class="ghost" disabled={busy || !canInstall} onclick={() => void catalogStore.uninstall(item, target)}>
      {busy ? t("marketplace.uninstalling") : t("marketplace.uninstall")}
    </button>
  {:else if confirming && item.detail.kind === "mcp"}
    <p class="label">{t("marketplace.willConfigure")}</p>
    <pre>{mcpPreview.join("\n")}</pre>
    <p class="hint">{t("marketplace.installWarning")}</p>
    <div class="row">
      <button class="install" disabled={busy || !paramsOk} onclick={() => void confirmInstallMcp()}>
        {busy ? t("marketplace.installing") : t("marketplace.confirmInstall")}
      </button>
      <button class="ghost" disabled={busy} onclick={() => (confirming = false)}>{t("marketplace.cancel")}</button>
    </div>
  {:else if confirming}
    <p class="label">{t("marketplace.willRun")}</p>
    <pre>{commandPreview}</pre>
    <p class="hint">{t("marketplace.installWarning")}</p>
    <div class="row">
      <button class="install" disabled={busy} onclick={() => void confirmInstall()}>
        {busy ? t("marketplace.installing") : t("marketplace.confirmInstall")}
      </button>
      <button class="ghost" disabled={busy} onclick={() => (confirming = false)}>{t("marketplace.cancel")}</button>
    </div>
  {:else}
    <button
      class="install"
      disabled={!canInstall || (item.class === "mcp" && !paramsOk)}
      onclick={() => (confirming = true)}
    >
      {t("marketplace.install")}
    </button>
    {#if !canInstall}
      <p class="hint">{t("marketplace.claudeCliMissing")}</p>
    {/if}
  {/if}

  {#if catalogStore.installError}
    <p class="error">{catalogStore.installError}</p>
  {/if}
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    overflow-y: auto;
  }

  .back {
    align-self: flex-start;
    padding: 4px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 11px;
    cursor: pointer;
  }

  .back:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .heading {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }

  h2 {
    margin: 0 0 2px;
    font-size: 15px;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 0;
  }

  .pill {
    padding: 2px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 999px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.8;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .description {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .facts {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px 10px;
    margin: 4px 0 0;
    font-size: 11px;
    color: var(--titlebar-fg, #d4d4d4);
  }

  dt {
    opacity: 0.6;
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    text-align: left;
    cursor: pointer;
    overflow-wrap: anywhere;
  }

  .label,
  .installed {
    margin: 4px 0 0;
    font-size: 11px;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .params {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .param {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .param-name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    opacity: 0.75;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .required {
    padding: 1px 5px;
    border-radius: 999px;
    background: rgba(241, 76, 76, 0.18);
    color: #f14c4c;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .param input {
    padding: 5px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
    color: var(--titlebar-fg, #d4d4d4);
    font: inherit;
    font-size: 12px;
  }

  .param input:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  pre {
    margin: 0;
    padding: 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .approval {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border: 1px solid #b58900;
    border-radius: 6px;
  }

  .approval-title {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    color: #e0a800;
  }

  .ghost {
    padding: 6px 12px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    cursor: pointer;
  }

  .ghost:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .error {
    margin: 0;
    font-size: 11px;
    color: #f14c4c;
  }

  .hint {
    margin: 0;
    font-size: 10px;
    opacity: 0.6;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .install {
    margin-top: 4px;
    padding: 6px 14px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    cursor: pointer;
  }

  .install:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
