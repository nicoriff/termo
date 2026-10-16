import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { MessageKey } from "$lib/i18n/translate";

/** Mirrors src-tauri/src/keep_awake.rs. */
export interface KeepAwakeState {
  active: boolean;
  /** The screen stays on too. */
  display: boolean;
  /** When it turns itself off (Unix ms); null: when turned off by hand. */
  until: number | null;
}

export function keepAwakeState(): Promise<KeepAwakeState> {
  return invoke("keep_awake_state");
}

/** On for `minutes` (null: until turned off), or off. */
export function setKeepAwake(active: boolean, display: boolean, minutes: number | null): Promise<KeepAwakeState> {
  return invoke("keep_awake_set", { active, display, minutes });
}

export function onKeepAwake(handler: (state: KeepAwakeState) => void): Promise<UnlistenFn> {
  return listen<KeepAwakeState>("keep-awake", (event) => handler(event.payload));
}

/** The choices of how long, in minutes; null is "until I turn it off". */
export const DURATIONS: (number | null)[] = [null, 30, 60, 120, 240, 480];

/** "30 min", "2 h", "until I turn it off". */
export function durationLabel(minutes: number | null): { key: MessageKey; params?: Record<string, string> } {
  if (minutes === null) return { key: "titlebar.keepAwake.forever" };
  if (minutes < 60) return { key: "titlebar.keepAwake.minutes", params: { n: String(minutes) } };
  if (minutes === 60) return { key: "titlebar.keepAwake.hour" };
  return { key: "titlebar.keepAwake.hours", params: { n: String(minutes / 60) } };
}

/** What's left of a timed session, rounded up to the minute: "1 h 20 min",
 * "45 min". Never less than a minute while it's still on. */
export function remaining(until: number, now: number): string {
  const minutes = Math.max(1, Math.ceil((until - now) / 60_000));
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) return `${rest} min`;
  return rest === 0 ? `${hours} h` : `${hours} h ${rest} min`;
}

/** Whole minutes left, to carry a session over when a setting changes. */
export function minutesLeft(until: number | null, now: number): number | null {
  return until === null ? null : Math.max(1, Math.ceil((until - now) / 60_000));
}

/** The last choices (how long, and the screen), remembered per computer. */
export interface KeepAwakePrefs {
  minutes: number | null;
  display: boolean;
}

const PREFS_KEY = "termo.keepAwake";

export function loadPrefs(): KeepAwakePrefs {
  try {
    const saved = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "null");
    if (saved && typeof saved === "object") {
      const minutes = DURATIONS.includes(saved.minutes) ? saved.minutes : null;
      return { minutes, display: saved.display === true };
    }
  } catch {
    // Falls back to the defaults below.
  }
  return { minutes: null, display: false };
}

export function savePrefs(prefs: KeepAwakePrefs): void {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // Only a convenience.
  }
}
