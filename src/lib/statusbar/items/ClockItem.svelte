<script lang="ts">
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { optionOn, type StatusItemRef } from "../layout";
  import { ticker } from "../ticker.svelte";

  let { item }: { item: StatusItemRef } = $props();

  $effect(() => ticker.hold());

  const seconds = $derived(optionOn(item, "seconds"));
  const time = $derived(
    new Date(ticker.now).toLocaleTimeString(i18n.locale, { hour: "2-digit", minute: "2-digit", second: seconds ? "2-digit" : undefined }),
  );
  const date = $derived(new Date(ticker.now).toLocaleDateString(i18n.locale, { weekday: "long", day: "numeric", month: "long" }));
</script>

<span class="status-item" title={date} aria-label={t("statusBar.items.clock")}>{time}</span>
