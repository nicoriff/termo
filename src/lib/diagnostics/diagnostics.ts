/**
 * The numbers behind the diagnostics panel: keeping a short history of each
 * reading, drawing it as a line, and writing values for people. Pure, so it
 * runs under Node (see diagnostics.test.ts).
 */

/** Readings kept per chart: one a second, so the last minute. */
export const HISTORY_LENGTH = 60;

/** Appends `value`, dropping the oldest once there are `max`. */
export function pushSample(series: readonly number[], value: number, max = HISTORY_LENGTH): number[] {
  const next = [...series, value];
  return next.length > max ? next.slice(next.length - max) : next;
}

/** A tree's CPU as a share of the whole machine: the raw reading counts a
 * full core as 100, so an 8-core machine can read 800. */
export function cpuOfMachine(cpu: number, cores: number): number {
  return Math.min(100, Math.max(0, cpu / Math.max(1, cores)));
}

/**
 * The `points` of an SVG polyline for `values` across `width` × `height`,
 * oldest on the left, newest on the right. Fixed to `ceiling` when given (a
 * percentage always spans 0–100); otherwise to the largest reading seen, so a
 * quiet chart still shows its shape. A chart of fewer than `length` readings
 * starts part-way across, so a new line grows in from the right.
 */
export function sparklinePoints(
  values: readonly number[],
  width: number,
  height: number,
  ceiling?: number,
  length = HISTORY_LENGTH,
): string {
  if (values.length === 0) return "";
  const top = Math.max(ceiling ?? Math.max(...values), 1e-9);
  const step = length > 1 ? width / (length - 1) : 0;
  const offset = (length - values.length) * step;
  return values
    .map((value, index) => {
      const x = offset + index * step;
      const y = height - (Math.min(Math.max(value, 0), top) / top) * height;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
}

/** The same line closed along the bottom, to fill the area under it. */
export function sparklineArea(points: string, height: number): string {
  if (!points) return "";
  const pairs = points.split(" ");
  const first = pairs[0].split(",")[0];
  const last = pairs[pairs.length - 1].split(",")[0];
  return `${first},${height} ${points} ${last},${height}`;
}

export function formatBytes(bytes: number): string {
  const mb = bytes / 1024 / 1024;
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return mb >= 10 ? `${Math.round(mb)} MB` : `${mb.toFixed(1)} MB`;
}

export function formatPercent(value: number): string {
  return `${value >= 10 ? Math.round(value) : value.toFixed(1)}%`;
}

/** "45 s", "3 min", "2 h 5 min": the same in both languages. */
export function formatUptime(secs: number): string {
  if (secs < 60) return `${Math.floor(secs)} s`;
  const minutes = Math.floor(secs / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours} h` : `${hours} h ${rest} min`;
}

export type SortKey = "tabs" | "cpu-desc" | "cpu-asc" | "memory-desc" | "memory-asc";

export const SORT_KEYS: readonly SortKey[] = ["tabs", "cpu-desc", "cpu-asc", "memory-desc", "memory-asc"];

/** The rows ordered by `key` without touching `rows`. "tabs" keeps them as
 * given (the tabs' own order); the others order by what's used, the other
 * figure breaking ties, and a row that is still tied keeps its place. */
export function sortRows<T extends { cpu: number; memory: number }>(rows: readonly T[], key: SortKey): T[] {
  if (key === "tabs") return [...rows];
  const [field, direction] = key.split("-") as ["cpu" | "memory", "asc" | "desc"];
  const other = field === "cpu" ? "memory" : "cpu";
  const sign = direction === "asc" ? 1 : -1;
  return [...rows].sort((a, b) => sign * (a[field] - b[field]) || sign * (a[other] - b[other]));
}

const HEAVY_CPU = 80;
const HEAVY_MEMORY = 1024 * 1024 * 1024;

/** Worth pointing out: most of a core, or a gigabyte. */
export function isHeavy(usage: { cpu: number; memory: number }): boolean {
  return usage.cpu >= HEAVY_CPU || usage.memory >= HEAVY_MEMORY;
}
