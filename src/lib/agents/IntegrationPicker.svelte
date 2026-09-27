<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { searchMcpRegistry, type CatalogItem, type McpDetail } from "$lib/core/marketplace-client";
  import { agentMcpServer, type McpServer } from "$lib/core/agents-client";
  import { initialParamValues, paramsFor, paramsSatisfied, type ParamSpec } from "$lib/marketplace/mcp-params";

  let {
    agentId,
    suggestions = [],
    onadd,
    oncancel,
  }: {
    agentId: string;
    /** Searches the AI proposed, with why. */
    suggestions?: { query: string; why: string }[];
    onadd: (server: McpServer, secrets: string[]) => void;
    oncancel: () => void;
  } = $props();

  let query = $state("");
  let results = $state<CatalogItem[]>([]);
  let searching = $state(false);
  let searched = $state(false);
  let error = $state("");
  let chosen = $state<CatalogItem | null>(null);
  let specs = $state<ParamSpec[]>([]);
  let values = $state<Record<string, string>>({});
  let adding = $state(false);
  let debounce: ReturnType<typeof setTimeout>;
  let seq = 0;

  const detail = $derived(chosen?.detail as McpDetail | undefined);

  function search(text: string) {
    query = text;
    clearTimeout(debounce);
    debounce = setTimeout(() => void run(text), 300);
  }

  async function run(text: string) {
    const mine = ++seq;
    searching = true;
    error = "";
    try {
      const page = await searchMcpRegistry(text.trim());
      if (mine !== seq) return;
      results = page.items.slice(0, 8);
      searched = true;
    } catch (reason) {
      if (mine === seq) error = String(reason);
    } finally {
      if (mine === seq) searching = false;
    }
  }

  function choose(item: CatalogItem) {
    chosen = item;
    specs = paramsFor(item.detail as McpDetail);
    values = initialParamValues(specs);
    error = "";
  }

  async function add() {
    if (!chosen || !detail) return;
    adding = true;
    error = "";
    try {
      const result = await agentMcpServer({
        agentId,
        serverName: detail.server_name,
        remotes: detail.remotes,
        packages: detail.packages,
        params: $state.snapshot(values),
        secretParams: specs.filter((spec) => spec.secret).map((spec) => spec.name),
      });
      onadd(result.server, result.secrets);
    } catch (reason) {
      error = String(reason);
    } finally {
      adding = false;
    }
  }
</script>

<div class="picker">
  {#if !chosen}
    <!-- svelte-ignore a11y_autofocus -->
    <input
      class="field"
      placeholder={t("agents.integrations.searchPlaceholder")}
      value={query}
      autofocus
      oninput={(e) => search(e.currentTarget.value)}
    />
    {#if suggestions.length > 0 && !query}
      <div class="suggestions">
        <span class="hint">{t("agents.integrations.suggested")}</span>
        {#each suggestions as suggestion (suggestion.query)}
          <button class="chip" title={suggestion.why} onclick={() => search(suggestion.query)}>{suggestion.query}</button>
        {/each}
      </div>
    {/if}
    <div class="results">
      {#if searching}
        <p class="hint">{t("agents.integrations.searching")}</p>
      {:else}
        {#each results as item (item.id)}
          <button class="result" onclick={() => choose(item)}>
            <span class="result-name">{item.displayName}</span>
            <span class="result-text">{item.description}</span>
          </button>
        {:else}
          {#if searched}<p class="hint">{t("agents.integrations.noResults")}</p>{/if}
        {/each}
      {/if}
    </div>
  {:else}
    <div class="chosen">
      <span class="result-name">{chosen.displayName}</span>
      <span class="result-text">{chosen.description}</span>
    </div>
    {#each specs as spec (spec.name)}
      <label class="param">
        <span class="param-name">{spec.name}{spec.required ? " *" : ""}{#if spec.secret}{" · "}🔒{/if}</span>
        {#if spec.description}<span class="hint">{spec.description}</span>{/if}
        <input class="field" type={spec.secret ? "password" : "text"} bind:value={values[spec.name]} />
      </label>
    {:else}
      <p class="hint">{t("agents.integrations.noParams")}</p>
    {/each}
    {#if specs.some((spec) => spec.secret)}
      <p class="hint">{t("agents.integrations.secretsNote")}</p>
    {/if}
  {/if}

  {#if error}<p class="error">{error}</p>{/if}

  <div class="actions">
    <button class="button" onclick={() => (chosen ? (chosen = null) : oncancel())}>
      {chosen ? t("agents.wizard.back") : t("agents.wizard.cancel")}
    </button>
    {#if chosen}
      <button class="button primary" disabled={adding || !paramsSatisfied(specs, values)} onclick={() => void add()}>
        {t("agents.integrations.add")}
      </button>
    {/if}
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 8px;
    padding: 12px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 8px;
  }

  .field {
    box-sizing: border-box;
    width: 100%;
    padding: 7px 9px;
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

  .suggestions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .chip {
    padding: 3px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .results {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 220px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .result,
  .chosen {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 7px 9px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
  }

  .result {
    cursor: pointer;
  }

  .result:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .chosen {
    background: var(--tab-focus-bg, rgba(58, 109, 240, 0.15));
  }

  .result-name,
  .param-name {
    font-weight: 600;
  }

  .result-text {
    font-size: 11.5px;
    opacity: 0.65;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .param {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .hint {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.45;
    opacity: 0.6;
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
