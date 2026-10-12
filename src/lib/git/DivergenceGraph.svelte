<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";

  /** How many commits each line draws before it just says the number. */
  const MAX_DOTS = 6;

  /** Two histories since they parted: the branch in hand and another (what
   * it tracks, or the base branch), each with the commits only it has. */
  let {
    ours,
    theirs,
    ahead,
    behind,
    title,
    hint,
  }: { ours: string; theirs: string; ahead: number; behind: number; title?: string; hint?: string } = $props();

  const level = $derived(ahead === 0 && behind === 0);
  const summary = $derived(
    level
      ? t("git.divergence.level", { other: theirs })
      : [
          ahead > 0 ? t(ahead === 1 ? "git.divergence.aheadOne" : "git.divergence.ahead", { n: String(ahead) }) : "",
          behind > 0 ? t(behind === 1 ? "git.divergence.behindOne" : "git.divergence.behind", { n: String(behind) }) : "",
        ]
          .filter(Boolean)
          .join(" · "),
  );
</script>

{#if title}
  <p class="title" title={hint}>{title}</p>
{/if}
<div class="divergence" class:level role="img" aria-label={`${ours} / ${theirs}: ${summary}`}>
  <!-- Where they parted, and a line off to each. -->
  <svg class="fork" viewBox="0 0 22 40" aria-hidden="true">
    <path class="line ours" d="M5 20 C13 20 9 10 22 10" />
    <path class="line theirs" d="M5 20 C13 20 9 30 22 30" />
    <circle class="base" cx="5" cy="20" r="3" />
  </svg>
  <div class="rows">
    {#each [{ side: "ours", name: ours, count: ahead, arrow: "↑" }, { side: "theirs", name: theirs, count: behind, arrow: "↓" }] as row (row.side)}
      <div class="row {row.side}" class:empty={row.count === 0}>
        <span class="track">
          {#each { length: Math.min(row.count, MAX_DOTS) } as _, index (index)}
            <i class="dot"></i>
          {/each}
          {#if row.count > MAX_DOTS}
            <i class="more">…</i>
          {/if}
        </span>
        {#if row.count > 0}
          <span class="count">{row.arrow}{row.count}</span>
        {/if}
        <span class="name" title={row.name}>{row.name}</span>
      </div>
    {/each}
  </div>
</div>
<p class="summary">{summary}</p>

<style>
  .title {
    margin: 8px 0 0;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    opacity: 0.55;
  }

  .divergence {
    display: flex;
    align-items: stretch;
    height: 40px;
    --ours: var(--pane-active-border, #3a6df0);
    --theirs: var(--git-behind, #d7a72a);
  }

  .fork {
    flex-shrink: 0;
    width: 22px;
    height: 40px;
  }

  .line {
    fill: none;
    stroke-width: 1.6;
  }

  .line.ours {
    stroke: var(--ours);
  }

  .line.theirs {
    stroke: var(--theirs);
  }

  .base {
    fill: var(--tabbar-bg, #1e1e1e);
    stroke: currentColor;
    stroke-width: 1.4;
    opacity: 0.75;
  }

  .rows {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .row.ours {
    --side: var(--ours);
  }

  .row.theirs {
    --side: var(--theirs);
  }

  /* The line goes on under the dots, as far as the last of them. */
  .track {
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 14px;
    padding: 0 6px 0 2px;
  }

  .track::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 1.6px;
    margin-top: -0.8px;
    background: var(--side);
  }

  .dot {
    position: relative;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--side);
  }

  .more {
    position: relative;
    font-style: normal;
    font-size: 10px;
    line-height: 1;
    color: var(--side);
    background: var(--tabbar-bg, #1e1e1e);
  }

  /* Nothing of its own on this side: the line is there, fainter. */
  .row.empty .track::before {
    opacity: 0.4;
  }

  .count {
    flex-shrink: 0;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--side);
  }

  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.8;
  }

  .row.ours .name {
    font-weight: 600;
    opacity: 1;
  }

  .summary {
    margin: 2px 0 0;
    font-size: 10.5px;
    opacity: 0.6;
  }
</style>
