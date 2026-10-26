<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { menuKeys } from "$lib/core/menu-keys";
  import { getOhMyPoshStatus, installOhMyPoshTheme, OH_MY_POSH_THEMES } from "$lib/core/oh-my-posh-client";
  import { getStarshipStatus, installStarshipPreset, STARSHIP_PRESETS } from "$lib/core/starship-client";
  import type { PromptEngine } from "$lib/core/types";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { openPreferencesWindow } from "$lib/core/window-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { promptEngineName } from "../format";

  /** The prompt new consoles get (Preferences → Prompt), by name; a click
   * opens a menu to pick another, and each tool's flyout its theme. One
   * that isn't installed yet is installed from Preferences. */
  type Tool = Exclude<PromptEngine, "none">;

  const THEMES: Record<Tool, readonly string[]> = { oh_my_posh: OH_MY_POSH_THEMES, starship: STARSHIP_PRESETS };
  const MENU_WIDTH = 220;
  const FLYOUT_WIDTH = 210;
  const MARGIN = 8;

  let open = $state(false);
  /** Placed against the window: the bar's left end clips what overflows it. */
  let menuPos = $state<{ left: number; bottom: number }>({ left: 0, bottom: 0 });
  let flyout = $state<Tool | null>(null);
  let flyoutPos = $state<{ left: number; bottom: number }>({ left: 0, bottom: 0 });
  /** Which tools have a binary to run, asked each time the menu opens. */
  let installed = $state<Partial<Record<Tool, boolean>>>({});
  /** The theme being installed, while it is. */
  let busy = $state<string | null>(null);
  let failed = $state(false);

  function label(engine: PromptEngine): string {
    return promptEngineName(engine) ?? t("statusBar.prompt.shell");
  }

  const name = $derived(label(appConfigStore.promptEngine));

  /** The theme a tool has picked; Starship's own config counts as none. */
  function currentTheme(tool: Tool): string | null {
    if (tool === "oh_my_posh") return appConfigStore.ohMyPoshTheme;
    return appConfigStore.starshipUseOwnConfig ? null : appConfigStore.starshipPreset;
  }

  function close(refocus = true) {
    open = false;
    flyout = null;
    failed = false;
    if (refocus) uiState.refocusTerminals();
  }

  function toggle(event: MouseEvent) {
    if (open) {
      close();
      return;
    }
    // From the item's side of the bar, towards the middle.
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const left = box.left + box.width / 2 > window.innerWidth / 2 ? box.right - MENU_WIDTH : box.left;
    menuPos = {
      left: Math.max(MARGIN, Math.min(left, window.innerWidth - MENU_WIDTH - MARGIN)),
      bottom: window.innerHeight - box.top + 5,
    };
    open = true;
    void getOhMyPoshStatus()
      .then((status) => (installed.oh_my_posh = !!status.path))
      .catch(() => {});
    void getStarshipStatus()
      .then((status) => (installed.starship = !!status.path))
      .catch(() => {});
  }

  /** Beside its row, growing upwards from the row's foot; flipped to the
   * menu's left where the window ends. */
  function openFlyout(tool: Tool, row: HTMLElement) {
    if (flyout === tool) return;
    const box = row.getBoundingClientRect();
    const menu = row.closest(".prompt-menu")!.getBoundingClientRect();
    const right = menu.right + 4;
    flyoutPos = {
      left: right + FLYOUT_WIDTH + MARGIN > window.innerWidth ? Math.max(MARGIN, menu.left - 4 - FLYOUT_WIDTH) : right,
      bottom: Math.max(MARGIN, window.innerHeight - box.bottom - 4),
    };
    failed = false;
    flyout = tool;
  }

  function pickShell() {
    appConfigStore.setPromptEngine("none");
    close();
  }

  function pickTheme(tool: Tool, theme: string) {
    if (busy) return;
    busy = theme;
    failed = false;
    const install = tool === "oh_my_posh" ? installOhMyPoshTheme : installStarshipPreset;
    void install(theme)
      .then(() => {
        if (tool === "oh_my_posh") {
          appConfigStore.setOhMyPoshTheme(theme);
        } else {
          appConfigStore.setStarshipPreset(theme);
          appConfigStore.setStarshipUseOwnConfig(false);
        }
        appConfigStore.setPromptEngine(tool);
        close();
      })
      .catch(() => (failed = true))
      .finally(() => (busy = null));
  }

  function pickOwnStarshipConfig() {
    appConfigStore.setStarshipUseOwnConfig(true);
    appConfigStore.setPromptEngine("starship");
    close();
  }

  /** Preferences has the Install button, and shows it for this tool now. */
  function installInPreferences(tool: Tool) {
    appConfigStore.setPromptEngine(tool);
    settings();
  }

  function settings() {
    close(false);
    void openPreferencesWindow("prompt");
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    if (flyout) flyout = null;
    else close();
  }
</script>

<svelte:window onpointerdown={() => open && close(false)} />

