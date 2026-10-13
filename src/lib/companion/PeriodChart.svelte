<script lang="ts">
  import { t } from "$lib/i18n/i18n.svelte";
  import { formatTokens, formatUsd, type Spend } from "./companion";

  interface Props {
    spend: Spend;
    /** The models told apart by color, in the order of their colors. */
    models: readonly string[];
  }

  let { spend, models }: Props = $props();

  /** The day being looked at: under the pointer, or walked to with the arrows. */
  let active = $state<number | null>(null);

  const format = (amount: number) => (spend.unit === "usd" ? formatUsd(amount) : formatTokens(amount));
  /** The scale's own figures are round: "$50", not "$50.00". */
  const tick = (amount: number) => (spend.unit === "usd" ? `$${amount >= 1 ? Math.round(amount) : amount}` : formatTokens(amount));

  const lang = () => document.documentElement.lang || undefined;
  const short = (at: number) => new Intl.DateTimeFormat(lang(), { day: "numeric", month: "short" }).format(at);
  const long = (at: number) => new Intl.DateTimeFormat(lang(), { weekday: "short", day: "numeric", month: "short" }).format(at);

  const name = (index: number) => models[index] ?? t("companion.period.otherModels");
  const day = $derived(active === null ? null : (spend.days[active] ?? null));
  /** Where the day's column is, across the plot: what its tooltip hangs from. */
  const at = $derived(active === null ? 0 : ((active + 0.5) / spend.days.length) * 100);

  function onKeydown(event: KeyboardEvent) {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const from = active ?? (step > 0 ? -1 : spend.days.length);
    active = Math.min(spend.days.length - 1, Math.max(0, from + step));
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="chart"
  role="group"
  aria-label={t("companion.period.chartLabel")}
  tabindex="0"
  onkeydown={onKeydown}
  onblur={() => (active = null)}
  onpointerleave={() => (active = null)}
>
  <div class="plot">
    <div class="scale" aria-hidden="true">
      <span class="line"><span class="tick">{tick(spend.top)}</span></span>
      <span class="line"><span class="tick">{tick(spend.top / 2)}</span></span>
      <span class="line base"></span>
    </div>
    <div class="columns">
      {#each spend.days as column, index (column.at)}
        <div
          class="column"
          class:active={active === index}
          role="img"
          aria-label={`${long(column.at)}: ${format(column.total)}`}
          onpointerenter={() => (active = index)}
        >
          <div class="stack" style="height:{(column.total / spend.top) * 100}%">
            {#each column.parts as part, series (series)}
              {#if part > 0}
                <span class="segment s{series < models.length ? series + 1 : 0}" style="flex-grow:{part}"></span>
              {/if}
            {/each}
          </div>
        </div>
      {/each}
    </div>
    {#if day}
      <div class="tooltip" role="status" style="left:clamp(84px, calc((100% - var(--gutter)) * {at / 100}), calc(100% - 84px))">
        <div class="tooltip-top">
          <span>{long(day.at)}</span>
          <span class="amount">{format(day.total)}</span>
        </div>
        {#each day.parts as part, series (series)}
          {#if part > 0}
            <div class="tooltip-row">
              <span class="key s{series < models.length ? series + 1 : 0}" aria-hidden="true"></span>
              <span class="tooltip-name">{name(series)}</span>
              <span class="amount">{format(part)}</span>
            </div>
          {/if}
        {/each}
        {#if day.total === 0}
          <div class="tooltip-row"><span class="tooltip-name">{t("companion.period.chartNothing")}</span></div>
        {/if}
      </div>
    {/if}
  </div>
  <div class="days" aria-hidden="true">
    <span>{spend.days.length > 0 ? short(spend.days[0].at) : ""}</span>
    <span>{spend.days.length > 1 ? short(spend.days[spend.days.length - 1].at) : ""}</span>
  </div>
</div>

<style>
  .chart {
    display: flex;
    flex-direction: column;
    gap: 4px;
    outline: none;
    /* Kept clear of columns, to the right, for the scale's figures. */
    --gutter: 30px;
  }

  .chart:focus-visible {
    outline: 1px solid var(--divider-hover-bg, #3a6df0);
    outline-offset: 4px;
    border-radius: 4px;
  }

  .plot {
    position: relative;
    height: 96px;
    /* Room above the tallest column for the scale's top figure. */
    margin-top: 12px;
  }

  .scale {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    pointer-events: none;
  }

  .line {
    position: relative;
    border-top: 1px solid color-mix(in srgb, currentColor 10%, transparent);
  }

  .line.base {
    border-top-color: color-mix(in srgb, currentColor 22%, transparent);
  }

  .tick {
    position: absolute;
    right: 0;
    bottom: 1px;
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    opacity: 0.55;
  }

  .columns {
    position: absolute;
    inset: 0 var(--gutter) 0 0;
    display: flex;
    align-items: stretch;
  }

  /* The whole slot answers to the pointer, not just what's drawn in it. */
  .column {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }

  .column.active {
    background: color-mix(in srgb, currentColor 7%, transparent);
  }

  .stack {
    width: min(24px, calc(100% - 2px));
    display: flex;
    flex-direction: column-reverse;
    gap: 2px;
    min-height: 0;
  }

  .segment {
    flex-basis: 0;
    flex-shrink: 0;
    min-height: 1px;
  }

  /* The top of the column is its last part drawn: rounded where the data ends. */
  .segment:last-child {
    border-radius: 3px 3px 0 0;
  }

  .s1 {
    background: var(--cp-s1);
  }

  .s2 {
    background: var(--cp-s2);
  }

  .s3 {
    background: var(--cp-s3);
  }

  .s4 {
    background: var(--cp-s4);
  }

  .s5 {
    background: var(--cp-s5);
  }

  .s0 {
    background: var(--cp-other);
  }

  .tooltip {
    position: absolute;
    bottom: calc(100% + 4px);
    transform: translateX(-50%);
    z-index: 5;
    width: 168px;
    box-sizing: border-box;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    background: var(--tabbar-active-bg-solid, var(--tabbar-bg, #1e1e1e));
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    font-size: 11px;
    pointer-events: none;
  }

  .tooltip-top,
  .tooltip-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .tooltip-top {
    justify-content: space-between;
    font-weight: 600;
  }

  .tooltip-name {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    opacity: 0.85;
  }

  .amount {
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .key {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }

  .days {
    display: flex;
    justify-content: space-between;
    margin-right: var(--gutter);
    font-size: 10px;
    opacity: 0.55;
  }
</style>
