<script lang="ts">
  import type { Theme } from "$lib/theme/theme-types";

  /** A theme as a tiny Termo window: its title bar, a tab strip (with its
   * tab colors when it has some), a few lines of console in its colors and
   * its status bar, so what picking it does shows before it's picked. */
  let { theme, selected, onpick }: { theme: Theme; selected: boolean; onpick: () => void } = $props();

  const ui = $derived(theme.ui);
  const c = $derived(theme.colors);

  function paint(stops: string[] | undefined, fallback: string): string {
    if (!stops || stops.length === 0) return fallback;
    if (stops.length === 1) return stops[0];
    return `linear-gradient(90deg, ${stops.join(", ")})`;
  }

  /** The three tabs' colors: the theme's own, else none (plain tabs). */
  const tabs = $derived(ui.tabColors?.slice(0, 3) ?? []);
</script>

<button
  class="card"
  class:selected
  aria-pressed={selected}
  style="--accent:{ui.accent}; --fg:{c.foreground}"
  onclick={onpick}
>
  <span
    class="window"
    style="background:{c.background}; border-color:{ui.border}; {ui.glow ? `box-shadow: 0 0 10px ${ui.glow}55` : ''}"
  >
    <span class="titlebar" style="background:{paint(ui.titlebarGradient, ui.background)}">
      <i style="background:{ui.foreground}"></i>
      <i style="background:{ui.foreground}"></i>
      <i style="background:{ui.foreground}"></i>
    </span>
    <span class="tabstrip" style="background:{paint(ui.tabbarGradient, ui.background)}; border-color:{ui.border}">
      {#each [0, 1, 2] as index (index)}
        {@const color = tabs[index]}
        <span
          class="tab"
          class:active={index === 0}
          style="background:{index === 0
            ? color
              ? `color-mix(in srgb, ${color} 30%, ${ui.activeBackground})`
              : ui.activeBackground
            : color
              ? `color-mix(in srgb, ${color} 12%, transparent)`
              : 'transparent'}; border-bottom-color:{color ?? (index === 0 ? ui.accent : 'transparent')}"
        >
          <span class="tab-text" style="background:{index === 0 ? ui.activeForeground : ui.foreground}"></span>
        </span>
      {/each}
    </span>
    <span class="console">
      <span class="line">
        <b style="background:{c.green}; width:14%"></b>
        <b style="background:{c.blue}; width:22%"></b>
        <b style="background:{c.foreground}; width:30%"></b>
      </span>
      <span class="line">
        <b style="background:{c.foreground}; width:46%; opacity:0.75"></b>
        <b style="background:{c.yellow}; width:18%"></b>
      </span>
      <span class="line">
        <b style="background:{c.red}; width:28%"></b>
        <b style="background:{c.magenta}; width:16%"></b>
        <b style="background:{c.cyan}; width:12%"></b>
      </span>
      <span class="line">
        <b style="background:{c.green}; width:10%"></b>
        <b class="cursor" style="background:{c.cursor}"></b>
      </span>
    </span>
    <span
      class="statusbar"
      style="background:{paint(ui.statusBar?.background, ui.background)}; border-color:{ui.border}"
    >
      <b style="background:{ui.statusBar?.foreground ?? ui.foreground}; width:22%"></b>
      <b style="background:{ui.statusBar?.foreground ?? ui.foreground}; width:12%; margin-left:auto"></b>
    </span>
  </span>
  <span class="label">
    <span class="name">{theme.name}</span>
    {#if selected}
      <svg class="check" viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="8" />
        <path d="M4.5 8.2 7 10.6l4.6-5" />
      </svg>
    {/if}
  </span>
</button>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: 6px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
    text-align: left;
    transition:
      border-color 120ms ease,
      background 120ms ease,
      transform 160ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .card:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.06));
    transform: translateY(-2px);
  }

  .card:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 1px;
  }

  .card.selected {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  /* The window: 16:10, rounded like Termo's own. */
  .window {
    display: flex;
    flex-direction: column;
    aspect-ratio: 16 / 10;
    overflow: hidden;
    border: 1px solid;
    border-radius: 5px;
  }

  .titlebar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 3px;
    height: 9%;
    min-height: 7px;
    padding: 0 5px;
  }

  .titlebar i {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    opacity: 0.55;
  }

  .tabstrip {
    display: flex;
    height: 12%;
    min-height: 9px;
    border-bottom: 1px solid;
  }

  .tab {
    display: flex;
    align-items: center;
    width: 26%;
    padding: 0 5px;
    border-bottom: 2px solid transparent;
  }

  .tab-text {
    display: block;
    width: 70%;
    height: 2px;
    border-radius: 1px;
    opacity: 0.7;
  }

  .tab.active .tab-text {
    opacity: 1;
  }

  .console {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 9%;
    padding: 0 8px;
  }

  .line {
    display: flex;
    gap: 4px;
    height: 3px;
  }

  .line b,
  .statusbar b {
    display: block;
    height: 100%;
    border-radius: 1.5px;
  }

  .line b.cursor {
    width: 4px;
  }

  .statusbar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 8%;
    min-height: 6px;
    padding: 0 5px;
    border-top: 1px solid;
  }

  .statusbar b {
    height: 2px;
    opacity: 0.8;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 0 2px;
    font-size: 11.5px;
  }

  .name {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .check {
    width: 13px;
    height: 13px;
    flex-shrink: 0;
    margin-left: auto;
  }

  .check circle {
    fill: #22a55b;
  }

  .check path {
    fill: none;
    stroke: #ffffff;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
