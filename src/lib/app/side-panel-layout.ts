/** The sections the side panel can hold, each the panel of one store. */
export const SIDE_PANEL_IDS = ["explorer", "git", "harness", "tabGroups", "kanban", "agents"] as const;
export type SidePanelId = (typeof SIDE_PANEL_IDS)[number];

/** Sections stacked as an accordion: one at most shows its content
 * (`expanded`; null with all folded), the rest only their header. `size` is
 * the share of the column's height. */
export interface PanelGroup {
  panels: SidePanelId[];
  expanded: SidePanelId | null;
  size: number;
}

/** The side panel's column: groups stacked top to bottom, all visible at
 * once. `active` is where a click on the activity bar adds a section. */
export interface SideLayout {
  groups: PanelGroup[];
  active: number;
}

export type DropSide = "above" | "below";

export const EMPTY_LAYOUT: SideLayout = { groups: [], active: 0 };

export function groupOf(layout: SideLayout, id: SidePanelId): number {
  return layout.groups.findIndex((group) => group.panels.includes(id));
}

export function isExpanded(layout: SideLayout, id: SidePanelId): boolean {
  return layout.groups.some((group) => group.expanded === id);
}

/** Sizes that add up to 1, each in the same proportion as before. */
function normalizeSizes(groups: PanelGroup[]): PanelGroup[] {
  const total = groups.reduce((sum, group) => sum + (group.size > 0 ? group.size : 0), 0);
  if (total <= 0) return groups.map((group) => ({ ...group, size: 1 / groups.length }));
  return groups.map((group) => ({ ...group, size: (group.size > 0 ? group.size : 0) / total }));
}

function clampActive(layout: SideLayout): SideLayout {
  const last = Math.max(0, layout.groups.length - 1);
  return { ...layout, active: Math.min(Math.max(0, layout.active), last) };
}

/** Takes a section out of the column. A group left empty goes, and the
 * others share its height. */
export function closePanel(layout: SideLayout, id: SidePanelId): SideLayout {
  const index = groupOf(layout, id);
  if (index < 0) return layout;
  const group = layout.groups[index];
  const at = group.panels.indexOf(id);
  const panels = group.panels.filter((panel) => panel !== id);
  if (panels.length === 0) {
    const groups = normalizeSizes(layout.groups.filter((_, i) => i !== index));
    const active = layout.active > index ? layout.active - 1 : layout.active;
    return clampActive({ groups, active });
  }
  // The one that was showing went: its neighbor shows instead.
  const expanded = group.expanded === id ? panels[Math.min(at, panels.length - 1)] : group.expanded;
  const groups = layout.groups.map((g, i) => (i === index ? { ...g, panels, expanded } : g));
  return { ...layout, groups };
}

/** Shows a section: unfolds it where it is, or adds it to the accordion of
 * the topmost group. With nothing open it's the column's only group. */
export function openInAccordion(layout: SideLayout, id: SidePanelId): SideLayout {
  const index = groupOf(layout, id);
  if (index >= 0) return expand(layout, index, id);
  if (layout.groups.length === 0) return { groups: [{ panels: [id], expanded: id, size: 1 }], active: 0 };
  const groups = layout.groups.map((group, i) =>
    i === 0 ? { ...group, panels: [...group.panels, id], expanded: id } : group,
  );
  return { groups, active: 0 };
}

/** Folds a group's unfolded section: only its headers show. */
export function collapse(layout: SideLayout, groupIndex: number): SideLayout {
  const group = layout.groups[groupIndex];
  if (!group || group.expanded === null) return layout;
  const groups = layout.groups.map((g, i) => (i === groupIndex ? { ...g, expanded: null } : g));
  return { groups, active: groupIndex };
}

/** A header clicked: its section unfolds, or folds when it was unfolded. */
export function toggleExpanded(layout: SideLayout, groupIndex: number, id: SidePanelId): SideLayout {
  return layout.groups[groupIndex]?.expanded === id ? collapse(layout, groupIndex) : expand(layout, groupIndex, id);
}

/** Unfolds one section of a group; the others fold to their header. */
export function expand(layout: SideLayout, groupIndex: number, id: SidePanelId): SideLayout {
  const group = layout.groups[groupIndex];
  if (!group || !group.panels.includes(id)) return layout;
  if (group.expanded === id && layout.active === groupIndex) return layout;
  const groups = layout.groups.map((g, i) => (i === groupIndex ? { ...g, expanded: id } : g));
  return { groups, active: groupIndex };
}

/** Puts a section in a group of its own, above or below `groupIndex`, taking
 * half that group's height. From the same group, only if something stays. */
