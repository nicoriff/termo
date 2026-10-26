<script lang="ts">
  import { onMount, tick } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import SettingDropdown from "$lib/preferences/SettingDropdown.svelte";
  import { statusBarCustomize } from "./customize.svelte";
  import {
    containsItem,
    findItem,
    itemSpec,
    optionValue,
    placeItem,
    removeItem,
    setItemOption,
    STATUS_CATEGORIES,
    STATUS_ITEMS,
    type StatusBarLayout,
    type StatusCategory,
    type StatusItemId,
    type StatusSide,
  } from "./layout";
  import StatusItem from "./StatusItem.svelte";
  import StatusItemSample from "./StatusItemSample.svelte";

  /** The status bar while it's being arranged: the real bar, with its real
   * readings, highlighted over the rest of the window, which dims; above it,
   * a tray of the items there are. Items are dragged from the tray onto the
   * bar, along it, or off it. */
  const layout = $derived(appConfigStore.effectiveStatusBar);
  const customized = $derived(appConfigStore.statusBarLayout !== null);

  /** The kind of items the tray offers, one kind at a time. */
  let category = $state<StatusCategory>("agent");

  let selected = $state<StatusItemId | null>(null);
  const selectedRef = $derived.by(() => {
    if (!selected) return null;
    return [...layout.left, ...layout.right].find((ref) => ref.id === selected) ?? null;
  });
  const selectedSide = $derived(selected ? (findItem(layout, selected)?.side ?? null) : null);

  /** A press on a chip, of the bar or of the tray. It becomes a drag once the
   * pointer has moved a few pixels; before that, letting go is a click. */
  interface Drag {
    id: StatusItemId;
    from: "bar" | "palette";
    startX: number;
    startY: number;
    x: number;
    y: number;
    active: boolean;
  }
  const DRAG_THRESHOLD = 4;
  /** How far above and below an end of the bar still drops into it. */
  const DROP_SLACK = 18;

  let drag = $state<Drag | null>(null);
  let target = $state<{ side: StatusSide; index: number } | null>(null);
  let zones: Record<StatusSide, HTMLElement | null> = $state({ left: null, right: null });
  let barHeight = $state(22);
  let doneButton = $state<HTMLButtonElement | null>(null);

  /** Items with nothing to show right now (no agent in this tab, no repo…):
   * they show their sample, dimmed, so there is something to grab. */
  let empty = $state<Partial<Record<StatusItemId, boolean>>>({});

  function watchEmpty(node: HTMLElement, id: StatusItemId) {
    const observer = new ResizeObserver(() => {
      empty[id] = node.offsetWidth === 0;
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  onMount(() => {
    // Off the console, so keys reach the bar and Escape ends this.
    void tick().then(() => doneButton?.focus());
  });

  function save(next: StatusBarLayout) {
    appConfigStore.setStatusBarLayout(next);
  }

  /** Each end's items as drawn: without the one being dragged, which goes
   * along with the pointer. */
  function shown(side: StatusSide) {
    return layout[side].filter((ref) => !(drag?.active && ref.id === drag.id));
  }

  function onChipDown(event: PointerEvent, id: StatusItemId, from: Drag["from"]) {
    if (event.button !== 0) return;
    event.preventDefault();
    drag = { id, from, startX: event.clientX, startY: event.clientY, x: event.clientX, y: event.clientY, active: false };
  }

  function onPointerMove(event: PointerEvent) {
    if (!drag) return;
    drag.x = event.clientX;
    drag.y = event.clientY;
    if (!drag.active && Math.hypot(drag.x - drag.startX, drag.y - drag.startY) < DRAG_THRESHOLD) return;
    drag.active = true;
    target = dropTarget(event.clientX, event.clientY);
  }

  /** Which end the pointer is over, and before which of its items. */
  function dropTarget(x: number, y: number): { side: StatusSide; index: number } | null {
    for (const side of ["left", "right"] as const) {
      const zone = zones[side];
      if (!zone) continue;
      const rect = zone.getBoundingClientRect();
      if (x < rect.left || x > rect.right || y < rect.top - DROP_SLACK || y > rect.bottom + DROP_SLACK) continue;
      const chips = [...zone.querySelectorAll<HTMLElement>("[data-chip]")];
      const index = chips.findIndex((chip) => {
        const box = chip.getBoundingClientRect();
        return x < box.left + box.width / 2;
      });
      return { side, index: index < 0 ? chips.length : index };
    }
    return null;
  }

  function onPointerUp() {
    if (!drag) return;
    const { id, from, active } = drag;
    const drop = target;
    drag = null;
    target = null;
    if (!active) {
      if (from === "bar" || containsItem(layout, id)) selected = id;
      else add(id);
      return;
    }
    if (drop) {
      save(placeItem(layout, id, drop.side, drop.index));
      selected = id;
    } else if (from === "bar") {
      // Dragged off the bar: taken off it.
      remove(id);
    }
  }

  function onPointerCancel() {
    drag = null;
    target = null;
  }

  /** From the tray by click or key: to the end of the left side, where most
   * of them go. */
  function add(id: StatusItemId) {
    save(placeItem(layout, id, "left", layout.left.length));
    selected = id;
  }

  function remove(id: StatusItemId) {
    save(removeItem(layout, id));
    if (selected === id) selected = null;
  }

  /** One place over, crossing to the other end past the edge. */
  async function nudge(id: StatusItemId, step: -1 | 1) {
    const at = findItem(layout, id);
    if (!at) return;
    const index = at.index + step;
    if (index < 0) {
      if (at.side === "left") return;
      save(placeItem(layout, id, "left", layout.left.length));
    } else if (index >= layout[at.side].length) {
      if (at.side === "right") return;
      save(placeItem(layout, id, "right", 0));
    } else {
      save(placeItem(layout, id, at.side, index));
    }
    await tick();
    document.querySelector<HTMLElement>(`[data-chip="${id}"]`)?.focus();
  }

  function moveToSide(id: StatusItemId, side: StatusSide) {
    save(placeItem(layout, id, side, side === "left" ? layout.left.length : 0));
  }

  function onChipKey(event: KeyboardEvent, id: StatusItemId) {
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      selected = id;
      void nudge(id, event.key === "ArrowLeft" ? -1 : 1);
    } else if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      remove(id);
    }
  }

  function onWindowKey(event: KeyboardEvent) {
    if (event.key !== "Escape" || event.defaultPrevented) return;
    event.preventDefault();
    event.stopPropagation();
    if (drag) onPointerCancel();
    else statusBarCustomize.finish();
  }

  function restoreDefault() {
    appConfigStore.setStatusBarLayout(null);
    selected = null;
  }

  const ghostLabel = $derived(drag?.active ? t(itemSpec(drag.id).label) : "");
  /** Let go here and the item comes off the bar. */
  const removing = $derived(!!drag?.active && drag.from === "bar" && !target);
</script>

<svelte:window
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerCancel}
  onkeydown={onWindowKey}
