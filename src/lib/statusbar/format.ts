/**
 * How the status bar writes its numbers. Pure, so it runs under Node (see
 * format.test.ts); anything worded comes back as a key for the component to
 * translate.
 */
import type { MessageKey } from "$lib/i18n/translate";

export function formatBytes(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
}

/** Cents are enough for a session total, but a session that has only just
 * started would read "$0.00" as if it were free. */
export function formatUsd(value: number): string {
  return value > 0 && value < 0.01 ? "<$0.01" : `$${value.toFixed(2)}`;
}

/** A gauge can't run past its track; the number beside it still shows what
 * was reported (a spend limit can go over 100%). */
export function clampPercent(percent: number): number {
  return Math.min(100, Math.max(0, percent));
}

/** 15500 → "15.5k", 1200000 → "1.2M": a context window at a glance. */
export function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) return `${trim(tokens / 1_000_000)}M`;
  if (tokens >= 1000) return `${trim(tokens / 1000)}k`;
  return String(Math.round(tokens));
}

function trim(value: number): string {
  return value >= 100 ? String(Math.round(value)) : value.toFixed(1).replace(/\.0$/, "");
}

/** A running time as "45s", "12m", "1h 05m" or "2d 3h". */
export function formatDuration(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${String(minutes % 60).padStart(2, "0")}m`;
  return `${Math.floor(hours / 24)}d ${hours % 24}h`;
}

/** How long ago, worded: the key and what goes into it. */
export function agoLabel(ms: number): { key: MessageKey; params?: Record<string, string> } {
  const minutes = Math.floor(Math.max(0, ms) / 60_000);
  if (minutes < 1) return { key: "statusBar.ago.now" };
  if (minutes < 60) return { key: "statusBar.ago.minutes", params: { n: String(minutes) } };
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return { key: "statusBar.ago.hours", params: { n: String(hours) } };
  return { key: "statusBar.ago.days", params: { n: String(Math.floor(hours / 24)) } };
}

export type PathFormat = "full" | "home" | "repo" | "folder";

function trimSlash(path: string): string {
  return path.length > 1 ? path.replace(/[\\/]+$/, "") : path;
}

function separatorOf(path: string): string {
  return path.includes("\\") && !path.includes("/") ? "\\" : "/";
}

/** Whether `path` is `base` or inside it; case-blind on Windows paths. */
function within(path: string, base: string): boolean {
  const windows = /^[a-z]:/i.test(base) || base.includes("\\");
  const a = windows ? path.toLowerCase() : path;
  const b = windows ? base.toLowerCase() : base;
  return a === b || a.startsWith(b + "\\") || a.startsWith(b + "/");
}

/** A console's folder as the status bar shows it: whole, with the home
 * folder as `~`, from the repository's root (its name first), or just the
 * last folder. Falls back to the home form when there's no repository. */
export function formatPath(path: string, format: PathFormat, home: string | null, repoRoot: string | null): string {
  const clean = trimSlash(path);
  if (format === "full") return clean;
  if (format === "folder") return clean.split(/[\\/]/).filter(Boolean).pop() ?? clean;
  if (format === "repo" && repoRoot) {
    const root = trimSlash(repoRoot);
    if (within(clean, root)) {
      const name = root.split(/[\\/]/).filter(Boolean).pop() ?? root;
      return name + clean.slice(root.length);
    }
  }
  if (home) {
    const base = trimSlash(home);
    if (within(clean, base)) return "~" + (clean.length > base.length ? separatorOf(clean) + clean.slice(base.length + 1) : "");
  }
  return clean;
}

/** How long until a plan window starts over, from what the agent reported:
 * seconds since the epoch (both Claude and Codex), or milliseconds should
 * one ever send those. Null once it's past or when nothing was reported. */
export function resetsInMs(resetsAt: number | null, now: number): number | null {
  if (!resetsAt) return null;
  const at = resetsAt > 1e12 ? resetsAt : resetsAt * 1000;
  return at > now ? at - now : null;
}
