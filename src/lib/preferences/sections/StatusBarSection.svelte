<script lang="ts">
  import "$lib/statusbar/status-bar.css";
  import { tick } from "svelte";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import StatusItemSample from "$lib/statusbar/StatusItemSample.svelte";
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
    type StatusItemId,
    type StatusSide,
  } from "$lib/statusbar/layout";
  import SettingDropdown from "../SettingDropdown.svelte";

  const layout = $derived(appConfigStore.effectiveStatusBar);
  const customized = $derived(appConfigStore.statusBarLayout !== null);

  let selected = $state<StatusItemId | null>(null);
  const selectedRef = $derived.by(() => {
    if (!selected) return null;
    return [...layout.left, ...layout.right].find((ref) => ref.id === selected) ?? null;
  });
  const selectedSide = $derived(selected ? (findItem(layout, selected)?.side ?? null) : null);

  /** A press on a chip, of the bar or of the list below it. It becomes a
   * drag once the pointer has moved a few pixels; before that, letting go
   * is a click. */
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
      // Wrapped onto a second row: the rows below the pointer's come after it.
      const index = chips.findIndex((chip) => {
        const box = chip.getBoundingClientRect();
        if (y < box.top) return true;
        return y <= box.bottom && x < box.left + box.width / 2;
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
      // Dragged out of the bar: taken off it.
      remove(id);
    }
  }

  function onPointerCancel() {
    drag = null;
    target = null;
  }

  /** From the list: to the end of the left side, where most of them go. */
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

  function restoreDefault() {
    appConfigStore.setStatusBarLayout(null);
    selected = null;
  }

  const ghostLabel = $derived(drag?.active ? t(itemSpec(drag.id).label) : "");
  /** Let go here and the item comes off the bar. */
  const removing = $derived(!!drag?.active && drag.from === "bar" && !target);
</script>

<svelte:window onpointermove={onPointerMove} onpointerup={onPointerUp} onpointercancel={onPointerCancel} />

