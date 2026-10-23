<script lang="ts">
  import type { GraphCommit } from "$lib/core/git-client";
  import { ago } from "$lib/agents/agent-spec";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import type { CommitSide, GraphRow } from "./git-graph";

  /** A history drawn in lanes: a dot per commit, a line per branch on its
   * way down, and next to each the refs on it and what it says. `compact` is
   * the side panel's: fewer lanes, no columns. */
  let {
    rows,
    sides,
    compact = false,
    selected = null,
    now = Date.now(),
    onpick,
  }: {
    rows: GraphRow<GraphCommit>[];
    sides: Map<string, CommitSide>;
    compact?: boolean;
    selected?: string | null;
    now?: number;
    onpick?: (commit: GraphCommit) => void;
  } = $props();

  const ROW = $derived(compact ? 22 : 24);
  const LANE = $derived(compact ? 11 : 14);
  /** Past this many lanes the rest is clipped: the text matters more. */
  const MAX_LANES = $derived(compact ? 6 : 14);
  const DOT = 3.6;

  const lanes = $derived(Math.min(MAX_LANES, Math.max(1, ...rows.map((row) => row.width))));
  const graphWidth = $derived(lanes * LANE + 6);

  /** Each lane's color, out of the theme's own palette so it holds up on a
   * light theme as well as a dark one. */
  const palette = $derived.by(() => {
    const c = themeStore.current.colors;
    return [c.blue, c.green, c.magenta, c.yellow, c.cyan, c.red, c.brightBlue, c.brightGreen, c.brightMagenta, c.brightCyan];
  });
  const colorOf = (index: number) => palette[index % palette.length];
  const x = (lane: number) => lane * LANE + LANE / 2 + 2;

  /** A line between a lane at the row's edge and the dot in the middle. */
  function link(from: number, to: number, down: boolean): string {
    const [y0, y1] = down ? [ROW / 2, ROW] : [0, ROW / 2];
    const [x0, x1] = down ? [x(to), x(from)] : [x(from), x(to)];
    if (x0 === x1) return `M${x0} ${y0}V${y1}`;
    const mid = (y0 + y1) / 2;
    return `M${x0} ${y0}C${x0} ${mid} ${x1} ${mid} ${x1} ${y1}`;
  }

  function when(seconds: number): string {
    if (compact) {
      const age = ago(seconds * 1000, now);
      return t(age.key, age.params);
    }
    return new Date(seconds * 1000).toLocaleString(i18n.locale, { dateStyle: "medium", timeStyle: "short" });
  }
</script>

<div class="commit-graph" class:compact>
  {#each rows as row (row.commit.sha)}
    {@const commit = row.commit}
    {@const side = sides.get(commit.sha) ?? "shared"}
    {@const hint = `${commit.sha.slice(0, 8)} · ${commit.author}\n${commit.subject}`}
    {#snippet line()}
      <svg class="lanes" width={graphWidth} height={ROW} viewBox="0 0 {graphWidth} {ROW}" aria-hidden="true">
        {#each row.through as line (line.lane)}
          <path d="M{x(line.lane)} 0V{ROW}" stroke={colorOf(line.color)} />
        {/each}
        {#each row.top as line (line.lane)}
          <path d={link(line.lane, row.lane, false)} stroke={colorOf(line.color)} />
        {/each}
        {#each row.bottom as line (line.lane)}
          <path d={link(line.lane, row.lane, true)} stroke={colorOf(line.color)} />
        {/each}
        {#if side === "behind"}
          <!-- Not here yet: only on the remote. -->
          <circle class="dot hollow" cx={x(row.lane)} cy={ROW / 2} r={DOT} stroke={colorOf(row.color)} />
        {:else}
          {#if side === "ahead"}
            <circle class="halo" cx={x(row.lane)} cy={ROW / 2} r={DOT + 2.4} stroke={colorOf(row.color)} />
          {/if}
          <circle class="dot" cx={x(row.lane)} cy={ROW / 2} r={commit.parents.length > 1 ? DOT - 1 : DOT} fill={colorOf(row.color)} />
        {/if}
      </svg>
      <span class="what">
        {#if commit.detachedHead}
          <span class="ref head">HEAD</span>
        {/if}
        {#each commit.refs as ref (ref.kind + ref.name)}
          <span class="ref {ref.kind}" title={t(`git.graph.refs.${ref.kind}`, { name: ref.name })}>{ref.name}</span>
        {/each}
        <span class="subject">{commit.subject}</span>
      </span>
      {#if !compact}
        <span class="author">{commit.author}</span>
      {/if}
      <span class="when">{when(commit.time)}</span>
      {#if !compact}
        <span class="sha">{commit.sha.slice(0, 7)}</span>
      {/if}
    {/snippet}
    <!-- A row to pick in the tab's graph; just a row in the panel's. -->
    {#if onpick}
      <button class="commit-row {side}" class:selected={selected === commit.sha} style="height:{ROW}px" title={hint} onclick={() => onpick(commit)}>
        {@render line()}
      </button>
    {:else}
      <div class="commit-row {side}" style="height:{ROW}px" title={hint}>
        {@render line()}
      </div>
    {/if}
  {/each}
</div>

<style>
  .commit-graph {
    display: flex;
    flex-direction: column;
    font-size: 12px;
  }

  .commit-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    width: 100%;
    box-sizing: border-box;
    padding: 0 10px 0 4px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
  }

  button.commit-row {
    cursor: pointer;
  }

  button.commit-row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
  }

  .commit-row.selected {
    background: rgba(58, 109, 240, 0.18);
    box-shadow: inset 2px 0 0 var(--pane-active-border, #3a6df0);
  }

  .commit-row:focus-visible {
    outline: 1px solid var(--pane-active-border, #3a6df0);
    outline-offset: -1px;
  }

  .lanes {
    flex-shrink: 0;
    overflow: hidden;
  }

  .lanes path {
    fill: none;
    stroke-width: 1.6;
  }

  .dot.hollow {
    fill: var(--tabbar-bg, #1e1e1e);
    stroke-width: 1.6;
  }

  .halo {
    fill: none;
    stroke-width: 1.2;
    opacity: 0.55;
  }

  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 5px;
    overflow: hidden;
  }

  .subject {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Only on the remote: there, but not what's in hand. */
  .commit-row.behind .subject {
    opacity: 0.6;
  }

  .commit-row.ahead .subject {
    font-weight: 600;
  }

  .ref {
    flex-shrink: 1;
    min-width: 24px;
    max-width: 45%;
    padding: 0 5px;
    border: 1px solid currentColor;
    border-radius: 8px;
    font-size: 10px;
    line-height: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ref.head {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-weight: 700;
  }

  .ref.local {
    color: var(--git-ref-local, #5fb86a);
  }

  .ref.remote {
    color: var(--git-ref-remote, #c98a3c);
  }

  .ref.tag {
    border-style: dashed;
    opacity: 0.8;
  }

  .author,
  .when,
  .sha {
    flex-shrink: 0;
    opacity: 0.6;
    white-space: nowrap;
    font-size: 11px;
  }

  .author {
    width: 130px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .commit-graph:not(.compact) .when {
    width: 140px;
  }

  .sha {
    width: 56px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  }

  .compact .when {
    font-size: 10px;
  }
</style>
