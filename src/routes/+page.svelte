<script lang="ts">
  import { onMount } from "svelte";
  import MainWindow from "$lib/app/MainWindow.svelte";
  import Preferences from "$lib/preferences/Preferences.svelte";
  import { startI18nSync } from "$lib/i18n/i18n.svelte";
  import { routeWindowOpen } from "$lib/core/links";

  const isPreferences = location.hash === "#preferences";

  onMount(() => {
    routeWindowOpen();
    startI18nSync();
  });
</script>

{#if isPreferences}
  <Preferences />
{:else}
  <MainWindow />
{/if}

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
</style>
