/**
 * Pure keyboard-shortcut logic: parsing/serializing the canonical keybinding
 * string format, matching it against a live key event, and formatting it for
 * display. Deliberately has zero dependency on `app-config.svelte.ts` (that
 * module imports from here instead) and types its event parameter as the
 * structural `KeyLike` rather than DOM's `KeyboardEvent`, so all of this can
 * be unit-tested in plain Node — no jsdom needed (same approach already used
 * by `src/lib/ai-mode/classifier.ts`).
 *
 * Canonical string format: modifiers sorted alphabetically, joined with "+",
 * followed by the key, e.g. "ctrl+shift+tab", "meta+t". The zoom/restore
 * action is the one special case — the user only ever holds a modifier while
 * pressing an arrow, and either arrow means the same thing (up = zoom, down
 * = restore) — so its binding uses the fixed placeholder key "arrow" (e.g.
 * "meta+arrow") and matches either ArrowUp or ArrowDown; which one actually
 * fired is read straight off the event by the caller, not decided here.
 */

import type { MessageKey } from "../i18n/translate";

export type ShortcutAction =
  | "newTab"
  | "cycleTab"
  | "zoomTile"
  | "aiRunning"
  | "aiAwaiting"
  | "aiError"
  | "aiFinished"
  | "aiNextPending";

export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  "newTab",
  "cycleTab",
  "zoomTile",
  "aiRunning",
  "aiAwaiting",
  "aiError",
  "aiFinished",
  "aiNextPending",
];

/** Catalog keys (translated by the UI with `t()`), keeping this module free of
 * the reactive i18n store so it stays testable in plain Node. */
export const SHORTCUT_LABEL_KEYS: Record<ShortcutAction, MessageKey> = {
  newTab: "shortcuts.labels.newTab",
  cycleTab: "shortcuts.labels.cycleTab",
  zoomTile: "shortcuts.labels.zoomTile",
  aiRunning: "shortcuts.labels.aiRunning",
  aiAwaiting: "shortcuts.labels.aiAwaiting",
  aiError: "shortcuts.labels.aiError",
  aiFinished: "shortcuts.labels.aiFinished",
  aiNextPending: "shortcuts.labels.aiNextPending",
};

export type ModifierName = "alt" | "ctrl" | "meta" | "shift";

const MODIFIER_NAMES: ModifierName[] = ["alt", "ctrl", "meta", "shift"];

const ARROW_KEY = "arrow";

/** Structural stand-in for `KeyboardEvent` — anything with these fields
 * (real or synthetic) can be matched/recorded, so tests don't need jsdom. */
export interface KeyLike {
  key: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

interface ParsedBinding {
  modifiers: Set<ModifierName>;
  key: string;
}

function activeModifiers(event: KeyLike): Set<ModifierName> {
  const mods = new Set<ModifierName>();
  if (event.altKey) mods.add("alt");
  if (event.ctrlKey) mods.add("ctrl");
  if (event.metaKey) mods.add("meta");
  if (event.shiftKey) mods.add("shift");
  return mods;
}

export function serializeKeybinding(modifiers: Iterable<ModifierName>, key: string): string {
  const sorted = MODIFIER_NAMES.filter((m) => new Set(modifiers).has(m));
  return [...sorted, key.toLowerCase()].join("+");
}

export function parseKeybinding(binding: string): ParsedBinding {
  const parts = binding.split("+");
  const key = parts.pop() ?? "";
  const modifiers = new Set<ModifierName>();
  for (const part of parts) {
    if ((MODIFIER_NAMES as string[]).includes(part)) modifiers.add(part as ModifierName);
  }
  return { modifiers, key };
}

export interface MatchOptions {
  /** Modifiers that must NOT be active on the event, regardless of whether
   * the binding itself requires them — e.g. Alt for "newTab", so that
   * whatever the user remaps it to, an AltGr chord (reported as Ctrl+Alt)
   * can never also be interpreted as "new tab". */
  exclude?: ModifierName[];
  /** Modifiers excluded from the modifier-set comparison entirely, on both
   * sides — e.g. Shift for "cycleTab", which flips direction (forward vs.
   * backward) without changing which shortcut it is: Ctrl+Tab and
   * Ctrl+Shift+Tab both count as "the cycle-tab shortcut", and the caller
   * reads event.shiftKey itself to decide which way to cycle. */
  ignore?: ModifierName[];
}

export function eventMatchesKeybinding(event: KeyLike, binding: string, options?: MatchOptions): boolean {
  const parsed = parseKeybinding(binding);
  const eventMods = activeModifiers(event);

  if (options?.exclude?.some((mod) => eventMods.has(mod))) return false;

  for (const mod of options?.ignore ?? []) {
    eventMods.delete(mod);
    parsed.modifiers.delete(mod);
  }

  if (eventMods.size !== parsed.modifiers.size) return false;
  for (const mod of parsed.modifiers) {
    if (!eventMods.has(mod)) return false;
  }

  if (parsed.key === ARROW_KEY) {
    return event.key === "ArrowUp" || event.key === "ArrowDown";
  }
  return event.key.toLowerCase() === parsed.key;
}

/** Keys that produce a visible character on their own — recording one of
 * these with no modifier held is almost always the user just typing, not
 * trying to set a shortcut, so `normalizeEventToBinding` rejects it. */
function isPrintableKey(key: string): boolean {
  return key.length === 1;
}

const NON_RECORDABLE_KEYS = new Set(["Control", "Alt", "Meta", "Shift", "AltGraph"]);

/** Turns a keydown event into a canonical binding string for the shortcut
 * recorder, or `null` if the event shouldn't be captured at all (a bare
 * modifier press, or an unmodified printable key that's more likely typing
 * than an attempt to record a shortcut). */
export function normalizeEventToBinding(event: KeyLike): string | null {
  if (NON_RECORDABLE_KEYS.has(event.key)) return null;
  const mods = activeModifiers(event);
  if (mods.size === 0 && isPrintableKey(event.key)) return null;
  return serializeKeybinding(mods, event.key);
}

/** Like `normalizeEventToBinding`, but for the zoom/restore row: only an
 * arrow key is a legal recording (any other key is rejected with a message
 * key the recorder UI translates), and at least one modifier is required so
 * plain arrow keys keep doing whatever they already do (e.g. scrollback). */
export function normalizeEventToZoomBinding(event: KeyLike): { binding: string } | { errorKey: MessageKey } {
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") {
    return { errorKey: "shortcuts.zoomNeedsArrow" };
  }
  const mods = activeModifiers(event);
  if (mods.size === 0) {
    return { errorKey: "shortcuts.zoomNeedsModifier" };
  }
  return { binding: serializeKeybinding(mods, ARROW_KEY) };
}