export function openAsGroup(layout: SideLayout, id: SidePanelId, groupIndex: number, side: DropSide): SideLayout {
  const target = layout.groups[groupIndex];
  if (!target) return openInAccordion(layout, id);
  if (target.panels.length === 1 && target.panels[0] === id) return layout;
  const without = closePanel(layout, id);
  // Where the target is once the section left (its group may have gone).
  let at = without.groups.findIndex((group) => group.panels.includes(target.panels.find((p) => p !== id)!));
  if (at < 0) at = Math.min(groupIndex, without.groups.length - 1);
  const half = without.groups[at].size / 2;
  const groups = without.groups.map((group, i) => (i === at ? { ...group, size: half } : group));
  const insertAt = side === "above" ? at : at + 1;
  groups.splice(insertAt, 0, { panels: [id], expanded: id, size: half });
  return { groups: normalizeSizes(groups), active: insertAt };
}

/** Moves a section into a group's accordion at `position` (among the
 * headers), and unfolds it there. */
export function moveIntoGroup(layout: SideLayout, id: SidePanelId, groupIndex: number, position: number): SideLayout {
  const target = layout.groups[groupIndex];
  if (!target) return openInAccordion(layout, id);
  const from = groupOf(layout, id);
  if (from === groupIndex) {
    const panels = target.panels.filter((panel) => panel !== id);
    const old = target.panels.indexOf(id);
    const at = Math.min(Math.max(0, old < position ? position - 1 : position), panels.length);
    panels.splice(at, 0, id);
    const groups = layout.groups.map((g, i) => (i === groupIndex ? { ...g, panels, expanded: id } : g));
    return { groups, active: groupIndex };
  }
  const anchor = target.panels[0];
  const without = closePanel(layout, id);
  const at = without.groups.findIndex((group) => group.panels.includes(anchor));
  const groups = without.groups.map((group, i) => {
    if (i !== at) return group;
    const panels = [...group.panels];
    panels.splice(Math.min(Math.max(0, position), panels.length), 0, id);
    return { ...group, panels, expanded: id };
  });
  return { groups, active: at };
}

/** Moves the border between group `index` and the next one by `delta` (a
 * share of the column's height), each keeping at least `min`. */
export function resizeGroups(layout: SideLayout, index: number, delta: number, min: number): SideLayout {
  const a = layout.groups[index];
  const b = layout.groups[index + 1];
  if (!a || !b) return layout;
  const pair = a.size + b.size;
  const floor = Math.min(min, pair / 2);
  const size = Math.min(Math.max(a.size + delta, floor), pair - floor);
  const groups = layout.groups.map((g, i) => (i === index ? { ...g, size } : i === index + 1 ? { ...g, size: pair - size } : g));
  return { ...layout, groups };
}

/** Where a drop over a group goes: a strip at its top or bottom opens a new
 * group there, the rest joins its accordion. `fy` is 0..1 down the group. */
export function dropSideAt(fy: number, edge = 0.25): DropSide | null {
  if (fy < edge) return "above";
  if (fy > 1 - edge) return "below";
  return null;
}

/** A layout read back from storage, made whole: unknown or repeated sections
 * dropped, empty groups gone, sizes adding up to 1. */
export function sanitizeLayout(value: unknown, allowed: readonly SidePanelId[] = SIDE_PANEL_IDS): SideLayout {
  if (!value || typeof value !== "object") return EMPTY_LAYOUT;
  const raw = value as { groups?: unknown; active?: unknown };
  if (!Array.isArray(raw.groups)) return EMPTY_LAYOUT;
  const seen = new Set<SidePanelId>();
  const groups: PanelGroup[] = [];
  for (const item of raw.groups) {
    if (!item || typeof item !== "object") continue;
    const g = item as { panels?: unknown; expanded?: unknown; size?: unknown };
    if (!Array.isArray(g.panels)) continue;
    const panels: SidePanelId[] = [];
    for (const p of g.panels) {
      if (typeof p !== "string" || !allowed.includes(p as SidePanelId) || seen.has(p as SidePanelId)) continue;
      seen.add(p as SidePanelId);
      panels.push(p as SidePanelId);
    }
    if (panels.length === 0) continue;
    const expanded =
      g.expanded === null ? null : panels.includes(g.expanded as SidePanelId) ? (g.expanded as SidePanelId) : panels[0];
    const size = typeof g.size === "number" && Number.isFinite(g.size) && g.size > 0 ? g.size : 1;
    groups.push({ panels, expanded, size });
  }
  const active = typeof raw.active === "number" ? raw.active : 0;
  return clampActive({ groups: normalizeSizes(groups), active });
}
