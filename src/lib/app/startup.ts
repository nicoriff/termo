import { tick } from "svelte";
import { lazyLoadsSettled } from "$lib/core/lazy";
import { windowReady } from "$lib/core/window-client";

/** How long showing the window waits on the UI font before giving up on it:
 * it's bundled, so normally it's there long before. */
const FONT_WAIT_MS = 300;

/** This page's startup timings, in ms since it started loading. */
const marks: [string, number][] = [];

/** One more point on the way to the window showing, for startup.log. */
export function markStartup(name: string): void {
  marks.push([name, Math.round(performance.now())]);
}

/** The tabs are in place: once they're laid out — a file tab's editor
 * loaded too (see core/lazy.ts) —, the window, built hidden, shows (see
 * startup.rs). Not on an animation frame: a window never shown doesn't
 * paint, so it would never come. */
export async function showWhenLaidOut(): Promise<void> {
  await tick();
  await lazyLoadsSettled();
  await tick();
  await Promise.race([document.fonts.ready, new Promise((resolve) => setTimeout(resolve, FONT_WAIT_MS))]);
  markStartup("ready");
  await windowReady(marks).catch(() => {});
}
