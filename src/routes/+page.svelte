<script lang="ts">
  import { onMount } from "svelte";
  import ResizeBorders from "$lib/titlebar/ResizeBorders.svelte";
  import TitleBar from "$lib/titlebar/TitleBar.svelte";
  import SplitView from "$lib/layout/SplitView.svelte";
  import SettingsPanel from "$lib/settings/SettingsPanel.svelte";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { uiState } from "$lib/core/ui-state.svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { onWindowResized } from "$lib/core/window-client";
  import { listShells } from "$lib/core/pty-client";
  import { resolveShellArgs } from "$lib/core/shell-args";

  onMount(() => {
    let resizeDebounce: ReturnType<typeof setTimeout>;
    let unlisten: (() => void) | null = null;

    (async () => {
      await appConfigStore.init();

      // Always resolve a concrete ShellInfo (even for "no explicit default",
      // which just means "first detected shell") rather than letting the
      // backend pick blind — resolveShellArgs needs to know the shell id to
      // decide whether to inject the PSReadLine menu-complete startup command.
      const shells = await listShells();
      const shell =
        (appConfigStore.defaultShellId && shells.find((s) => s.id === appConfigStore.defaultShellId)) || shells[0];
      const shellArgs = resolveShellArgs(shell?.id, shell?.args, appConfigStore.psreadlineMenuComplete);
      workspaceStore.initRoot(shell?.path, shellArgs, shell?.display_name);

      unlisten = await onWindowResized(() => {
        clearTimeout(resizeDebounce);
        resizeDebounce = setTimeout(() => void appConfigStore.recordWindowSize(), 400);
      });
    })();

    return () => {
      clearTimeout(resizeDebounce);
      unlisten?.();
    };
  });

  $effect(() => {
    // Track theme/font changes and persist them once the initial config load
    // has completed (avoids overwriting the saved config with defaults).
    void themeStore.current.name;
    void themeStore.fontSizeOverride;
    void themeStore.fontFamilyOverride;
    void themeStore.backgroundOpacity;
    void themeStore.outputHighlightingEnabled;
    if (appConfigStore.loaded) void appConfigStore.persist();
  });
</script>

<div class="window-chrome" class:maximized={uiState.maximized}>
  <ResizeBorders />
  <TitleBar />
  <main class="content">
    {#if workspaceStore.root}
      <SplitView node={workspaceStore.root} />
    {/if}
    {#if uiState.settingsOpen}
      <SettingsPanel />
    {/if}
  </main>
</div>

<style>
  @font-face {
    font-family: "Ubuntu";
    font-style: normal;
    font-weight: 400;
    font-display: swap;
    src: url("/fonts/ubuntu-400.woff2") format("woff2");
  }
  @font-face {
    font-family: "Ubuntu";
    font-style: normal;
    font-weight: 500;
    font-display: swap;
    src: url("/fonts/ubuntu-500.woff2") format("woff2");
  }
  @font-face {
    font-family: "Ubuntu";
    font-style: normal;
    font-weight: 700;
    font-display: swap;
    src: url("/fonts/ubuntu-700.woff2") format("woff2");
  }

  :global(html, body) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
    font-family: "Ubuntu", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .window-chrome {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    border-radius: 10px;
    overflow: hidden;
  }

  .window-chrome.maximized {
    border-radius: 0;
  }

  .content {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
  }
</style>
