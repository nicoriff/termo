<script lang="ts">
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import type { CatalogItem, ComponentClass } from "$lib/core/marketplace-client";
  import type { ItemGroup } from "./filter";
  import ItemIcon from "./ItemIcon.svelte";
  import ItemStats from "./ItemStats.svelte";

  interface Props {
    groups: ItemGroup[];
    /** Where the MCP servers listed come from, by name. */
    mcpSource: string;
    highlighted: number;
    /** More MCP servers are being fetched, or can be. */
    mcpLoading: boolean;
    mcpHasMore: boolean;
    /** Plugins that match but aren't listed yet. */
    hiddenPlugins: number;
    onmoremcp: () => void;
    onmoreplugins: () => void;
    onpick: (item: CatalogItem) => void;
    onhighlight: (index: number) => void;
  }

  const { groups, mcpSource, highlighted, mcpLoading, mcpHasMore, hiddenPlugins, onmoremcp, onmoreplugins, onpick, onhighlight }: Props =
    $props();

  const CLASS_LABEL_KEYS: Record<ComponentClass, MessageKey> = {
    plugin: "marketplace.classes.plugin",
    mcp: "marketplace.classes.mcp",
    skill: "marketplace.classes.skill",
  };

  /** Flattened alongside the groups so a single index can walk the whole
   * list with the arrow keys, headers included but unselectable. */
  const rows = $derived(groups.flatMap((group) => group.items));
  /** Each row's place in that walk, looked up rather than searched for. */
  const indexOf = $derived(new Map(rows.map((item, index) => [item.id, index])));
</script>

<div class="results" role="listbox" aria-label={t("marketplace.title")}>
  {#each groups as group (group.class)}
    <!-- Each group says where it comes from and who takes it: a plugin is
         Claude Code's alone, an MCP server goes into every agent. -->
    <div class="group">
      {t(CLASS_LABEL_KEYS[group.class])}
      <span class="group-note">
        {group.class === "mcp"
          ? t("marketplace.groupMcp", { source: mcpSource })
          : group.class === "plugin"
            ? t("marketplace.groupPlugin")
            : ""}
      </span>
    </div>
    {#each group.items as item (item.id)}
      {@const index = indexOf.get(item.id) ?? -1}
      <button
        class="item"
        class:highlighted={index === highlighted}
        role="option"
        aria-selected={index === highlighted}
        onpointermove={() => onhighlight(index)}
        onclick={() => onpick(item)}
      >
        <ItemIcon {item} />
        <span class="body">
          <span class="name">
            {item.displayName}
            {#if item.extra?.verified}<span class="verified" title={t("marketplace.verified")}>✓</span>{/if}
            {#if item.installedIn.length > 0}<span class="badge">{t("marketplace.installedShort")}</span>{/if}
            {#if item.extra?.remote === false}<span class="badge">{t("marketplace.localShort")}</span>{/if}
            {#if item.declaresSkills.length > 0}<span class="badge">{t("marketplace.skillsShort")}</span>{/if}
          </span>
          {#if item.author}<span class="author">{t("marketplace.byAuthor", { author: item.author })}</span>{/if}
          <span class="description">{item.description}</span>
          <ItemStats {item} />
        </span>
      </button>
    {/each}
    <!-- More of this group: right under it, not at the end of the list. -->
    {#if group.class === "mcp"}
      {#if mcpLoading}
        <p class="more-note"><span class="spinner" aria-hidden="true"></span>{t("marketplace.loadingMcp")}</p>
      {:else if mcpHasMore}
        <button class="load-more" onclick={onmoremcp}>{t("marketplace.loadMoreMcp")}</button>
      {/if}
    {:else if group.class === "plugin" && hiddenPlugins > 0}
      <button class="load-more" onclick={onmoreplugins}>
        {t("marketplace.morePlugins", { n: String(hiddenPlugins) })}
      </button>
    {/if}
  {/each}
</div>

<style>
  .results {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    min-height: 0;
    overflow-y: auto;
  }

  .group {
    flex-shrink: 0;
    padding: 8px 8px 2px;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .item {
    flex-shrink: 0;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 7px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    text-align: left;
    cursor: pointer;
  }

  .item.highlighted {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  /* The icon sits beside a column with everything else, so a long
     description wraps under the text and not under the icon. */
  .body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }

  .author {
    font-size: 10px;
    opacity: 0.55;
  }

  .item.highlighted .name {
    color: var(--pane-active-border, #3a6df0);
  }

  .badge {
    padding: 1px 5px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 999px;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.75;
  }

  /* Two lines of the description is enough to tell entries apart without
     turning the list into a wall of marketing copy. */
  .description {
    font-size: 11px;
    line-height: 1.4;
    opacity: 0.65;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .verified {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 13px;
    height: 13px;
    margin-left: 4px;
    border-radius: 50%;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 9px;
    font-weight: 700;
  }

  .group-note {
    margin-left: 6px;
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    opacity: 0.75;
  }

  .load-more {
    flex-shrink: 0;
    margin: 4px 8px 8px;
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

  .more-note {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 8px;
    font-size: 11px;
    opacity: 0.75;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .spinner {
    width: 11px;
    height: 11px;
    border: 2px solid color-mix(in srgb, var(--pane-active-border, #3a6df0) 25%, transparent);
    border-top-color: var(--pane-active-border, #3a6df0);
    border-radius: 50%;
    animation: results-spin 0.8s linear infinite;
  }

  @keyframes results-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
