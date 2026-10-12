<script lang="ts">
  import type { ForgeKind } from "$lib/core/git-client";

  /** Where the repository is hosted, as a small mark: the service's own for
   * the ones everybody knows, and git's for any other host (or none). */
  let { kind = null, size = 15 }: { kind?: ForgeKind | null; size?: number } = $props();
</script>

<svg class="forge-icon" viewBox="0 0 16 16" width={size} height={size} aria-hidden="true">
  {#if kind === "github"}
    <!-- In the text's color: its mark is black or white, whichever reads. -->
    <path
      fill="currentColor"
      d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8z"
    />
  {:else if kind === "gitlab"}
    <path fill="#e24329" d="M8 14.6 5.3 6.4h5.4z" />
    <path fill="#fc6d26" d="M8 14.6 5.3 6.4H1.5z M8 14.6l2.7-8.2h3.8z" />
    <path fill="#fca326" d="M1.5 6.4.7 8.9c-.1.2 0 .5.2.6L8 14.6z M14.5 6.4l.8 2.5c.1.2 0 .5-.2.6L8 14.6z" />
    <path fill="#e24329" d="M1.5 6.4h3.8L3.7 1.5c-.1-.3-.4-.3-.5 0z M14.5 6.4h-3.8l1.6-4.9c.1-.3.4-.3.5 0z" />
  {:else if kind === "bitbucket"}
    <path
      fill="#2684ff"
      fill-rule="evenodd"
      d="M1.6 1.8c-.3 0-.5.3-.5.6l1.9 11.3c0 .3.3.5.6.5h8.9c.2 0 .4-.2.5-.4l1.9-11.4c0-.3-.2-.6-.5-.6zM9.5 10h-3l-.8-4.1h4.5z"
    />
  {:else if kind === "azure"}
    <path fill="#0078d7" d="M15 3.1v9.6l-4 3.3-6.2-2.3V16l-3.5-4.6 10.2.8V3.5zm-3.4.5L5.9 0v2.3L.6 3.9 0 5.8v6.1l1.5.6V4.9z" />
  {:else if kind === "gitea"}
    <path
      fill="none"
      stroke="#609926"
      stroke-width="1.5"
      stroke-linejoin="round"
      d="M2 4.5h9.5v4.2a4 4 0 0 1-4 4H6a4 4 0 0 1-4-4z M11.5 6h1.3a1.7 1.7 0 0 1 0 3.4h-1.4"
    />
    <path fill="none" stroke="#609926" stroke-width="1.2" stroke-linecap="round" d="M5.2 2.8v-1.3 M8.2 2.8v-1.3" />
  {:else}
    <!-- Git's own diamond, with a branch drawn in it. -->
    <path fill="#f05133" d="M8 .6 15.4 8 8 15.4.6 8z" />
    <path fill="none" stroke="#fff" stroke-width="1.1" stroke-linecap="round" d="M6.2 5v6 M6.2 7.2c2.6 0 3.6.4 3.6 2.2" />
    <circle cx="6.2" cy="4.7" r="1.1" fill="#fff" />
    <circle cx="6.2" cy="11.3" r="1.1" fill="#fff" />
    <circle cx="9.8" cy="9.8" r="1.1" fill="#fff" />
  {/if}
</svg>

<style>
  .forge-icon {
    flex-shrink: 0;
    display: block;
  }
</style>
