<script lang="ts">
  import type { CatalogItem } from "$lib/core/marketplace-client";

  interface Props {
    item: CatalogItem;
    size?: number;
  }

  const { item, size = 28 }: Props = $props();

  /** The image comes off the network and plenty of entries have none, so
   * the initials are the normal case, not an error path. */
  let failed = $state(false);

  const initials = $derived(
    item.displayName
      .split(/[\s\-_.@/]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0]?.toUpperCase() ?? "")
      .join(""),
  );

  /** A stable hue per entry, so the same plugin always gets the same colour
   * and a list of them stays visually distinguishable. */
  const hue = $derived(
    [...item.id].reduce((accumulator, char) => (accumulator * 31 + char.charCodeAt(0)) % 360, 7),
  );
</script>

{#if item.iconUrl && !failed}
  <img
    class="icon"
    src={item.iconUrl}
    alt=""
    width={size}
    height={size}
    loading="lazy"
    style="width:{size}px;height:{size}px"
    onerror={() => (failed = true)}
  />
{:else}
  <span
    class="icon initials"
    aria-hidden="true"
    style="width:{size}px;height:{size}px;font-size:{Math.round(size * 0.4)}px;background:hsl({hue} 45% 28%);color:hsl({hue} 70% 82%)"
  >
    {initials}
  </span>
{/if}

<style>
  .icon {
    flex-shrink: 0;
    border-radius: 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    object-fit: cover;
    background: var(--terminal-bg, #1a1a1a);
  }

  .initials {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-weight: 600;
    letter-spacing: 0.02em;
  }
</style>
