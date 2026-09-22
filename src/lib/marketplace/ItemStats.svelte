<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { i18n } from "$lib/i18n/i18n.svelte";
  import type { CatalogItem } from "$lib/core/marketplace-client";

  interface Props {
    item: CatalogItem;
  }

  const { item }: Props = $props();

  /** Grouped by the app's language, so 3211 reads as 3.211 in Spanish and
   * 3,211 in English rather than as a bare run of digits. */
  const installs = $derived(
    item.installCount === null ? null : new Intl.NumberFormat(i18n.locale).format(item.installCount),
  );

  /** Whole stars plus a half, the way a 4.5 is normally drawn. Only ever
   * shown when a source actually published a rating. */
  const stars = $derived(
    item.rating === null ? "" : "★".repeat(Math.floor(item.rating)) + (item.rating % 1 >= 0.5 ? "½" : ""),
  );
</script>

{#if installs !== null || item.rating !== null}
  <span class="stats">
    {#if installs !== null}
      <span title={t("marketplace.installs")}>↓ {installs}</span>
    {/if}
    {#if item.rating !== null}
      <span title={t("marketplace.rating")}>{stars} {item.rating.toFixed(1)}</span>
    {/if}
  </span>
{/if}

<style>
  .stats {
    display: flex;
    gap: 10px;
    font-size: 10px;
    opacity: 0.6;
    font-variant-numeric: tabular-nums;
  }
</style>
