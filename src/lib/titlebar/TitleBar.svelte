<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    closeWindow,
    getDecorationLayout,
    isWindowFullscreen,
    isWindowMaximized,
    minimizeWindow,
    onWindowResized,
    openPreferencesWindow,
    toggleMaximizeWindow,
  } from "$lib/core/window-client";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { isLinux, isMac } from "$lib/core/platform";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { requestCloseWindow } from "$lib/app/quit";
  import AppMenu from "./AppMenu.svelte";
  import KeepAwakeButton from "./KeepAwakeButton.svelte";
  import { DEFAULT_DECORATION_LAYOUT, parseDecorationLayout, type WindowButton } from "./decoration-layout";

  // This bar is also the Preferences window's, where closing just hides it.
  // A console window's X closes that window and its consoles, so it asks
  // first (and the last one closing ends Termo).
  const isConsoleWindow = getCurrentWindow().label !== "preferences";

  let {
    title = "Termo",
    showSettingsButton = true,
    showMaximizeButton = true,
  }: {
    title?: string;
    showSettingsButton?: boolean;
    showMaximizeButton?: boolean;
  } = $props();

  let maximized = $state(false);
  let fullscreen = $state(false);
  let unlisten: (() => void) | null = null;
  let menuOpen = $state(false);
  let menuButtonEl = $state<HTMLButtonElement | undefined>(undefined);
  let layout = $state(DEFAULT_DECORATION_LAYOUT);
  const startButtons = $derived(layout.start.filter(shown));
  const endButtons = $derived(layout.end.filter(shown));

  function shown(button: WindowButton): boolean {
    return button !== "maximize" || showMaximizeButton;
  }

  async function refreshMaximized() {
    maximized = await isWindowMaximized();
    uiState.setMaximized(maximized);
    // macOS hides its window controls in fullscreen, so the space reserved
    // for them has to go away too.
    if (isMac) fullscreen = await isWindowFullscreen();
  }

  async function toggleMaximize() {
    await toggleMaximizeWindow();
    await refreshMaximized();
  }

  onMount(() => {
    void refreshMaximized();
    if (isLinux) {
      getDecorationLayout()
        .then((value) => (layout = parseDecorationLayout(value)))
        .catch(() => {});
    }
    // Best-effort sync for external changes (e.g. Aero-snap to top of screen).
    // The toggle button/double-click above refresh state directly since this
    // event doesn't reliably fire for every maximize/restore transition.
    void onWindowResized(() => void refreshMaximized()).then((fn) => (unlisten = fn));
  });

  onDestroy(() => unlisten?.());

  function handleDoubleClick() {
    void toggleMaximize();
  }
</script>

<!-- On macOS the window uses the native close/minimize/zoom buttons, drawn
     by the OS over this bar (titleBarStyle "Overlay" — see
     tauri.macos.conf.json and open_preferences_window), so this bar only
     reserves room for them on the left instead of drawing its own. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="titlebar"
  class:mac={isMac}
  class:mac-fullscreen={isMac && fullscreen}
  class:linux={isLinux}
  data-tauri-drag-region
  ondblclick={handleDoubleClick}
