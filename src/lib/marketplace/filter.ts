/**
 * Searching and grouping the marketplace catalog. Pure — no runes, no Tauri,
 * no i18n store — so it can be unit-tested in plain Node, same approach as
 * `commands/commands.ts` and `core/shortcuts.ts`.
 *
 * The catalog is a few hundred entries and already in memory, so filtering
 * happens here rather than server-side; what matters is the ranking, since a
 * word like "git" appears in dozens of descriptions and in exactly one name.
 */

import { normalize, scoreMatch } from "$lib/core/fuzzy-match";
import type { AgentTarget, CatalogItem, ComponentClass } from "$lib/core/marketplace-client";

export interface Facets {
  /** Empty means every class. */
  classes?: ComponentClass[];
  /** Only items installable into this agent. */
  target?: AgentTarget | null;
  installedOnly?: boolean;
  withSkillsOnly?: boolean;
}

/** Display order of the groups, stable so the list doesn't reshuffle while
 * typing. MCP servers first: they're what the source picker and the search
 * go out for, and what every agent takes — under 300 plugins that only
 * Claude Code has, they were never found. */
export const CLASS_ORDER: ComponentClass[] = ["mcp", "plugin", "skill"];

/** A name match is worth much more than a description match: the description
 * is a paragraph of marketing copy and would otherwise drown the name out. */
const NAME_WEIGHT = 0;
const TAG_WEIGHT = 10;
const DESCRIPTION_WEIGHT = 20;

/** Lower is better; null means no match at all. */
export function scoreItem(item: CatalogItem, query: string): number | null {
  if (normalize(query).trim() === "") return NAME_WEIGHT;

  const byName = scoreMatch(item.name, query) ?? scoreMatch(item.displayName, query);
  if (byName !== null) return NAME_WEIGHT + byName;

  const byTag = item.tags.map((tag) => scoreMatch(tag, query)).find((score) => score !== null);
  if (byTag !== undefined && byTag !== null) return TAG_WEIGHT + byTag;

  // Only a literal run of the query counts in a description — a subsequence
  // match there hits nearly every long text and means nothing.
  const description = normalize(item.description);
  const needle = normalize(query).trim();
  if (description.includes(needle)) return DESCRIPTION_WEIGHT;

  return null;
}

export function matchesFacets(item: CatalogItem, facets: Facets): boolean {
  if (facets.classes?.length && !facets.classes.includes(item.class)) return false;
  if (facets.target && !item.targets.includes(facets.target)) return false;
  if (facets.installedOnly && item.installedIn.length === 0) return false;
  if (facets.withSkillsOnly && item.declaresSkills.length === 0) return false;
  return true;
}

/** How a result list is ordered. "installs" puts the most downloaded first —
 * the only popularity signal these catalogs publish — and "relevance" puts
 * the closest match to what was typed first. Both break ties with the other
 * one, and finally by name, so the order never depends on whatever sequence
 * the catalog happened to arrive in. */
export type SortMode = "installs" | "relevance";

/** Matching items, ordered by `sort`. */
export function matchItems(
  items: CatalogItem[],
  query: string,
  facets: Facets = {},
  sort: SortMode = "installs",
): CatalogItem[] {
  const byInstalls = (a: CatalogItem, b: CatalogItem) => (b.installCount ?? 0) - (a.installCount ?? 0);

  return items
    .filter((item) => matchesFacets(item, facets))
    .map((item) => ({ item, score: scoreItem(item, query) }))
    .filter((entry): entry is { item: CatalogItem; score: number } => entry.score !== null)
    .sort((a, b) => {
      const relevance = a.score - b.score;
      const installs = byInstalls(a.item, b.item);
      const primary = sort === "installs" ? installs || relevance : relevance || installs;
      return primary || a.item.name.localeCompare(b.item.name);
    })
    .map((entry) => entry.item);
}

export interface ItemGroup {
  class: ComponentClass;
  items: CatalogItem[];
}

/** Groups by component class, keeping each group's incoming (ranked) order
 * and dropping classes with nothing in them. */
export function groupItems(items: CatalogItem[]): ItemGroup[] {
  return CLASS_ORDER.map((componentClass) => ({
    class: componentClass,
    items: items.filter((item) => item.class === componentClass),
  })).filter((group) => group.items.length > 0);
}
