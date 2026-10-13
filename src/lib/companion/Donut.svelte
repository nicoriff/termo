<script lang="ts">
  import type { Slice } from "./companion";

  interface Props {
    /** What the whole is, said to a screen reader. */
    label: string;
    slices: readonly Slice[];
    format: (value: number) => string;
  }

  let { label, slices, format }: Props = $props();

  const RADIUS = 38;
  const AROUND = 2 * Math.PI * RADIUS;
  /** Left clear between two parts, along the ring. */
  const GAP = 2;

  /** The part being looked at, in the ring or in its legend. */
  let active = $state<string | null>(null);

  const total = $derived(slices.reduce((sum, slice) => sum + slice.value, 0));
  const arcs = $derived.by(() => {
    let from = 0;
    return slices.map((slice, index) => {
      const length = total > 0 ? (slice.value / total) * AROUND : 0;
      const arc = { slice, from, length: Math.max(0, length - (slices.length > 1 ? GAP : 0)), series: slice.rest ? 0 : index + 1 };
      from += length;
      return arc;
    });
  });
  const percent = (value: number) => {
    const share = total > 0 ? (value / total) * 100 : 0;
    return share > 0 && share < 1 ? "<1%" : `${Math.round(share)}%`;
  };
  const shown = $derived(slices.find((slice) => slice.key === active) ?? null);
</script>

<div class="donut" role="group" aria-label={label} onpointerleave={() => (active = null)}>
  <svg viewBox="0 0 100 100" aria-hidden="true">
    <g transform="rotate(-90 50 50)">
      {#each arcs as arc (arc.slice.key)}
        {#if arc.length > 0}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <circle
            class="arc s{arc.series}"
            class:dim={active !== null && active !== arc.slice.key}
            cx="50"
            cy="50"
            r={RADIUS}
            stroke-dasharray="{arc.length} {AROUND - arc.length}"
            stroke-dashoffset={-arc.from}
            onpointerenter={() => (active = arc.slice.key)}
          />
        {/if}
      {/each}
    </g>
    <text class="center" x="50" y="49">{shown ? percent(shown.value) : format(total)}</text>
    {#if shown}<text class="center-label" x="50" y="62">{format(shown.value)}</text>{/if}
  </svg>

  <ul class="legend">
    {#each arcs as arc (arc.slice.key)}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <li class:dim={active !== null && active !== arc.slice.key} title={arc.slice.hint} onpointerenter={() => (active = arc.slice.key)}>
        <span class="key s{arc.series}" aria-hidden="true"></span>
        <span class="name">{arc.slice.label}</span>
        {#if arc.slice.note}<span class="note" title={arc.slice.noteHint}>{arc.slice.note}</span>{/if}
        <span class="figure">{format(arc.slice.value)}</span>
        <span class="share">{percent(arc.slice.value)}</span>
      </li>
    {/each}
  </ul>
</div>

<style>
  .donut {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  svg {
    flex-shrink: 0;
    width: 96px;
    height: 96px;
  }

  .arc {
    fill: none;
    stroke-width: 13;
    transition: opacity 0.12s;
  }

  .dim {
    opacity: 0.35;
  }

  .center,
  .center-label {
    fill: currentColor;
    text-anchor: middle;
    dominant-baseline: middle;
  }

  .center {
    font-size: 15px;
    font-weight: 600;
  }

  .center-label {
    font-size: 9px;
    opacity: 0.7;
  }

  .legend {
    flex: 1;
    min-width: 0;
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .legend li {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    transition: opacity 0.12s;
  }

  .key {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }

  .name {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .note {
    flex-shrink: 0;
    font-size: 10px;
    padding: 0 5px;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, var(--cp-light) 60%, transparent);
    white-space: nowrap;
  }

  .figure,
  .share {
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .figure {
    font-weight: 600;
  }

  .share {
    width: 30px;
    text-align: right;
    font-size: 11px;
    opacity: 0.6;
  }

  .arc.s1 {
    stroke: var(--cp-s1);
  }

  .arc.s2 {
    stroke: var(--cp-s2);
  }

  .arc.s3 {
    stroke: var(--cp-s3);
  }

  .arc.s4 {
    stroke: var(--cp-s4);
  }

  .arc.s5 {
    stroke: var(--cp-s5);
  }

  .arc.s0 {
    stroke: var(--cp-other);
  }

  .key.s1 {
    background: var(--cp-s1);
  }

  .key.s2 {
    background: var(--cp-s2);
  }

  .key.s3 {
    background: var(--cp-s3);
  }

  .key.s4 {
    background: var(--cp-s4);
  }

  .key.s5 {
    background: var(--cp-s5);
  }

  .key.s0 {
    background: var(--cp-other);
  }
</style>