{#snippet check(on: boolean)}
  <span class="check" aria-hidden="true">{on ? "✓" : ""}</span>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span class="prompt-anchor" onpointerdown={(event) => event.stopPropagation()} onkeydown={onKeydown}>
  <button class="prompt-item" class:open title={t("statusBar.prompt.hint")} aria-haspopup="menu" aria-expanded={open} onclick={toggle}>
    {name}
  </button>

  {#if open}
    <div
      class="prompt-menu"
      role="menu"
      tabindex="-1"
      style="left:{menuPos.left}px; bottom:{menuPos.bottom}px; width:{MENU_WIDTH}px"
      use:menuKeys={{ close, escape: false }}
    >
      <div class="menu-head">{t("statusBar.prompt.menuTitle")}</div>
      <button
        role="menuitemradio"
        aria-checked={appConfigStore.promptEngine === "none"}
        onmouseenter={() => (flyout = null)}
        onclick={pickShell}
      >
        {@render check(appConfigStore.promptEngine === "none")}
        {label("none")}
      </button>
      {#each ["oh_my_posh", "starship"] as const as tool (tool)}
        <button
          role="menuitem"
          aria-haspopup="menu"
          aria-expanded={flyout === tool}
          class:expanded={flyout === tool}
          onmouseenter={(event) => openFlyout(tool, event.currentTarget)}
          onclick={(event) => openFlyout(tool, event.currentTarget)}
        >
          {@render check(appConfigStore.promptEngine === tool)}
          {label(tool)}
          <span class="aside">
            {#if installed[tool] === false}{t("statusBar.prompt.notInstalled")}{/if}
            <span class="chevron" aria-hidden="true">›</span>
          </span>
        </button>
      {/each}
      <p class="menu-note">{t("statusBar.prompt.newConsoles")}</p>
      <hr />
      <button role="menuitem" onmouseenter={() => (flyout = null)} onclick={settings}>{t("statusBar.prompt.settings")}</button>
    </div>

    {#if flyout}
      {@const tool = flyout}
      <div
        class="prompt-menu flyout"
        role="menu"
        tabindex="-1"
        aria-label={label(tool)}
        style="left:{flyoutPos.left}px; bottom:{flyoutPos.bottom}px; width:{FLYOUT_WIDTH}px"
        use:menuKeys={{ close, back: () => (flyout = null), escape: false }}
      >
        {#if installed[tool] === false}
          <p class="menu-note first">{t("statusBar.prompt.notInstalledHint", { name: label(tool) })}</p>
          <button role="menuitem" onclick={() => installInPreferences(tool)}>{t("statusBar.prompt.installInPreferences")}</button>
        {:else}
          <div class="menu-head">{t(tool === "oh_my_posh" ? "statusBar.prompt.themes" : "statusBar.prompt.presets")}</div>
          <div class="theme-list">
            {#if tool === "starship"}
              <button
                role="menuitemradio"
                aria-checked={appConfigStore.starshipUseOwnConfig}
                disabled={busy !== null}
                onclick={pickOwnStarshipConfig}
              >
                {@render check(appConfigStore.starshipUseOwnConfig)}
                {t("statusBar.prompt.ownStarshipConfig")}
              </button>
            {/if}
            {#each THEMES[tool] as theme (theme)}
              <button
                role="menuitemradio"
                aria-checked={currentTheme(tool) === theme}
                disabled={busy !== null}
                onclick={() => pickTheme(tool, theme)}
              >
                {@render check(currentTheme(tool) === theme)}
                {theme}
                {#if busy === theme}<span class="aside">{t("statusBar.prompt.installingTheme")}</span>{/if}
              </button>
            {/each}
          </div>
          {#if failed}<p class="menu-note error">{t("statusBar.prompt.themeFailed")}</p>{/if}
        {/if}
      </div>
    {/if}
  {/if}
</span>

<style>
  .prompt-anchor {
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
    position: fixed;
    z-index: 60;
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

  .prompt-menu.flyout {
    z-index: 61;
    max-height: calc(100vh - 2 * 8px);
  }

  .theme-list {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: thin;
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
    flex-shrink: 0;
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

  .prompt-menu button:hover:not(:disabled),
  .prompt-menu button:focus-visible,
  .prompt-menu button.expanded {
    outline: none;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .prompt-menu button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .check {
    width: 12px;
    flex-shrink: 0;
    color: var(--pane-active-border, #3a6df0);
  }

  .aside {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    font-size: 10.5px;
    opacity: 0.6;
  }

  .chevron {
    font-size: 13px;
    opacity: 0.9;
  }

  .menu-note {
    margin: 0;
    padding: 4px 8px 2px 26px;
    font-size: 10.5px;
    opacity: 0.55;
    white-space: normal;
  }

  .menu-note.first {
    padding: 6px 8px;
  }

  .menu-note.error {
    color: #e05a5a;
    opacity: 0.9;
  }

  hr {
    margin: 4px 6px;
    border: none;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }
</style>
