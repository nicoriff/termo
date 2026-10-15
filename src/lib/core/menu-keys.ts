/**
 * Keyboard for the app's own menus (the ≡ menu, the tab strip's "˅" menu,
 * context menus): a Svelte action on the menu's container that moves focus
 * through its items the way a native menu does.
 *
 * - On open the first item takes focus (after a click no ring shows: the
 *   browser only draws :focus-visible after keyboard use).
 * - ↑/↓ move and wrap, Home/End jump, a letter jumps to the next item that
 *   starts with it; Enter/Space are the buttons' own.
 * - → (or Enter) on an item with a flyout opens it; the flyout, its own
 *   `menuKeys` with `back`, focuses its first item, and ← or Escape returns.
 * - Escape or Tab closes; then focus goes back to the console.
 *
 * Items are the container's enabled `[role=menuitem]` and buttons, skipping
 * any marked `data-menu-skip` (a row's secondary button, reachable by Shift
 * instead).
 */

import { uiState } from "./ui-state.svelte";

export interface MenuKeysOptions {
  /** Closes the whole menu (Escape, Tab). */
  close: () => void;
  /** A flyout: closes just it (←, Escape), focus back on its row. */
  back?: () => void;
  /** Where focus goes once it closes: the console (the default, for menus
   * over it), or what had it before (a panel's menu, back to its button). */
  returnTo?: "terminal" | "previous";
  /** False where the menu handles Escape itself (one with steps of its own). */
  escape?: boolean;
}

/** The index ↑/↓/Home/End lead to among `count` items, or null for other keys. */
export function stepIndex(key: string, current: number, count: number): number | null {
  if (count === 0) return null;
  switch (key) {
    case "ArrowDown":
      return current < 0 ? 0 : (current + 1) % count;
    case "ArrowUp":
      return current < 0 ? count - 1 : (current - 1 + count) % count;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

/** The next label after `current` (wrapping) that starts with `letter`, or -1. */
export function indexByLetter(labels: string[], current: number, letter: string): number {
  const wanted = letter.toLocaleLowerCase();
  for (let offset = 1; offset <= labels.length; offset++) {
    const index = (current + offset + labels.length) % labels.length;
    if (labels[index].trim().toLocaleLowerCase().startsWith(wanted)) return index;
  }
  return -1;
}

function items(node: HTMLElement): HTMLElement[] {
  return [...node.querySelectorAll<HTMLElement>('[role="menuitem"], button')].filter(
    (item) =>
      !item.hasAttribute("data-menu-skip") &&
      !(item as HTMLButtonElement).disabled &&
      item.getAttribute("aria-disabled") !== "true" &&
      item.getClientRects().length > 0,
  );
}

function focusItem(item: HTMLElement | undefined) {
  item?.focus({ preventScroll: true });
  item?.scrollIntoView({ block: "nearest" });
}

export function menuKeys(node: HTMLElement, initial: MenuKeysOptions) {
  let options = initial;
  const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  // A flyout's row, to go back to. The flyout takes focus only when it was
  // opened from the keyboard, not by the pointer passing over the row.
  const opener = previous?.getAttribute("aria-haspopup") ? previous : null;
  if (!options.back || opener?.matches(":focus-visible")) queueMicrotask(() => focusItem(items(node)[0]));

  function onKeydown(event: KeyboardEvent) {
    if (event.ctrlKey || event.altKey || event.metaKey) return;
    // In a text box inside the menu (a filter), only ↑/↓ leave it.
    const typing = (event.target as HTMLElement).matches("input, textarea, [contenteditable]");
    if (typing && event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    const list = items(node);
    const current = list.indexOf(document.activeElement as HTMLElement);
    const step = stepIndex(event.key, current, list.length);
    if (step !== null) {
      focusItem(list[step]);
    } else if (event.key === "ArrowRight" && list[current]?.getAttribute("aria-haspopup")) {
      if (list[current].getAttribute("aria-expanded") !== "true") list[current].click();
    } else if (event.key === "ArrowLeft" && options.back) {
      options.back();
    } else if (event.key === "Escape" && options.escape !== false) {
      (options.back ?? options.close)();
    } else if (event.key === "Tab") {
      options.close();
    } else if (event.key.length === 1 && event.key !== " ") {
      const index = indexByLetter(
        list.map((item) => item.textContent ?? ""),
        current,
        event.key,
      );
      if (index < 0) return;
      focusItem(list[index]);
    } else {
      return;
    }
    event.preventDefault();
    // The keys are the menu's: not the dialog it may be in, nor the console.
    event.stopPropagation();
  }

  // The pointer and the keyboard share one place in the list: moving the
  // pointer over an item makes the arrows go on from there.
  function onPointerOver(event: PointerEvent) {
    const item = (event.target as HTMLElement).closest<HTMLElement>('[role="menuitem"], button');
    if (item && node.contains(item) && items(node).includes(item)) item.focus({ preventScroll: true });
  }

  node.addEventListener("keydown", onKeydown);
  node.addEventListener("pointerover", onPointerOver);

  return {
    update(next: MenuKeysOptions) {
      options = next;
    },
    destroy() {
      node.removeEventListener("keydown", onKeydown);
      node.removeEventListener("pointerover", onPointerOver);
      // Closed with focus inside, it would fall to the page and typing would
      // go nowhere. Checked a frame later: whatever was picked (a new tab, a
      // dialog) takes focus first, and keeps it.
      requestAnimationFrame(() => {
        const active = document.activeElement;
        if (active && active !== document.body) return;
        if (options.back && opener?.isConnected) opener.focus({ preventScroll: true });
        else if (previous?.isConnected && (options.returnTo === "previous" || previous.closest(".xterm")))
          previous.focus({ preventScroll: true });
        else uiState.refocusTerminals();
      });
    },
  };
}
