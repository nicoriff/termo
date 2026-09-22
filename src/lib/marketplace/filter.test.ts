import { describe, expect, it } from "vitest";
import type { CatalogItem } from "$lib/core/marketplace-client";
import { groupItems, matchItems, scoreItem } from "./filter";

function item(overrides: Partial<CatalogItem> & { name: string }): CatalogItem {
  return {
    id: `plugin:${overrides.name}`,
    class: "plugin",
    displayName: overrides.name,
    description: "",
    sourceId: "claude-marketplace:official",
    sourceLabel: "official",
    category: null,
    author: null,
    homepage: null,
    version: null,
    tags: [],
    installCount: null,
    iconUrl: null,
    rating: null,
    declaresSkills: [],
    targets: ["claude"],
    installedIn: [],
    detail: { kind: "plugin", plugin_id: `${overrides.name}@official`, marketplace: "official", source: null, strict: null },
    ...overrides,
  };
}

describe("scoreItem", () => {
  it("ranks a name match above a description match", () => {
    const byName = item({ name: "github-tools" });
    const byDescription = item({ name: "unrelated", description: "works with github repos" });

    expect(scoreItem(byName, "github")!).toBeLessThan(scoreItem(byDescription, "github")!);
  });

  it("ignores case and accents", () => {
    expect(scoreItem(item({ name: "Migración" }), "migracion")).not.toBeNull();
  });

  it("does not match a description by loose subsequence", () => {
    // "xyz" appears as scattered letters in this sentence; that must not count.
    const loose = item({ name: "nothing", description: "extremely lazy zebras" });
    expect(scoreItem(loose, "xyz")).toBeNull();
  });
});

describe("matchItems", () => {
  const items = [
    item({ name: "beta", installCount: 10 }),
    item({ name: "alpha", installCount: 500 }),
    item({ name: "gamma", class: "mcp", targets: ["claude", "codex"] }),
    item({ name: "installed-one", installedIn: ["claude"] }),
    item({ name: "with-skills", declaresSkills: ["./skills/a"] }),
  ];

  it("returns everything for an empty query, most installed first", () => {
    const result = matchItems(items, "");
    expect(result).toHaveLength(items.length);
    expect(result[0].name).toBe("alpha");
  });

  it("filters by class", () => {
    expect(matchItems(items, "", { classes: ["mcp"] }).map((i) => i.name)).toEqual(["gamma"]);
  });

  it("filters by target, and plugins never match codex", () => {
    expect(matchItems(items, "", { target: "codex" }).map((i) => i.name)).toEqual(["gamma"]);
  });

  it("composes facets", () => {
    expect(matchItems(items, "", { installedOnly: true, withSkillsOnly: true })).toEqual([]);
    expect(matchItems(items, "", { installedOnly: true }).map((i) => i.name)).toEqual(["installed-one"]);
    expect(matchItems(items, "", { withSkillsOnly: true }).map((i) => i.name)).toEqual(["with-skills"]);
  });
});

describe("sorting", () => {
  const items = [
    item({ name: "exact-match", installCount: 5 }),
    item({ name: "popular", description: "mentions exact-match in passing", installCount: 9000 }),
  ];

  it("puts the most downloaded first by default", () => {
    expect(matchItems(items, "exact-match").map((i) => i.name)).toEqual(["popular", "exact-match"]);
  });

  it("puts the closest match first when asked for relevance", () => {
    expect(matchItems(items, "exact-match", {}, "relevance").map((i) => i.name)).toEqual([
      "exact-match",
      "popular",
    ]);
  });

  it("breaks ties by name so the order never wobbles", () => {
    const tied = [item({ name: "b" }), item({ name: "a" })];
    expect(matchItems(tied, "").map((i) => i.name)).toEqual(["a", "b"]);
  });
});

describe("groupItems", () => {
  it("keeps class order and drops empty groups", () => {
    const groups = groupItems([item({ name: "a", class: "mcp" }), item({ name: "b" })]);
    expect(groups.map((group) => group.class)).toEqual(["plugin", "mcp"]);
    expect(groups[0].items.map((i) => i.name)).toEqual(["b"]);
  });
});
