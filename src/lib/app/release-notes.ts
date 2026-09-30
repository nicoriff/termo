import type { Locale } from "$lib/i18n/i18n.svelte";

/** One version in changelog.json: what it brings, in both languages. */
export interface ChangelogEntry {
  version: string;
  /** YYYY-MM-DD. */
  date: string;
  es: string[];
  en: string[];
}

/**
 * `a` against `b` as versions (`0.10.0` is after `0.9.2`): negative, zero or
 * positive. Anything after a `-` or `+` is ignored, and a missing or
 * non-numeric part counts as 0 — a malformed version sorts first rather than
 * throwing.
 */
export function compareVersions(a: string, b: string): number {
  const parts = (version: string) =>
    version
      .trim()
      .replace(/^v/, "")
      .split(/[-+]/)[0]
      .split(".")
      .map((part) => Number.parseInt(part, 10) || 0);
  const [left, right] = [parts(a), parts(b)];
  for (let i = 0; i < Math.max(left.length, right.length); i++) {
    const diff = (left[i] ?? 0) - (right[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/** The entries after `from` up to and including `to`, newest first: what an
 * update from `from` to `to` brought. */
export function entriesBetween(changelog: ChangelogEntry[], from: string, to: string): ChangelogEntry[] {
  return changelog
    .filter((entry) => compareVersions(entry.version, from) > 0 && compareVersions(entry.version, to) <= 0)
    .sort((a, b) => compareVersions(b.version, a.version));
}

const MARKER = /<!--\s*lang:(\w+)\s*-->/g;

/**
 * A release's notes in `locale`. The body carries both languages, each after
 * a `<!-- lang:xx -->` marker (see scripts/release-notes.mjs), which GitHub's
 * page doesn't show. Falls back to the other language, then to the whole
 * body when it has no markers at all (a release written by hand).
 */
export function notesFor(body: string | undefined, locale: Locale): string {
  const text = body?.trim() ?? "";
  const sections = new Map<string, string>();
  const markers = [...text.matchAll(MARKER)];
  markers.forEach((marker, i) => {
    const start = (marker.index ?? 0) + marker[0].length;
    const end = markers[i + 1]?.index ?? text.length;
    sections.set(marker[1], text.slice(start, end).trim());
  });
  if (sections.size === 0) return text;
  const other: Locale = locale === "es" ? "en" : "es";
  return sections.get(locale) || sections.get(other) || "";
}

/** The Markdown list an entry makes, as the release body carries it. */
export function asList(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}
