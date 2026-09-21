import type { MessageKey } from "$lib/i18n/translate";

/** What to call a rate-limit window in the UI, given its length in minutes.
 * The agents don't report the same windows — Claude has a 5-hour and a weekly
 * one, Codex a primary and an optional secondary whose length depends on the
 * plan — so they're named from their length rather than from a fixed list.
 * The two Claude windows keep their familiar short names. */
export interface PlanWindowLabel {
  key: MessageKey;
  params?: Record<string, string | number>;
}

const HOUR = 60;
const DAY = 24 * HOUR;

export function planWindowLabel(windowMinutes: number | null): PlanWindowLabel {
  if (windowMinutes === null || windowMinutes <= 0) return { key: "statusBar.planWindow" };
  if (windowMinutes === 5 * HOUR) return { key: "statusBar.fiveHour" };
  if (windowMinutes === 7 * DAY) return { key: "statusBar.sevenDay" };
  if (windowMinutes < DAY) return { key: "statusBar.windowHours", params: { hours: round(windowMinutes / HOUR) } };
  return { key: "statusBar.windowDays", params: { days: round(windowMinutes / DAY) } };
}

/** Whole numbers where the window divides evenly (30 days, 5 hours), one
 * decimal otherwise, so an odd window doesn't read as a rounded lie. */
function round(value: number): number {
  return Number.isInteger(value) ? value : Math.round(value * 10) / 10;
}
