<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import {
    DURATIONS,
    durationLabel,
    keepAwakeState,
    loadPrefs,
    minutesLeft,
    onKeepAwake,
    remaining,
    savePrefs,
    setKeepAwake,
    type KeepAwakeState,
  } from "./keep-awake";

  let session = $state<KeepAwakeState>({ active: false, display: false, until: null });
  let prefs = $state(loadPrefs());
  let error = $state("");
  let menuOpen = $state(false);
  let buttonEl = $state<HTMLButtonElement | undefined>(undefined);
  let now = $state(Date.now());
  let unlisten: (() => void) | null = null;
  let ticker: ReturnType<typeof setInterval> | null = null;

  const MENU_WIDTH = 250;
  const MARGIN = 8;

  const pos = $derived.by(() => {
    if (!buttonEl || !menuOpen) return { top: 0, left: 0 };
    const box = buttonEl.getBoundingClientRect();
    return {
      top: box.bottom + 2,
      left: Math.max(MARGIN, Math.min(box.right - MENU_WIDTH, window.innerWidth - MENU_WIDTH - MARGIN)),
    };
  });

  const left = $derived(session.active && session.until !== null ? remaining(session.until, now) : null);
  const tooltip = $derived(
    session.active
      ? `${left ? t("titlebar.keepAwake.onFor", { time: left }) : t("titlebar.keepAwake.onForever")} · ${t("titlebar.keepAwake.hint")}`
      : `${t("titlebar.keepAwake.off")} · ${t("titlebar.keepAwake.hint")}`,
  );

  onMount(() => {
    void keepAwakeState()
      .then((value) => (session = value))
      .catch(() => {});
    void onKeepAwake((value) => (session = value)).then((stop) => (unlisten = stop));
    // Enough for a countdown shown in minutes.
    ticker = setInterval(() => (now = Date.now()), 20_000);
  });

  onDestroy(() => {
    unlisten?.();
    if (ticker) clearInterval(ticker);
  });

  async function apply(active: boolean, display: boolean, minutes: number | null) {
    error = "";
    try {
      session = await setKeepAwake(active, display, minutes);
      now = Date.now();
    } catch (reason) {
      error = String(reason);
      // A failure shows where the options are.
      menuOpen = true;
    }
  }

  function toggle() {
    void apply(!session.active, prefs.display, prefs.minutes);
  }

  function choose(minutes: number | null) {
    prefs = { ...prefs, minutes };
    savePrefs(prefs);
    menuOpen = false;
    void apply(true, prefs.display, minutes);
  }

  function setDisplay(display: boolean) {
    prefs = { ...prefs, display };
    savePrefs(prefs);
    // A session going on keeps its time left.
    if (session.active) void apply(true, display, minutesLeft(session.until, Date.now()));
  }

  function openMenu(event: MouseEvent) {
    event.preventDefault();
    now = Date.now();
    menuOpen = !menuOpen;
  }
</script>

<button
  class="keep-awake"
  class:active={session.active}
  aria-label={t("titlebar.keepAwake.label")}
  aria-pressed={session.active}
  title={tooltip}
  bind:this={buttonEl}
  onclick={toggle}
  oncontextmenu={openMenu}