const DEFAULT_KEYBINDINGS: Record<ShortcutAction, { mac: string; other: string }> = {
  newTab: { mac: "meta+t", other: "ctrl+t" },
  cycleTab: { mac: "ctrl+tab", other: "ctrl+tab" },
  zoomTile: { mac: serializeKeybinding(["meta"], ARROW_KEY), other: serializeKeybinding(["alt"], ARROW_KEY) },
  aiRunning: { mac: "meta+1", other: "ctrl+1" },
  aiAwaiting: { mac: "meta+2", other: "ctrl+2" },
  aiError: { mac: "meta+3", other: "ctrl+3" },
  aiFinished: { mac: "meta+4", other: "ctrl+4" },
  aiNextPending: { mac: "meta+shift+a", other: "ctrl+shift+a" },
};

export function getDefaultKeybinding(action: ShortcutAction, isMacPlatform: boolean): string {
  return isMacPlatform ? DEFAULT_KEYBINDINGS[action].mac : DEFAULT_KEYBINDINGS[action].other;
}

export function resolveKeybinding(
  action: ShortcutAction,
  overrides: Partial<Record<ShortcutAction, string | null>>,
  isMacPlatform: boolean,
): string {
  return overrides[action] ?? getDefaultKeybinding(action, isMacPlatform);
}

/** Returns the action whose EFFECTIVE binding (override or default) already
 * uses `binding`, if any other than `except`. Two bindings "conflict" iff
 * their canonical strings are equal — canonicalization (sorted modifiers)
 * already makes that a safe check, no separate normalization needed. */
export function findConflict(
  binding: string,
  except: ShortcutAction,
  overrides: Partial<Record<ShortcutAction, string | null>>,
  isMacPlatform: boolean,
): ShortcutAction | null {
  for (const action of SHORTCUT_ACTIONS) {
    if (action === except) continue;
    if (resolveKeybinding(action, overrides, isMacPlatform) === binding) return action;
  }
  return null;
}

const MAC_MODIFIER_SYMBOLS: Record<ModifierName, string> = {
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
  meta: "⌘",
};

const OTHER_MODIFIER_LABELS: Record<ModifierName, string> = {
  ctrl: "Ctrl",
  alt: "Alt",
  shift: "Shift",
  meta: "Meta",
};

function formatKeyLabel(key: string): string {
  if (key === "tab") return "Tab";
  if (key.length === 1) return key.toUpperCase();
  return key.charAt(0).toUpperCase() + key.slice(1);
}

export function formatKeybindingLabel(binding: string, isMacPlatform: boolean): string {
  const { modifiers, key } = parseKeybinding(binding);
  const ordered = MODIFIER_NAMES.filter((m) => modifiers.has(m));

  if (key === ARROW_KEY) {
    const prefix = isMacPlatform
      ? ordered.map((m) => MAC_MODIFIER_SYMBOLS[m]).join("")
      : ordered.map((m) => OTHER_MODIFIER_LABELS[m]).join("+") + (ordered.length ? "+" : "");
    return isMacPlatform ? `${prefix}↑ / ${prefix}↓` : `${prefix}↑ / ${prefix}↓`;
  }

  if (isMacPlatform) {
    return ordered.map((m) => MAC_MODIFIER_SYMBOLS[m]).join("") + formatKeyLabel(key);
  }
  return [...ordered.map((m) => OTHER_MODIFIER_LABELS[m]), formatKeyLabel(key)].join("+");
}