/>

{#snippet zone(side: StatusSide)}
  <div class="status-side {side}" class:target={target?.side === side} bind:this={zones[side]}>
    {#each shown(side) as ref, index (ref.id)}
      {#if target?.side === side && target.index === index}<span class="drop-marker"></span>{/if}
      <div
        class="chip"
        class:selected={selected === ref.id}
        class:placeholder={empty[ref.id]}
        role="button"
        tabindex="0"
        data-chip={ref.id}
        aria-label={t("preferences.statusBar.chipLabel", { name: t(itemSpec(ref.id).label) })}
        aria-pressed={selected === ref.id}
        title={empty[ref.id] ? t("statusBar.edit.placeholderHint", { name: t(itemSpec(ref.id).label) }) : t(itemSpec(ref.id).description)}
        onpointerdown={(event) => onChipDown(event, ref.id, "bar")}
        onkeydown={(event) => onChipKey(event, ref.id)}
        onfocus={() => (selected = ref.id)}
      >
        <span class="live" use:watchEmpty={ref.id}><StatusItem item={ref} /></span>
        {#if empty[ref.id]}<StatusItemSample item={ref} />{/if}
      </div>
    {/each}
    {#if target?.side === side && target.index >= shown(side).length}<span class="drop-marker"></span>{/if}
    {#if shown(side).length === 0 && target?.side !== side}
      <span class="zone-empty">{t(side === "left" ? "preferences.statusBar.emptyLeft" : "preferences.statusBar.emptyRight")}</span>
    {/if}
  </div>
{/snippet}

<div class="customize-shade" class:removing aria-hidden="true"></div>

<div class="tray" role="dialog" aria-label={t("statusBar.edit.title")} style="bottom:{barHeight + 8}px">
  <div class="tray-row">
    <div class="category-pick">
      <SettingDropdown
        options={STATUS_CATEGORIES.map((value) => ({ value, label: t(`preferences.statusBar.categories.${value}`) }))}
        value={category}
        label={t("statusBar.edit.category")}
        onpick={(value) => (category = value as StatusCategory)}
      />
    </div>
    <div class="palette">
      {#each STATUS_ITEMS.filter((spec) => spec.category === category) as spec (spec.id)}
        {@const inBar = containsItem(layout, spec.id)}
        <button
          class="palette-chip"
          class:in-bar={inBar}
          title={t(spec.description)}
          aria-label={t(inBar ? "preferences.statusBar.inBar" : "preferences.statusBar.add", { name: t(spec.label) })}
          onpointerdown={(event) => onChipDown(event, spec.id, "palette")}
          onkeydown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              if (inBar) selected = spec.id;
              else add(spec.id);
            }
          }}
        >
          {#if inBar}<span class="check" aria-hidden="true">✓</span>{/if}
          {t(spec.label)}
        </button>
      {/each}
    </div>
    <div class="tray-actions">
      <button class="text-button" disabled={!customized} onclick={restoreDefault}>{t("preferences.statusBar.restore")}</button>
      <button class="text-button primary" bind:this={doneButton} onclick={() => statusBarCustomize.finish()}>{t("statusBar.edit.done")}</button>
    </div>
  </div>

  {#if selectedRef}
    {@const spec = itemSpec(selectedRef.id)}
    <div class="details">
      <strong class="details-name" title={t(spec.description)}>{t(spec.label)}</strong>
      <div class="details-controls">
        {#each spec.options ?? [] as option (option.key)}
          {#if option.kind === "toggle"}
            <label class="option">
              <input
                type="checkbox"
                checked={optionValue(selectedRef, option.key) === "on"}
                onchange={(e) => save(setItemOption(layout, selectedRef.id, option.key, e.currentTarget.checked ? "on" : "off"))}
              />
              <span>{t(option.label)}</span>
            </label>
          {:else}
            <div class="option">
              <span>{t(option.label)}</span>
              <SettingDropdown
                options={option.choices.map((choice) => ({ value: choice.value, label: t(choice.label) }))}
                value={optionValue(selectedRef, option.key)}
                label={t(option.label)}
                onpick={(value) => save(setItemOption(layout, selectedRef.id, option.key, value))}
              />
            </div>
          {/if}
        {/each}
        {#if selectedSide === "right"}
          <button class="text-button" onclick={() => moveToSide(selectedRef.id, "left")}>← {t("preferences.statusBar.toLeft")}</button>
        {:else}
          <button class="text-button" onclick={() => moveToSide(selectedRef.id, "right")}>{t("preferences.statusBar.toRight")} →</button>
        {/if}
        <button class="text-button danger" onclick={() => remove(selectedRef.id)}>{t("preferences.statusBar.remove")}</button>
      </div>
    </div>
  {:else}
    <p class="hint">{t("statusBar.edit.hint")}</p>
  {/if}
</div>

<div class="status-bar editing" class:dragging={drag?.active} class:removing bind:offsetHeight={barHeight}>
  {@render zone("left")}
  {@render zone("right")}
</div>

{#if drag?.active}
  <div class="ghost" class:removing style="left:{drag.x + 10}px; top:{drag.y - 30}px">
    {removing ? t("preferences.statusBar.dropToRemove", { name: ghostLabel }) : ghostLabel}
  </div>
{/if}

<style>
  /* Everything but the bar, dimmed: the bar is drawn above it. */
  .customize-shade {
    position: fixed;
    inset: 0;
    z-index: 900;
    background: rgba(0, 0, 0, 0.55);
    transition: background 0.12s;
  }

  .customize-shade.removing {
    background: rgba(40, 0, 0, 0.6);
  }

  .status-bar.editing {
    position: relative;
    z-index: 901;
    user-select: none;
    box-shadow:
      0 0 0 1px var(--pane-active-border, #3a6df0),
      0 -4px 24px rgba(58, 109, 240, 0.35);
  }

  /* The left end takes the room the right one leaves, so either can be
     dropped into even when empty. */
  .editing .status-side {
    gap: 4px;
    min-width: 60px;
    height: 100%;
    border-radius: 3px;
    outline: 1px dashed transparent;
    outline-offset: -1px;
  }

  .editing .status-side.left {
    flex: 1 1 auto;
  }

  .editing.dragging .status-side {
    outline-color: rgba(128, 128, 128, 0.5);
  }

  .editing .status-side.target {
    outline-color: var(--pane-active-border, #3a6df0);
  }

  .editing.removing .status-side {
    outline-color: rgba(224, 90, 90, 0.6);
  }

  .chip {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    height: 18px;
    padding: 0 4px;
    border: 1px solid transparent;
    border-radius: 3px;
    cursor: grab;
    touch-action: none;
  }

  .chip:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .chip.selected {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .chip:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 0;
  }

  .chip.placeholder {
    border-style: dashed;
    border-color: rgba(128, 128, 128, 0.5);
    opacity: 0.55;
  }

  .chip.placeholder.selected {
    border-color: var(--pane-active-border, #3a6df0);
  }

  /* What the items do on a click (menus, copying, fetching) waits until
     the bar is done being arranged. */
  .chip :global(*) {
    pointer-events: none;
  }

  .live {
    display: flex;
    align-items: center;
  }

  .drop-marker {
    flex-shrink: 0;
    width: 2px;
    height: 16px;
    border-radius: 1px;
    background: var(--pane-active-border, #3a6df0);
  }

  .zone-empty {
    padding: 0 6px;
    font-size: 10px;
    opacity: 0.6;
    white-space: nowrap;
  }

  /* Centered without a transform: a transform would make it the box the
     dropdowns' fixed panels are placed in. */
  .tray {
    position: fixed;
    left: 0;
    right: 0;
    z-index: 902;
    width: min(820px, calc(100vw - 32px));
    margin: 0 auto;
    padding: 8px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    background: var(--tabbar-active-bg, #1e1e1e);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.45);
    user-select: none;
  }

  .tray-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .category-pick {
    flex: 0 0 150px;
  }

  .hint {
    margin: 6px 2px 0;
    font-size: 10px;
    opacity: 0.55;
  }

  .tray-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .text-button {
    flex-shrink: 0;
    padding: 4px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .text-button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .text-button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .text-button.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .text-button.primary:hover {
    filter: brightness(1.1);
    background: var(--pane-active-border, #3a6df0);
  }

  .text-button.danger {
    color: #e05a5a;
  }

  .details {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 6px 12px;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
  }

  .details-name {
    font-weight: 600;
    white-space: nowrap;
  }

  .details-controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }

  .option {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }

  .option input {
    accent-color: var(--pane-active-border, #3a6df0);
    cursor: pointer;
  }

  .palette {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .palette-chip {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 3px 9px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 11px;
    background: rgba(255, 255, 255, 0.04);
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: grab;
    touch-action: none;
  }

  .palette-chip:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .palette-chip.in-bar {
    opacity: 0.5;
  }

  .check {
    font-size: 10px;
    color: var(--pane-active-border, #3a6df0);
  }

  .ghost {
    position: fixed;
    z-index: 950;
    padding: 3px 9px;
    border-radius: 11px;
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    font-size: 11px;
    white-space: nowrap;
    pointer-events: none;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.35);
  }

  .ghost.removing {
    background: #c94a4a;
  }
</style>
