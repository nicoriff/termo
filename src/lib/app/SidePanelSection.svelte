<script lang="ts">
  import type { Component } from "svelte";
  import Lazy from "$lib/core/Lazy.svelte";
  import { provideSectionHost, type SectionHost } from "./side-panel-host.svelte";

  /** One section of the side panel, with what it hands its header. */
  let { host, load }: { host: SectionHost; load: () => Promise<{ default: Component }> } = $props();
  // svelte-ignore state_referenced_locally
  provideSectionHost(host);
</script>

<Lazy {load} />
