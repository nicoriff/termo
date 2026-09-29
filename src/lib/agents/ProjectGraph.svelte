<script lang="ts">
  import type { AgentSpec } from "$lib/core/agents-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { AUTONOMY_TITLE_KEYS } from "./agent-spec";
  import { GRAPH, layoutGraph } from "./project-spec";

  let {
    agents,
    entryId,
    selected = null,
    colorOf = null,
    onselect,
  }: {
    agents: AgentSpec[];
    entryId: string;
    selected?: string | null;
    /** A status color per agent, for the dot on each node. */
    colorOf?: ((agentId: string) => string) | null;
    onselect?: (agentId: string) => void;
  } = $props();

  const PAD = 14;
  /** Room above the first row for the entry agent's label. */
  const TOP = 14;
  const uid = crypto.randomUUID().slice(0, 8);
  const layout = $derived(layoutGraph(agents, entryId));
  const positions = $derived(new Map(layout.nodes.map((node) => [node.agent.id, node])));

  function short(text: string, max = 18): string {
    return text.length > max ? `${text.slice(0, max - 1)}…` : text;
  }

  /** From the right edge of one node to the left edge of the other; a
   * connection going back (or down the same column) bows out to the side so
   * it doesn't run through the nodes. */
  function edgePath(fromId: string, toId: string): string {
    const from = positions.get(fromId)!;
    const to = positions.get(toId)!;
    const y1 = from.y + GRAPH.nodeHeight / 2 + TOP;
    const y2 = to.y + GRAPH.nodeHeight / 2 + TOP;
    if (to.x > from.x) {
      const x1 = from.x + GRAPH.nodeWidth;
      const x2 = to.x - 4;
      const bend = (x2 - x1) / 2;
      return `M${x1},${y1} C${x1 + bend},${y1} ${x2 - bend},${y2} ${x2},${y2}`;
    }
    const x1 = from.x + GRAPH.nodeWidth;
    const x2 = to.x + GRAPH.nodeWidth + 4;
    const out = 34;
    return `M${x1},${y1} C${x1 + out},${y1} ${x2 + out},${y2} ${x2},${y2}`;
  }
</script>

<div class="graph">
  <svg
    width={layout.width + PAD * 2 + 40}
    height={layout.height + PAD * 2 + TOP}
    viewBox="{-PAD} {-PAD} {layout.width + PAD * 2 + 40} {layout.height + PAD * 2 + TOP}"
    role="img"
    aria-label={t("agents.projects.review.graphLabel")}
  >
    <defs>
      <marker id="arrow-{uid}" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto">
        <path d="M0,0 L8,4 L0,8 z" fill="currentColor" />
      </marker>
    </defs>
    {#each layout.edges as edge (`${edge.from}-${edge.to}-${edge.kind}`)}
      <path
        class="edge {edge.kind}"
        d={edgePath(edge.from, edge.to)}
        fill="none"
        stroke="currentColor"
        marker-end="url(#arrow-{uid})"
      />
    {/each}
    {#each layout.nodes as node (node.agent.id)}
      <!-- A button only when something listens (onselect): the role and the
           tabindex come and go together. -->
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_tabindex -->
      <g
        class="node"
        class:selected={selected === node.agent.id}
        class:clickable={Boolean(onselect)}
        class:paused={!node.agent.enabled}
        transform="translate({node.x},{node.y + TOP})"
        role={onselect ? "button" : undefined}
        tabindex={onselect ? 0 : undefined}
        onclick={() => onselect?.(node.agent.id)}
        onkeydown={(e) => e.key === "Enter" && onselect?.(node.agent.id)}
      >
        <title>{node.agent.name}</title>
        {#if node.agent.id === entryId}
          <text class="entry" x="2" y="-5">▶ {t("agents.projects.review.entryBadge")}</text>
        {/if}
        <rect width={GRAPH.nodeWidth} height={GRAPH.nodeHeight} rx="7" />
        <text class="icon" x="12" y={GRAPH.nodeHeight / 2 + 5}>{node.agent.icon || "🤖"}</text>
        <text class="name" x="34" y={GRAPH.nodeHeight / 2 - 2}>{short(node.agent.name || "…")}</text>
        <text class="level" x="34" y={GRAPH.nodeHeight / 2 + 12}>
          {t(AUTONOMY_TITLE_KEYS[node.agent.autonomy])}
        </text>
        {#if colorOf}
          <circle cx={GRAPH.nodeWidth - 10} cy="10" r="4" fill={colorOf(node.agent.id)} />
        {/if}
      </g>
    {/each}
  </svg>
  <div class="legend">
    <span><svg width="26" height="8" aria-hidden="true"><path d="M1,4 H25" stroke="currentColor" /></svg>
      {t("agents.projects.review.legendAfter")}</span
    >
    <span><svg width="26" height="8" aria-hidden="true"><path d="M1,4 H25" stroke="currentColor" stroke-dasharray="4 3" /></svg>
      {t("agents.projects.review.legendAsks")}</span
    >
  </div>
</div>

<style>
  .graph {
    overflow: auto;
    padding: 4px 0;
  }

  svg {
    display: block;
    color: inherit;
    overflow: visible;
  }

  .edge {
    stroke-width: 1.3;
    opacity: 0.55;
  }

  .edge.asks {
    stroke-dasharray: 5 4;
  }

  .node rect {
    fill: var(--terminal-bg, rgba(0, 0, 0, 0.25));
    stroke: var(--tabbar-border, #3a3a3a);
    stroke-width: 1;
  }

  .node.clickable {
    cursor: pointer;
  }

  .node.clickable:hover rect,
  .node:focus-visible rect {
    stroke: var(--pane-active-border, #3a6df0);
    outline: none;
  }

  .node:focus {
    outline: none;
  }

  .node.selected rect {
    stroke: var(--pane-active-border, #3a6df0);
    stroke-width: 2;
  }

  .node.paused {
    opacity: 0.75;
  }

  text {
    fill: currentColor;
    font-family: inherit;
  }

  .icon {
    font-size: 15px;
  }

  .name {
    font-size: 12px;
    font-weight: 600;
  }

  .level,
  .entry {
    font-size: 10px;
    opacity: 0.6;
  }

  .legend {
    display: flex;
    gap: 16px;
    margin-top: 6px;
    font-size: 11px;
    opacity: 0.6;
  }

  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .legend svg {
    opacity: 0.8;
  }
</style>
