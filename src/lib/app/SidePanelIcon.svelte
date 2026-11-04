<script lang="ts">
  import ForgeIcon from "$lib/git/ForgeIcon.svelte";
  import { gitStore } from "$lib/git/git-store.svelte";
  import type { SidePanelId } from "./side-panel-layout";

  /** A section's mark in its header, folded or not. Git's is where the
   * repository in focus is hosted (GitHub, GitLab...). */
  let { id }: { id: SidePanelId } = $props();
</script>

{#if id === "git"}
  <ForgeIcon kind={gitStore.snapshot ? (gitStore.forge?.kind ?? "generic") : null} />
{:else}
  <svg class="section-icon" viewBox={id === "harness" || id === "agents" ? "0 0 24 24" : "0 0 16 16"} aria-hidden="true">
    {#if id === "explorer"}
      <path d="M1.5 4.5v8h13v-7h-6l-1.5-1.5h-5.5z" fill="none" stroke="currentColor" stroke-width="1.1" />
    {:else if id === "harness"}
      <path d="M3.5 20.5L8.5 3.5M20.5 20.5L15.5 3.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path d="M12 19V8.5M9.4 11.1L12 8.5l2.6 2.6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
    {:else if id === "tabGroups"}
      <rect x="1.5" y="4" width="13" height="9.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.1" />
      <path d="M1.5 6.5h13M4.5 4V2.5h4V4" fill="none" stroke="currentColor" stroke-width="1.1" />
    {:else if id === "kanban"}
      <rect x="1.5" y="2" width="3.4" height="12" rx="1" fill="none" stroke="currentColor" stroke-width="1.1" />
      <rect x="6.3" y="2" width="3.4" height="8" rx="1" fill="none" stroke="currentColor" stroke-width="1.1" />
      <rect x="11.1" y="2" width="3.4" height="10" rx="1" fill="none" stroke="currentColor" stroke-width="1.1" />
    {:else if id === "agents"}
      <rect x="4" y="7.5" width="16" height="12" rx="3.2" fill="none" stroke="currentColor" stroke-width="1.6" />
      <path d="M12 7.5V4.2" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <circle cx="12" cy="3.6" r="1.2" fill="currentColor" />
      <circle cx="9" cy="13" r="1.4" fill="currentColor" />
      <circle cx="15" cy="13" r="1.4" fill="currentColor" />
      <path d="M9.5 16.6h5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
    {/if}
  </svg>
{/if}

<style>
  .section-icon {
    width: 15px;
    height: 15px;
    flex-shrink: 0;
    opacity: 0.8;
  }
</style>
