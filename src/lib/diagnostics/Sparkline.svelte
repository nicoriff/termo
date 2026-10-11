<script lang="ts">
  import { sparklineArea, sparklinePoints } from "./diagnostics";

  let {
    values,
    ceiling,
    width = 180,
    height = 36,
    label,
  }: {
    values: readonly number[];
    /** Top of the scale; the largest reading when not given. */
    ceiling?: number;
    width?: number;
    height?: number;
    label: string;
  } = $props();

  const line = $derived(sparklinePoints(values, width, height, ceiling));
  const area = $derived(sparklineArea(line, height));
</script>

<svg
  class="spark"
  viewBox="0 0 {width} {height}"
  preserveAspectRatio="none"
  role="img"
  aria-label={label}
>
  {#if line}
    <polygon class="area" points={area} />
    <polyline class="line" points={line} />
  {/if}
</svg>

<style>
  .spark {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .area {
    fill: var(--spark-color, var(--pane-active-border, #3a6df0));
    opacity: 0.18;
  }

  .line {
    fill: none;
    stroke: var(--spark-color, var(--pane-active-border, #3a6df0));
    stroke-width: 1.5;
    stroke-linejoin: round;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
  }
</style>
