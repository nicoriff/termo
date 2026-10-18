<script lang="ts">
  import { onMount } from "svelte";
  import { openLink } from "$lib/core/links";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import {
    clearSourceKey,
    marketplaceSources,
    setSourceKey,
    testSourceKey,
    type SourceId,
    type SourceStatus,
  } from "$lib/core/marketplace-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "../SettingDropdown.svelte";

  /** Product names: shown as they are in every language. */
  const NAMES: Record<SourceId, string> = { official: "Official MCP Registry", smithery: "Smithery" };
  const KEY_PAGES: Partial<Record<SourceId, string>> = { smithery: "https://smithery.ai/account/api-keys" };
  const sourceOptions = (Object.keys(NAMES) as SourceId[]).map((id) => ({ value: id, label: NAMES[id] }));

  let sources = $state<SourceStatus[]>([]);
  /** What's being typed for each source's key: never what's saved, which
   * stays in the keychain and isn't sent back here. */
  let typed = $state<Partial<Record<SourceId, string>>>({});
  let working = $state<SourceId | null>(null);
  let notes = $state<Partial<Record<SourceId, { ok: boolean; text: string }>>>({});

  onMount(() => {
    void marketplaceSources().then((list) => (sources = list));
  });

  function replace(status: SourceStatus) {
    sources = sources.map((source) => (source.id === status.id ? status : source));
  }

  async function run(source: SourceId, work: () => Promise<string>) {
    working = source;
    notes = { ...notes, [source]: undefined };
    try {
      notes = { ...notes, [source]: { ok: true, text: await work() } };
    } catch (reason) {
      notes = { ...notes, [source]: { ok: false, text: String(reason) } };
    } finally {
      working = null;
    }
  }

  const save = (source: SourceId) =>
    run(source, async () => {
      replace(await setSourceKey(source, (typed[source] ?? "").trim()));
      typed = { ...typed, [source]: "" };
      return t("preferences.marketplace.saved");
    });

  const test = (source: SourceId) =>
    run(source, async () => {
      await testSourceKey(source);
      return t("preferences.marketplace.works");
    });

  const remove = (source: SourceId) =>
    run(source, async () => {
      replace(await clearSourceKey(source));
      return t("preferences.marketplace.removed");
    });
</script>

<section>
  <h3>{t("preferences.marketplace.title")}</h3>
  <p class="setting-hint first">{t("preferences.marketplace.intro")}</p>
  <div class="row">
    <span>{t("preferences.marketplace.defaultSource")}</span>
    <SettingDropdown
      options={sourceOptions}
      value={appConfigStore.marketplaceSource}
      onpick={(value) => appConfigStore.setMarketplaceSource(value as SourceId)}
    />
  </div>
</section>

{#each sources as source (source.id)}
  <section class="source">
    <h4>
      {NAMES[source.id]}
      {#if source.takesKey}
        <span class="state" class:on={source.hasKey}>
          {source.hasKey ? t("preferences.marketplace.withKey") : t("preferences.marketplace.withoutKey")}
        </span>
      {/if}
    </h4>
    {#if !source.takesKey}
      <p class="setting-hint">{t("preferences.marketplace.noSetup")}</p>
    {:else}
      <p class="setting-hint">{t("preferences.marketplace.smitheryIntro")}</p>
      <div class="key-row">
        <input
          type="password"
          autocomplete="off"
          spellcheck="false"
          placeholder={source.hasKey ? t("preferences.marketplace.keyReplace") : t("preferences.marketplace.keyPlaceholder")}
          aria-label={t("preferences.marketplace.keyLabel", { source: NAMES[source.id] })}
          value={typed[source.id] ?? ""}
          oninput={(e) => (typed = { ...typed, [source.id]: e.currentTarget.value })}
          onkeydown={(e) => {
            if (e.key === "Enter" && (typed[source.id] ?? "").trim()) void save(source.id);
          }}
        />
        <button disabled={working !== null || !(typed[source.id] ?? "").trim()} onclick={() => void save(source.id)}>
          {t("preferences.marketplace.save")}
        </button>
        {#if source.hasKey}
          <button disabled={working !== null} onclick={() => void test(source.id)}>{t("preferences.marketplace.test")}</button>
          <button disabled={working !== null} onclick={() => void remove(source.id)}>{t("preferences.marketplace.remove")}</button>
        {/if}
      </div>
      {#if working === source.id}
        <p class="setting-hint">{t("preferences.marketplace.checking")}</p>
      {:else if notes[source.id]}
        <p class="note" class:bad={!notes[source.id]!.ok}>{notes[source.id]!.text}</p>
      {/if}
      {#if KEY_PAGES[source.id]}
        {@const page = KEY_PAGES[source.id]!}
        <button class="link" onclick={() => void openLink(page)}>{t("preferences.marketplace.getKey")} ↗</button>
      {/if}
      <p class="setting-hint">{t("preferences.marketplace.keychain")}</p>
    {/if}
  </section>
{/each}

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

  h4 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 6px;
    font-size: 12.5px;
    font-weight: 600;
  }

  .state {
    margin-left: auto;
    padding: 1px 8px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 999px;
    font-size: 10px;
    font-weight: 400;
    opacity: 0.75;
  }

  .state.on {
    border-color: #2ea043;
    color: #3fb950;
    opacity: 1;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    font-size: 12px;
  }

  input {
    padding: 4px 8px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.25));
    color: inherit;
    font: inherit;
    font-size: 12px;
  }

  .key-row {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }

  .key-row input {
    flex: 1;
    min-width: 0;
  }

  button {
    flex-shrink: 0;
    padding: 4px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .link {
    margin-top: 6px;
    padding: 0;
    border: none;
    color: var(--pane-active-border, #3a6df0);
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    line-height: 1.5;
    opacity: 0.6;
  }

  .setting-hint.first {
    margin: 0 0 10px;
  }

  .note {
    margin: 6px 0 0;
    font-size: 11px;
    color: #3fb950;
  }

  .note.bad {
    color: var(--error-fg, #f48771);
  }
</style>