>
  <!-- Cup, handle and saucer span x 2–14 and y 4–12: centered on (8, 8)
       like the other buttons' glyphs, with or without the steam, which
       only rises above it while on. -->
  <svg class="cup" viewBox="0 0 16 16" width="18" height="18" aria-hidden="true">
    {#if session.active}
      <path
        d="M5.5 1c-.5.6-.5 1.1 0 1.8M7 1c-.5.6-.5 1.1 0 1.8M8.5 1c-.5.6-.5 1.1 0 1.8"
        fill="none"
        stroke="currentColor"
        stroke-width="0.9"
        stroke-linecap="round"
      />
    {/if}
    <path
      d="M3 4h8v3.5a3 3 0 0 1-3 3h-2a3 3 0 0 1-3-3z"
      fill={session.active ? "currentColor" : "none"}
      stroke="currentColor"
      stroke-width="1"
      stroke-linejoin="round"
    />
    <path d="M11 5h1a1.75 1.75 0 0 1 0 3.5h-1" fill="none" stroke="currentColor" stroke-width="1" />
    <path d="M2.5 12h9" stroke="currentColor" stroke-width="1" stroke-linecap="round" />
  </svg>
  {#if left}<span class="left">{left}</span>{/if}
</button>

{#if menuOpen}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="backdrop" onpointerdown={() => (menuOpen = false)}></div>
  <div class="menu" style="top:{pos.top}px; left:{pos.left}px" role="menu" aria-label={t("titlebar.keepAwake.label")}>
    <div class="menu-title">{t("titlebar.keepAwake.label")}</div>
    <p class="menu-status">
      {session.active ? (left ? t("titlebar.keepAwake.onFor", { time: left }) : t("titlebar.keepAwake.onForever")) : t("titlebar.keepAwake.off")}
    </p>
    <div class="menu-section">{t("titlebar.keepAwake.duration")}</div>
    {#each DURATIONS as minutes (minutes ?? "forever")}
      {@const label = durationLabel(minutes)}
      <button class="menu-item" role="menuitemradio" aria-checked={prefs.minutes === minutes} onclick={() => choose(minutes)}>
        <span class="check">{prefs.minutes === minutes ? "✓" : ""}</span>
        {t(label.key, label.params)}
      </button>
    {/each}
    <div class="menu-divider"></div>
    <label class="menu-item toggle">
      <input type="checkbox" checked={prefs.display} onchange={(e) => setDisplay(e.currentTarget.checked)} />
      {t("titlebar.keepAwake.display")}
    </label>
    {#if session.active}
      <div class="menu-divider"></div>
      <button class="menu-item" role="menuitem" onclick={() => ((menuOpen = false), void apply(false, prefs.display, null))}>
        <span class="check"></span>
        {t("titlebar.keepAwake.turnOff")}
      </button>
    {/if}
    <p class="menu-note">{t("titlebar.keepAwake.note")}</p>
    {#if error}<p class="menu-error">{error}</p>{/if}
  </div>
{/if}

<style>
  .keep-awake {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    /* The width of the bar's other buttons; wider only with a countdown. */
    min-width: 44px;
    padding: 0 10px;
    box-sizing: border-box;
    border: none;
    background: transparent;
    color: inherit;
    opacity: 0.75;
    cursor: pointer;
  }

  .keep-awake:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .keep-awake.active {
    opacity: 1;
    color: var(--warning-fg, #d7ba7d);
  }

  .cup {
    display: block;
    flex-shrink: 0;
  }

  .left {
    font-size: 11px;
    line-height: 1;
    white-space: nowrap;
  }

  /* The rounded, compact buttons of the macOS and GNOME title bars. */
  :global(.titlebar.mac) .keep-awake,
  :global(.titlebar.linux) .keep-awake {
    align-self: center;
    height: 24px;
    min-width: 28px;
    border-radius: 6px;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 601;
  }

  .menu {
    position: fixed;
    z-index: 602;
    width: 250px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    background: var(--tabbar-active-bg, #1e1e1e);
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
  }

  .menu-title {
    padding: 6px 10px 0;
    font-weight: 600;
  }

  .menu-status {
    margin: 2px 10px 6px;
    font-size: 11px;
    opacity: 0.7;
  }

  .menu-section {
    padding: 4px 10px 2px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.5;
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .menu-item:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .check {
    width: 12px;
    flex-shrink: 0;
    text-align: center;
  }

  .toggle input {
    width: 12px;
    margin: 0;
  }

  .menu-divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--tabbar-border, #2a2a2a);
  }

  .menu-note,
  .menu-error {
    margin: 6px 10px 4px;
    font-size: 10.5px;
    line-height: 1.4;
  }

  .menu-note {
    opacity: 0.5;
  }

  .menu-error {
    color: var(--error-fg, #f48771);
    overflow-wrap: anywhere;
  }
</style>
