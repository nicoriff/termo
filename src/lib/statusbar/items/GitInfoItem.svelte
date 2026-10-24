<script lang="ts">
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { gitStore } from "$lib/git/git-store.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { agoLabel } from "../format";
  import type { StatusItemId } from "../layout";
  import { ticker } from "../ticker.svelte";

  /** One reading of the active tab's repository each, beyond what the
   * repository's own item shows. */
  let { id }: { id: StatusItemId } = $props();

  const snap = $derived(appConfigStore.gitEnabled ? gitStore.snapshot : null);
  const counts = $derived(id === "git.lastCommit" || id === "git.lastFetch");

  $effect(() => {
    if (counts) return ticker.hold();
  });

  function ago(ms: number): string {
    const label = agoLabel(ms);
    return t(label.key, label.params);
  }

  const MAX_SUBJECT = 40;
  function short(subject: string): string {
    return subject.length > MAX_SUBJECT ? `${subject.slice(0, MAX_SUBJECT - 1)}…` : subject;
  }
</script>

{#if snap}
  {#if id === "git.lastCommit" && snap.head}
    <span class="status-item" title={`${snap.head.sha.slice(0, 10)} · ${snap.head.author}\n${snap.head.subject}`}>
      <span class="status-dim">{snap.head.sha.slice(0, 7)}</span>
      {short(snap.head.subject)}
      <span class="status-dim">· {ago(ticker.now - snap.head.time * 1000)}</span>
    </span>
  {:else if id === "git.stash" && snap.stashCount > 0}
    <span class="status-item" title={t("statusBar.items.stashHint")}>{t("statusBar.stash", { n: String(snap.stashCount) })}</span>
  {:else if id === "git.upstream" && snap.upstream}
    <span class="status-item" title={snap.upstreamGone ? t("statusBar.upstreamGone") : t("statusBar.items.upstreamHint")}>
      → <span class:status-dim={snap.upstreamGone}>{snap.upstream}</span>{#if snap.upstreamGone} ✕{/if}
    </span>
  {:else if id === "git.lastFetch" && snap.remotes.length > 0}
    <!-- A click fetches: the reading is there to say it's been a while. -->
    <button
      class="status-item"
      title={t("statusBar.lastFetchHint")}
      disabled={gitStore.running !== null}
      onclick={() => void gitStore.run({ kind: "fetch" }, true)}
    >
      ⇣ {snap.lastFetch ? ago(ticker.now - snap.lastFetch) : t("statusBar.neverFetched")}
    </button>
  {/if}
{/if}
