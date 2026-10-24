<script lang="ts">
  import "./status-bar.css";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { menuKeys } from "$lib/core/menu-keys";
  import { openPreferencesWindow } from "$lib/core/window-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import StatusItem from "./StatusItem.svelte";

  /** The bar as arranged in Preferences → Status bar: each end's items, in
   * order. An item with nothing to say for the tab in focus isn't drawn. */
  const layout = $derived(appConfigStore.effectiveStatusBar);

  /** Where the right-click menu opened, or null. */
  let menuAt = $state<{ x: number; y: number } | null>(null);

  function openMenu(event: MouseEvent) {
    event.preventDefault();
    menuAt = { x: event.clientX, y: event.clientY };
  }

  /** Keyboard closes go through menuKeys, which gives the console its focus back. */
  function closeMenu() {
    menuAt = null;
  }

  function customize() {
    closeMenu();
    void openPreferencesWindow("status-bar");
  }
</script>

<svelte:window onpointerdown={() => menuAt && closeMenu()} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="status-bar" oncontextmenu={openMenu}>
  <div class="status-side left">
    {#each layout.left as item (item.id)}
      <StatusItem {item} />
    {/each}
  </div>
  <div class="status-side right">
    {#each layout.right as item (item.id)}
      <StatusItem {item} />
    {/each}
  </div>
</div>

{#if menuAt}
  <div
    class="status-menu"
    role="menu"
    tabindex="-1"
    style="left:{menuAt.x}px; bottom:{window.innerHeight - menuAt.y + 4}px"
    onpointerdown={(event) => event.stopPropagation()}
    use:menuKeys={{ close: () => closeMenu() }}
  >
    <button role="menuitem" onclick={customize}>{t("statusBar.customize")}</button>
  </div>
{/if}

<style>
  .status-menu {
    position: fixed;
    z-index: 70;
    display: flex;
    flex-direction: column;
    padding: 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 7px;
    background: var(--tabbar-active-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    box-shadow: 0 -6px 22px rgba(0, 0, 0, 0.38);
  }

  .status-menu button {
    padding: 5px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }

  .status-menu button:hover,
  .status-menu button:focus-visible {
    outline: none;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }
</style>
