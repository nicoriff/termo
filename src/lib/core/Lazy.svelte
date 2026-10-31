<script lang="ts">
  import type { Component } from "svelte";

  /** A component without props, loaded the first time it shows (see lazy.ts). */
  let { load }: { load: () => Promise<{ default: Component }> } = $props();
</script>

{#await load() then { default: Loaded }}
  <Loaded />
{/await}
