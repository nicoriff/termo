<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { agentCustomMcp, type McpServer } from "$lib/core/agents-client";
  import { looksSecret, parseServerJson } from "./integration-info";

  let {
    agentId,
    onadd,
    oncancel,
  }: {
    agentId: string;
    onadd: (server: McpServer, secrets: string[]) => void;
    oncancel: () => void;
  } = $props();

  const JSON_EXAMPLE = `{
  "mcpServers": {
    "mi-server": {
      "command": "npx",
      "args": ["-y", "@mi-org/mi-server"],
      "env": { "API_KEY": "…" }
    }
  }
}`;

  let name = $state("");
  /** The user typed a name: a pasted config doesn't overwrite it. */
  let nameTouched = $state(false);
  let kind = $state<"stdio" | "http" | "sse">("stdio");
  let url = $state("");
  let values = $state<{ name: string; value: string; secret: boolean }[]>([]);
  let adding = $state(false);
  let error = $state("");

  /** "Comando local": the server's JSON, pasted as its README gives it. */
  let json = $state("");
  const pasted = $derived(json.trim() ? parseServerJson(json) : null);
  /** Which of the pasted variables go to the keychain; by default, the ones
   * whose name says they're secret. */
  let secretOverrides = $state<Record<string, boolean>>({});
  const pastedValues = $derived(
    Object.entries(pasted?.values ?? {}).map(([key, value]) => ({
      name: key,
      value,
      secret: secretOverrides[key] ?? looksSecret(key),
    })),
  );

  $effect(() => {
    if (pasted?.name && !nameTouched) name = pasted.name;
  });

  const remote = $derived(kind !== "stdio");
  const ready = $derived(Boolean(name.trim()) && (remote ? Boolean(url.trim()) : Boolean(pasted)));

  async function add() {
    adding = true;
    error = "";
    try {
      const request =
        remote || !pasted
          ? { kind, command: "", args: [], url: url.trim(), values: $state.snapshot(values).filter((entry) => entry.name.trim()) }
          : { kind: pasted.kind, command: pasted.command, args: pasted.args, url: pasted.url, values: pastedValues };
      const result = await agentCustomMcp({ agentId, name: name.trim(), ...request });
      onadd(result.server, result.secrets);
    } catch (reason) {
      error = String(reason);
    } finally {
      adding = false;
    }
  }
</script>

<div class="custom">
  <p class="hint">{t("agents.custom.explain")}</p>

  <label class="field-label" for="custom-name">{t("agents.custom.name")}</label>
  <!-- svelte-ignore a11y_autofocus -->
  <input
    id="custom-name"
    class="field"
    placeholder={t("agents.custom.namePlaceholder")}
    bind:value={name}
    oninput={() => (nameTouched = true)}
    autofocus
  />

  <div class="kinds" role="radiogroup" aria-label={t("agents.custom.kind")}>
    <button class="kind" class:selected={!remote} role="radio" aria-checked={!remote} onclick={() => (kind = "stdio")}>
      <span class="kind-title">{t("agents.custom.local")}</span>
      <span class="hint">{t("agents.custom.localText")}</span>
    </button>
    <button class="kind" class:selected={remote} role="radio" aria-checked={remote} onclick={() => (kind = "http")}>
      <span class="kind-title">{t("agents.custom.remote")}</span>
      <span class="hint">{t("agents.custom.remoteText")}</span>
    </button>
  </div>

  {#if remote}
    <label class="field-label" for="custom-url">{t("agents.custom.url")}</label>
    <div class="row">
      <input id="custom-url" class="field" placeholder="https://…/mcp" bind:value={url} />
      <select class="field narrow" bind:value={kind} aria-label={t("agents.custom.transport")}>
        <option value="http">HTTP</option>
        <option value="sse">SSE</option>
      </select>
    </div>
  {:else}
    <label class="field-label" for="custom-json">{t("agents.custom.json")}</label>
    <p class="hint">{t("agents.custom.jsonHint")}</p>
    <textarea id="custom-json" class="field mono json" rows="8" spellcheck="false" placeholder={JSON_EXAMPLE} bind:value={json}
    ></textarea>
    {#if json.trim() && !pasted}
      <p class="error">{t("agents.custom.jsonInvalid")}</p>
    {:else if pasted}
      <div class="summary">
        <span class="mono">
          {pasted.kind === "stdio" ? [pasted.command, ...pasted.args].join(" ") : pasted.url}
        </span>
        {#if pasted.skipped.length > 0}
          <span class="hint">{t("agents.custom.jsonSkipped", { names: pasted.skipped.join(", ") })}</span>
        {/if}
        {#each pastedValues as entry (entry.name)}
          <label class="secret-row" title={t("agents.custom.secretHint")}>
            <input
              type="checkbox"
              checked={entry.secret}
              onchange={(e) => (secretOverrides = { ...secretOverrides, [entry.name]: e.currentTarget.checked })}
            />
            <span class="mono">{entry.name}</span>
            <span class="hint">{entry.secret ? t("agents.custom.toKeychain") : t("agents.custom.plain")}</span>
          </label>
        {/each}
      </div>
    {/if}
  {/if}

  {#if remote}
  <div class="field-label">{t("agents.custom.headers")}</div>
  {#each values as entry, index (index)}
    <div class="row">
      <input class="field mono" placeholder="Authorization" bind:value={entry.name} />
      <input
        class="field"
        type={entry.secret ? "password" : "text"}
        placeholder={t("agents.custom.value")}
        bind:value={entry.value}
      />
      <label class="secret" title={t("agents.custom.secretHint")}>
        <input type="checkbox" bind:checked={entry.secret} />
        🔒
      </label>
      <button class="remove" aria-label={t("agents.integrations.remove")} onclick={() => values.splice(index, 1)}>×</button>
    </div>
  {/each}
  <button class="small" onclick={() => values.push({ name: "", value: "", secret: true })}>
    + {t("agents.custom.addHeader")}
  </button>
  {/if}

  {#if error}<p class="error">{error}</p>{/if}

  <div class="actions">
    <button class="button" onclick={oncancel}>{t("agents.wizard.cancel")}</button>
    <button class="button primary" disabled={adding || !ready} onclick={() => void add()}>
      {t("agents.integrations.add")}
    </button>
  </div>
</div>

<style>
  .custom {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 8px;
    padding: 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
  }

  .hint {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.45;
    opacity: 0.65;
  }

  .field-label {
    margin-top: 6px;
    font-weight: 600;
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    padding: 6px 9px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
    color: inherit;
    font: inherit;
    outline: none;
  }

  .field:focus {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .field.mono {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
  }

  .field.json {
    resize: vertical;
    line-height: 1.45;
  }

  .summary {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
    overflow-wrap: anywhere;
  }

  .mono {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
  }

  .secret-row {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }

  .field.narrow {
    width: 90px;
    flex-shrink: 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .kinds {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-top: 6px;
  }

  .kind {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .kind.selected {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.2));
  }

  .kind-title {
    font-weight: 600;
  }

  .secret {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
    cursor: pointer;
  }

  .remove {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font-size: 15px;
    opacity: 0.6;
    cursor: pointer;
  }

  .remove:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .small {
    align-self: flex-start;
    padding: 3px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .error {
    margin: 0;
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }

  .button {
    padding: 5px 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .button.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