{#snippet zone(side: StatusSide)}
  <div class="status-side {side}" class:target={target?.side === side} bind:this={zones[side]}>
    {#each shown(side) as ref, index (ref.id)}
      {#if target?.side === side && target.index === index}<span class="drop-marker"></span>{/if}
      <button
        class="chip"
        class:selected={selected === ref.id}
        data-chip={ref.id}
        aria-label={t("preferences.statusBar.chipLabel", { name: t(itemSpec(ref.id).label) })}
        aria-pressed={selected === ref.id}
        title={t(itemSpec(ref.id).description)}
        onpointerdown={(event) => onChipDown(event, ref.id, "bar")}
        onkeydown={(event) => onChipKey(event, ref.id)}
        onfocus={() => (selected = ref.id)}
      >
        <StatusItemSample item={ref} />
      </button>
    {/each}
    {#if target?.side === side && target.index >= shown(side).length}<span class="drop-marker"></span>{/if}
    {#if shown(side).length === 0 && target?.side !== side}
      <span class="zone-empty">{t(side === "left" ? "preferences.statusBar.emptyLeft" : "preferences.statusBar.emptyRight")}</span>
    {/if}
  </div>
{/snippet}

<section class="preview-section">
  <h3>{t("preferences.statusBar.preview")}</h3>
  <p class="setting-hint first">{t("preferences.statusBar.previewHint")}</p>
  <div class="status-bar preview" class:dragging={drag?.active} class:removing>
    {@render zone("left")}
    {@render zone("right")}
  </div>
  <div class="preview-actions">
    <span class="setting-hint">{t("preferences.statusBar.keysHint")}</span>
    <button class="text-button" disabled={!customized} onclick={restoreDefault}>{t("preferences.statusBar.restore")}</button>
  </div>
</section>

{#if selectedRef}
  {@const spec = itemSpec(selectedRef.id)}
  <section class="details">
    <h3>{t(spec.label)}</h3>
    <p class="setting-hint first">{t(spec.description)}</p>
    {#each spec.options ?? [] as option (option.key)}
      {#if option.kind === "toggle"}
        <label class="row toggle-row">
          <span>{t(option.label)}</span>
          <input
            type="checkbox"
            checked={optionValue(selectedRef, option.key) === "on"}
            onchange={(e) => save(setItemOption(layout, selectedRef.id, option.key, e.currentTarget.checked ? "on" : "off"))}
          />
        </label>
      {:else}
        <div class="row">
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
    <div class="detail-actions">
      {#if selectedSide === "right"}
        <button class="text-button" onclick={() => moveToSide(selectedRef.id, "left")}>← {t("preferences.statusBar.toLeft")}</button>
      {:else}
        <button class="text-button" onclick={() => moveToSide(selectedRef.id, "right")}>{t("preferences.statusBar.toRight")} →</button>
      {/if}
      <button class="text-button danger" onclick={() => remove(selectedRef.id)}>{t("preferences.statusBar.remove")}</button>
    </div>
  </section>
{/if}

<section>
  <h3>{t("preferences.statusBar.available")}</h3>
  <p class="setting-hint first">{t("preferences.statusBar.availableHint")}</p>
  {#each STATUS_CATEGORIES as category (category)}
    <div class="category">
      <span class="category-name">{t(`preferences.statusBar.categories.${category}`)}</span>
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
    </div>
  {/each}
</section>

{#if drag?.active}
  <div class="ghost" class:removing style="left:{drag.x + 10}px; top:{drag.y + 10}px">
    {removing ? t("preferences.statusBar.dropToRemove", { name: ghostLabel }) : ghostLabel}
  </div>
{/if}

<style>
  section {
    margin-bottom: 20px;
  }

  /* The bar stays in view while what's below it scrolls: stuck to the top
     of the pane, over its padding, on the pane's own background. */
  .preview-section {
    position: sticky;
    top: -16px;
    z-index: 5;
    margin: -16px 0 16px;
    padding: 16px 0 12px;
    background: var(--terminal-bg, #1e1e1e);
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
  }

  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.6;
    margin: 0 0 8px;
  }

  .setting-hint {
    margin: 6px 0 0;
    font-size: 10px;
    opacity: 0.6;
  }

  .setting-hint.first {
    margin: 0 0 10px;
  }

  /* The bar, wrapped where it doesn't fit: the window is narrower than the
     one it stands for. */
  .preview.status-bar {
    height: auto;
    min-height: 34px;
    padding: 5px 8px;
    gap: 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
    user-select: none;
  }

  .preview .status-side {
    flex-wrap: wrap;
    row-gap: 4px;
    gap: 4px;
    min-height: 24px;
    border-radius: 4px;
    outline: 1px dashed transparent;
    outline-offset: 2px;
  }

  /* Each end gets its share of the width and wraps within it. */
  .preview .status-side.left {
    flex: 1 1 55%;
    min-width: 0;
    overflow: visible;
  }

  .preview .status-side.right {
    flex: 1 1 45%;
    min-width: 0;
    margin-left: 0;
    padding-left: 0;
    justify-content: flex-end;
  }

  .preview.dragging .status-side {
    outline-color: rgba(128, 128, 128, 0.45);
  }

  .preview .status-side.target {
    outline-color: var(--pane-active-border, #3a6df0);
  }

  .preview.removing .status-side {
    outline-color: rgba(224, 90, 90, 0.6);
  }

  .chip {
    flex-shrink: 0;
    max-width: 100%;
    display: flex;
    align-items: center;
    padding: 2px 5px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: none;
    color: inherit;
    font: inherit;
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
    outline-offset: 1px;
  }

  /* Inside a chip the samples are only something to look at. */
  .chip :global(*) {
    pointer-events: none;
  }

  .drop-marker {
    align-self: stretch;
    width: 2px;
    min-height: 16px;
    border-radius: 1px;
    background: var(--pane-active-border, #3a6df0);
  }

  .zone-empty {
    padding: 0 6px;
    font-size: 10px;
    opacity: 0.5;
  }

  .preview-actions,
  .detail-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 8px;
  }

  .detail-actions {
    justify-content: flex-start;
    margin-top: 12px;
  }

  .preview-actions .setting-hint {
    margin: 0;
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

  .text-button.danger {
    color: #e05a5a;
  }

  .details {
    padding: 10px 12px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 6px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 8px;
    font-size: 12px;
  }

  .toggle-row {
    cursor: pointer;
  }

  .toggle-row input {
    accent-color: var(--pane-active-border, #3a6df0);
    flex-shrink: 0;
    cursor: pointer;
  }

  .category {
    margin-top: 10px;
  }

  .category-name {
    display: block;
    margin-bottom: 5px;
    font-size: 11px;
    opacity: 0.75;
  }

  .palette {
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
    background: var(--tabbar-active-bg, rgba(255, 255, 255, 0.04));
    color: inherit;
    font: inherit;
    font-size: 11px;
    cursor: grab;
    user-select: none;
    touch-action: none;
  }

  .palette-chip:hover {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .palette-chip.in-bar {
    opacity: 0.55;
  }

  .check {
    font-size: 10px;
    color: var(--pane-active-border, #3a6df0);
  }

  .ghost {
    position: fixed;
    z-index: 100;
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
