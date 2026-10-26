<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { menuKeys } from "$lib/core/menu-keys";
  import { getOhMyPoshStatus } from "$lib/core/oh-my-posh-client";
  import { getStarshipStatus } from "$lib/core/starship-client";
  import type { PromptEngine } from "$lib/core/types";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { openPreferencesWindow } from "$lib/core/window-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { promptEngineName } from "../format";

  /** The prompt new consoles get (Preferences → Prompt), by name; a click
   * opens a menu to pick another. One that isn't installed yet is picked
   * in Preferences, which installs it. */
  const ENGINES: PromptEngine[] = ["none", "oh_my_posh", "starship"];

  let open = $state(false);
  /** The menu opens from the item's side of the bar, towards the middle. */
  let alignRight = $state(false);
  /** Which engines have a binary to run, asked each time the menu opens. */
  let installed = $state<Partial<Record<PromptEngine, boolean>>>({ none: true });

  function label(engine: PromptEngine): string {
    return promptEngineName(engine) ?? t("statusBar.prompt.shell");
  }

  const name = $derived(label(appConfigStore.promptEngine));

  function close(refocus = true) {
    open = false;
    if (refocus) uiState.refocusTerminals();
  }

  function toggle(event: MouseEvent) {
    if (open) {
      close();
      return;
    }
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    alignRight = box.left + box.width / 2 > window.innerWidth / 2;
    open = true;
    void getOhMyPoshStatus()
      .then((status) => (installed.oh_my_posh = !!status.path))
      .catch(() => {});
    void getStarshipStatus()
      .then((status) => (installed.starship = !!status.path))
      .catch(() => {});
  }

  function pick(engine: PromptEngine) {
    const ready = installed[engine] === true;
    appConfigStore.setPromptEngine(engine);
    if (ready) {
      close();
    } else {
      // Preferences has the Install button, and shows it for this engine now.
      close(false);
      void openPreferencesWindow("prompt");
    }
  }

  function settings() {
    close(false);
    void openPreferencesWindow("prompt");
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    close();
  }
</script>

<svelte:window onpointerdown={() => open && close(false)} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span class="prompt-anchor" onpointerdown={(event) => event.stopPropagation()} onkeydown={onKeydown}>
  <button class="prompt-item" class:open title={t("statusBar.prompt.hint")} aria-haspopup="menu" aria-expanded={open} onclick={toggle}>
    {name}
  </button>

  {#if open}
    <div class="prompt-menu" class:right={alignRight} role="menu" tabindex="-1" use:menuKeys={{ close, escape: false }}>
      <div class="menu-head">{t("statusBar.prompt.menuTitle")}</div>
      {#each ENGINES as engine (engine)}
        <button
          role="menuitemradio"
          aria-checked={appConfigStore.promptEngine === engine}
          onclick={() => pick(engine)}
        >
          <span class="check" aria-hidden="true">{appConfigStore.promptEngine === engine ? "✓" : ""}</span>
          {label(engine)}
          {#if installed[engine] === false}<span class="aside">{t("statusBar.prompt.notInstalled")}</span>{/if}
        </button>
      {/each}
      <p class="menu-note">{t("statusBar.prompt.newConsoles")}</p>
      <hr />
      <button role="menuitem" onclick={settings}>{t("statusBar.prompt.settings")}</button>
    </div>
  {/if}
</span>

<style>
  .prompt-anchor {
    position: relative;
    display: flex;
    align-items: center;
  }

  .prompt-item {
    display: flex;
    align-items: center;
    height: 18px;
    padding: 0 6px;
    border: none;
    border-radius: 3px;
    background: none;
    color: inherit;
    font: inherit;
    opacity: 0.85;
    white-space: nowrap;
    cursor: pointer;
  }

  .prompt-item:hover,
  .prompt-item.open {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.1));
  }

  /* Opens upward, like the repository's menu: the bar is the window's
     bottom edge. */
  .prompt-menu {
    position: absolute;
    left: 0;
    bottom: calc(100% + 5px);
    z-index: 60;
    width: 220px;
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

  .prompt-menu.right {
    left: auto;
    right: 0;
  }

  .menu-head {
    padding: 4px 8px 6px;
    font-size: 11px;
    opacity: 0.75;
  }

  .prompt-menu button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }

  .prompt-menu button:hover,
  .prompt-menu button:focus-visible {
    outline: none;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .check {
    width: 12px;
    flex-shrink: 0;
    color: var(--pane-active-border, #3a6df0);
  }

  .aside {
    margin-left: auto;
    font-size: 10.5px;
    opacity: 0.6;
  }

  .menu-note {
    margin: 0;
    padding: 4px 8px 2px 26px;
    font-size: 10.5px;
    opacity: 0.55;
    white-space: normal;
  }

  hr {
    margin: 4px 6px;
    border: none;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }
</style>