>
  <!-- Linux, like GNOME: window buttons where the desktop's setting puts
       them, the menu on the other side, the title centered. -->
  {#if isLinux}
    <div class="titlebar-controls">
      {#each startButtons as button (button)}
        {@render windowButton(button)}
      {/each}
      {#if layout.menuSide === "start"}
        {@render keepAwake()}
        {@render menuButton()}
      {/if}
    </div>
  {/if}

  <div class="titlebar-brand" data-tauri-drag-region>
    <img class="titlebar-icon" src="/app-icon.png" alt="" />
    <span class="titlebar-title">{title}</span>
  </div>

  <div class="titlebar-spacer" data-tauri-drag-region></div>

  <div class="titlebar-controls">
    {#if isLinux}
      {#if layout.menuSide === "end"}
        {@render keepAwake()}
        {@render menuButton()}
      {/if}
      {#each endButtons as button (button)}
        {@render windowButton(button)}
      {/each}
    {:else}
      {@render keepAwake()}
      {@render menuButton()}
      {#if !isMac}
        {@render windowButton("minimize")}
        {#if showMaximizeButton}
          {@render windowButton("maximize")}
        {/if}
        {@render windowButton("close")}
      {/if}
    {/if}
  </div>
</div>

<!-- Keeping the computer awake belongs to the app, not to a window: only
     the main window's bar offers it. -->
{#snippet keepAwake()}
  {#if isConsoleWindow && showSettingsButton && appConfigStore.keepAwakeButton}
    <KeepAwakeButton />
  {/if}
{/snippet}

{#snippet menuButton()}
  {#if showSettingsButton}
    <button
      class="titlebar-button titlebar-menu"
      aria-label={t("titlebar.menu")}
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      bind:this={menuButtonEl}
      onclick={() => (menuOpen = !menuOpen)}
    >
      <!-- A menu, not a settings button: the sliders glyph now belongs to
           the "System preferences" item inside it. -->
      <svg viewBox="0 0 14 14" width="13" height="13">
        <line x1="2" y1="3.5" x2="12" y2="3.5" stroke="currentColor" stroke-width="1" />
        <line x1="2" y1="7" x2="12" y2="7" stroke="currentColor" stroke-width="1" />
        <line x1="2" y1="10.5" x2="12" y2="10.5" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
    {#if menuOpen}
      <AppMenu anchor={menuButtonEl} onclose={() => (menuOpen = false)} />
    {/if}
  {/if}
{/snippet}

{#snippet windowButton(button: WindowButton)}
  {#if button === "minimize"}
    <button class="titlebar-button" aria-label={t("titlebar.minimize")} onclick={() => void minimizeWindow()}>
      <svg viewBox="0 0 10 10" width="10" height="10">
        <path d="M0 5h10" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
  {:else if button === "maximize"}
    <button
      class="titlebar-button"
      aria-label={t(maximized ? "titlebar.restore" : "titlebar.maximize")}
      onclick={() => void toggleMaximize()}
    >
      {#if maximized}
        <svg viewBox="0 0 10 10" width="10" height="10">
          <path d="M2 0h8v8h-2M0 2h8v8H0z" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      {:else}
        <svg viewBox="0 0 10 10" width="10" height="10">
          <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" stroke-width="1" />
        </svg>
      {/if}
    </button>
  {:else}
    <button
      class="titlebar-button titlebar-close"
      aria-label={t("titlebar.close")}
      onclick={() => void (isConsoleWindow ? requestCloseWindow() : closeWindow())}
    >
      <svg viewBox="0 0 10 10" width="10" height="10">
        <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" stroke-width="1" />
      </svg>
    </button>
  {/if}
{/snippet}

<style>
  .titlebar {
    display: flex;
    align-items: stretch;
    height: var(--titlebar-height, 34px);
    background: var(--titlebar-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    user-select: none;
    flex-shrink: 0;
  }

  .titlebar-brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    opacity: 0.85;
  }

  .titlebar-icon {
    width: 16px;
    height: 16px;
  }

  .titlebar-title {
    font-family: "Ubuntu", sans-serif;
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .titlebar-spacer {
    flex: 1;
  }

  .titlebar-controls {
    display: flex;
    align-items: stretch;
    padding-right: 4px;
  }

  .titlebar-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .titlebar-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .titlebar-close:hover {
    background: var(--titlebar-close-hover-bg, #e81123);
    color: #fff;
  }

  /* macOS: room for the native traffic lights on the left, a centered
     title in the system font, and a compact rounded settings button. */
  .titlebar.mac {
    position: relative;
    padding-left: 78px;
  }

  .titlebar.mac-fullscreen {
    padding-left: 0;
  }

  .titlebar.mac .titlebar-brand {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    transform: translateX(-50%);
    gap: 6px;
    padding: 0;
    pointer-events: none;
  }

  .titlebar.mac .titlebar-icon {
    width: 14px;
    height: 14px;
  }

  .titlebar.mac .titlebar-title {
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: normal;
  }

  .titlebar.mac .titlebar-controls {
    align-items: center;
    padding-right: 8px;
  }

  .titlebar.mac .titlebar-button {
    width: 28px;
    height: 24px;
    border-radius: 6px;
  }
  /* Linux: GNOME's header bar — centered bold title, no icon, round
     neutral window buttons (no red close), a flat rounded menu button. */
  .titlebar.linux {
    position: relative;
  }

  .titlebar.linux .titlebar-brand {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    transform: translateX(-50%);
    padding: 0;
    opacity: 1;
    pointer-events: none;
  }

  .titlebar.linux .titlebar-icon {
    display: none;
  }

  .titlebar.linux .titlebar-title {
    font-family: system-ui, "Cantarell", "Ubuntu", sans-serif;
    font-size: 13px;
    font-weight: 700;
    letter-spacing: normal;
  }

  .titlebar.linux .titlebar-controls {
    align-items: center;
    gap: 10px;
    padding: 0 8px;
  }

  .titlebar.linux .titlebar-controls:empty {
    display: none;
  }

  .titlebar.linux .titlebar-button {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: color-mix(in srgb, currentColor 10%, transparent);
  }

  .titlebar.linux .titlebar-button svg {
    width: 8px;
    height: 8px;
  }

  .titlebar.linux .titlebar-button:hover,
  .titlebar.linux .titlebar-close:hover {
    background: color-mix(in srgb, currentColor 18%, transparent);
    color: inherit;
  }

  .titlebar.linux .titlebar-menu {
    width: 32px;
    border-radius: 6px;
    background: transparent;
  }

  .titlebar.linux .titlebar-menu svg {
    width: 13px;
    height: 13px;
  }
</style>
