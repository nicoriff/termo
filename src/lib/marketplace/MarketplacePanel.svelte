<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import type { CatalogItem, ComponentClass } from "$lib/core/marketplace-client";
  import { catalogStore } from "./catalog-store.svelte";
  import { groupItems, matchesFacets, matchItems, type Facets, type SortMode } from "./filter";
  import MarketplaceResults from "./MarketplaceResults.svelte";
  import MarketplaceDetail from "./MarketplaceDetail.svelte";

  const WARNING_KEYS: Record<string, MessageKey> = {
    "claude-cli-missing": "marketplace.claudeCliMissing",
    "claude-cli-failed": "marketplace.claudeCliFailed",
  };

  const CLASS_FILTERS: { value: ComponentClass; key: MessageKey }[] = [
    { value: "plugin", key: "marketplace.classes.plugin" },
    { value: "mcp", key: "marketplace.classes.mcp" },
    { value: "skill", key: "marketplace.classes.skill" },
  ];

  let query = $state("");
  /** What the list is actually showing results for — set from `query` on Enter. */
  let submittedQuery = $state("");
  let highlighted = $state(0);
  // By id, not by object: install/uninstall replace catalogStore.items with
  // fresh objects (see refreshAfterChange in catalog-store.svelte.ts), so
  // holding the object itself would leave the open detail view stuck
  // showing pre-install state (still-empty installedIn) even though the
  // install went through — looking closed but actually just stale.
  let selectedId = $state<string | null>(null);
  const selected = $derived<CatalogItem | null>(
    selectedId === null
      ? null
      : (catalogStore.items.find((item) => item.id === selectedId) ??
          catalogStore.mcpItems.find((item) => item.id === selectedId) ??
          null),
  );
  let classFilter = $state<ComponentClass | null>(null);
  let installedOnly = $state(false);
  let withSkillsOnly = $state(false);
  let sort = $state<SortMode>("installs");
  let inputEl: HTMLInputElement | undefined = $state();

  const facets = $derived<Facets>({
    classes: classFilter ? [classFilter] : [],
    installedOnly,
    withSkillsOnly,
  });
  const showsMcp = $derived.by(() => {
    const classes = facets.classes ?? [];
    return classes.length === 0 || classes.includes("mcp");
  });
  // The Claude catalog is downloaded once and scored locally; the MCP
  // registry already filtered its results server-side, so those are only
  // run through the facet checks, never re-scored against the query text —
  // its own idea of a match doesn't have to agree with scoreItem's.
  const localMatches = $derived(matchItems(catalogStore.items, submittedQuery, facets, sort));
  const mcpVisible = $derived(showsMcp ? catalogStore.mcpItems.filter((item) => matchesFacets(item, facets)) : []);
  const matches = $derived([...localMatches, ...mcpVisible]);
  const groups = $derived(groupItems(matches));

  void catalogStore.load();

  // Both the local filter and the registry search run on what was submitted
  // with Enter, not on every keystroke. Starts empty, so opening the panel
  // still fetches the "browse everything" first page right away.
  $effect(() => {
    void catalogStore.searchMcp(submittedQuery);
  });

  // Clearing the box is the one change that doesn't wait for Enter: there's
  // nothing left to type, so it goes straight back to the full list.
  $effect(() => {
    if (query.trim() === "") submittedQuery = "";
  });

  $effect(() => {
    if (!selected) inputEl?.focus();
  });

  // A new search restarts the selection at the top, so Enter always opens the
  // best match rather than whatever row the previous query left highlighted.
  $effect(() => {
    void submittedQuery;
    void classFilter;
    highlighted = 0;
  });

  function close() {
    uiState.closeMarketplace();
  }

  function onKeydown(event: KeyboardEvent) {
    // The panel owns the keyboard while it's open (MainWindow's window-level
    // handler bails out on overlayOpen), so nothing here reaches the console.
    event.stopPropagation();
    if (event.key === "Escape") {
      // Escape backs out one level at a time: detail, then list, then shut.
      if (selected) selectedId = null;
      else close();
    } else if (selected) {
      return;
    } else if (event.key === "ArrowDown") {
      highlighted = matches.length === 0 ? 0 : (highlighted + 1) % matches.length;
    } else if (event.key === "ArrowUp") {
      highlighted = matches.length === 0 ? 0 : (highlighted - 1 + matches.length) % matches.length;
    } else if (event.key === "Enter") {
      // Enter searches what's typed; once that's what the list shows, the
      // next Enter opens the highlighted result.
      if (query.trim() !== submittedQuery) {
        submittedQuery = query.trim();
      } else {
        const item = matches[highlighted];
        if (item) selectedId = item.id;
      }
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="mp-backdrop" onpointerdown={close} transition:fade={{ duration: 120 }}></div>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="mp-panel"
  role="dialog"
  tabindex="-1"
  aria-label={t("marketplace.title")}
  onpointerdown={(event) => event.stopPropagation()}
  onkeydown={onKeydown}
  transition:fly={{ x: 420, duration: 180, easing: cubicOut }}
>
  <header class="mp-header">
    <h1>{t("marketplace.title")}</h1>
    <div class="mp-actions">
      <button onclick={() => void catalogStore.load(true)} disabled={catalogStore.loading}>
        {catalogStore.loading ? t("marketplace.loading") : t("marketplace.refresh")}
      </button>
      <button onclick={close} aria-label={t("marketplace.close")}>✕</button>
    </div>
  </header>

  {#if selected}
    <!-- Keyed so opening another item starts its detail view clean, rather
         than carrying over a half-finished confirmation from the last one. -->
    {#key selected.id}
      <MarketplaceDetail
        item={selected}
        canInstall={catalogStore.claudeCliAvailable}
        onback={() => (selectedId = null)}
      />
    {/key}
  {:else}
    <input
      bind:this={inputEl}
      bind:value={query}
      class="mp-input"
      placeholder={t("marketplace.searchPlaceholder")}
      aria-label={t("marketplace.searchPlaceholder")}
    />

    <div class="mp-filters">
      {#each CLASS_FILTERS as filter (filter.value)}
        <button
          class="chip"
          class:on={classFilter === filter.value}
          onclick={() => (classFilter = classFilter === filter.value ? null : filter.value)}
        >
          {t(filter.key)}
        </button>
      {/each}
      <button class="chip" class:on={installedOnly} onclick={() => (installedOnly = !installedOnly)}>
        {t("marketplace.filterInstalled")}
      </button>
      <button class="chip" class:on={withSkillsOnly} onclick={() => (withSkillsOnly = !withSkillsOnly)}>
        {t("marketplace.filterSkills")}
      </button>
      <span class="spacer"></span>
      <!-- Sorted by downloads by default: it's the only popularity signal
           these catalogs publish, and with 300+ entries it's what makes the
           list readable before you type anything. -->
      <button
        class="chip"
        title={t("marketplace.sortHint")}
        onclick={() => (sort = sort === "installs" ? "relevance" : "installs")}
      >
        {sort === "installs" ? t("marketplace.sortInstalls") : t("marketplace.sortRelevance")}
      </button>
    </div>

    {#each catalogStore.warnings as warning (warning)}
      {#if WARNING_KEYS[warning]}
        <p class="mp-warning">{t(WARNING_KEYS[warning])}</p>
      {/if}
    {/each}

    {#if catalogStore.error}
      <p class="mp-warning">{catalogStore.error}</p>
    {/if}

    {#if catalogStore.loading && catalogStore.items.length === 0}
      <p class="mp-empty">{t("marketplace.loading")}</p>
    {:else if groups.length === 0}
      <p class="mp-empty">{catalogStore.items.length === 0 ? t("marketplace.noCatalog") : t("marketplace.noResults")}</p>
    {:else}
      <MarketplaceResults
        {groups}
        {highlighted}
        onpick={(item) => (selectedId = item.id)}
        onhighlight={(index) => (highlighted = index)}
      />
    {/if}

    {#if showsMcp}
      {#if catalogStore.mcpError}
        <p class="mp-warning">{catalogStore.mcpError}</p>
      {:else if catalogStore.mcpLoading}
        <p class="mp-empty">{t("marketplace.loadingMcp")}</p>
      {:else if catalogStore.mcpHasMore}
        <button class="load-more" onclick={() => void catalogStore.loadMoreMcp()}>
          {t("marketplace.loadMoreMcp")}
        </button>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .mp-backdrop {
    position: absolute;
    inset: 0;
    z-index: 599;
    background: rgba(0, 0, 0, 0.35);
  }

  .mp-panel {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: 600;
    width: min(420px, 100%);
    display: flex;
    flex-direction: column;
    background: var(--tabbar-active-bg, #1e1e1e);
    border-left: 1px solid var(--tabbar-border, #2a2a2a);
    box-shadow: -8px 0 32px rgba(0, 0, 0, 0.45);
    overflow: hidden;
  }

  .mp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  h1 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .mp-actions {
    display: flex;
    gap: 6px;
  }

  .mp-actions button {
    padding: 4px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 11px;
    cursor: pointer;
  }

  .mp-actions button:hover:not(:disabled) {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .mp-actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .mp-input {
    margin: 0;
    padding: 10px 12px;
    border: none;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font: inherit;
    font-size: 13px;
  }

  .mp-input:focus {
    outline: none;
  }

  .mp-filters {
    flex-shrink: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .spacer {
    flex: 1;
  }

  .chip {
    padding: 3px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 999px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 10px;
    cursor: pointer;
  }

  .chip.on {
    border-color: var(--pane-active-border, #3a6df0);
    color: var(--pane-active-border, #3a6df0);
  }

  .mp-warning,
  .mp-empty {
    margin: 0;
    padding: 10px 12px;
    font-size: 11px;
    opacity: 0.7;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .load-more {
    flex-shrink: 0;
    margin: 4px 12px 10px;
    padding: 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 11px;
    cursor: pointer;
  }

  .load-more:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }
</style>
