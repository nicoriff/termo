/**
 * Tab groups, as Chrome has them: a name and a color that gather tabs behind
 * a chip at the head of their tab strip. Kept by the backend
 * (src-tauri/src/tab_groups.rs) with a copy of their tabs, so a group whose
 * tabs were all closed — or that outlived Termo — can be opened again.
 *
 * Pure — no runes, no Tauri, no translations — so it can be unit-tested in
 * plain Node, same approach as `layout-snapshot.ts`.
 */

import type { LayoutSession } from "$lib/layout/layout-snapshot";
import type { SessionTab } from "$lib/layout/split-tree";

export const GROUP_COLORS = {
  grey: "#8b8f97",
  blue: "#5b8def",
  red: "#e5534b",
  yellow: "#e0b341",
  green: "#3fb26b",
  pink: "#e86ab8",
  purple: "#a073e8",
  cyan: "#3bb8c9",
  orange: "#ec8a3c",
} as const;

export type GroupColor = keyof typeof GROUP_COLORS;

export const GROUP_COLOR_IDS = Object.keys(GROUP_COLORS) as GroupColor[];

/** What a layout records of the group a tab was in: enough to bring the
 * group back if it was deleted since. */
export interface GroupDef {
  id: string;
  name: string;
  color: GroupColor;
}

export interface TabGroup extends GroupDef {
  /** Only its own tabs are folded behind the chip. */
  collapsed: boolean;
  /** When it was collapsed (ms), for the tab saver (see tab-saver.svelte.ts). */
  collapsedAt?: number;
  /** Its tabs as they last were, which is what reopening it opens. */
  tabs: LayoutSession[];
}

export function isGroupColor(value: unknown): value is GroupColor {
  return typeof value === "string" && value in GROUP_COLORS;
}

/** The first color no group uses yet, as Chrome picks them; once all are
 * taken, the one fewest groups use. */
export function nextColor(groups: readonly Pick<GroupDef, "color">[]): GroupColor {
  const uses = new Map<GroupColor, number>(GROUP_COLOR_IDS.map((color) => [color, 0]));
  for (const group of groups) uses.set(group.color, (uses.get(group.color) ?? 0) + 1);
  let best = GROUP_COLOR_IDS[0];
  for (const color of GROUP_COLOR_IDS) if (uses.get(color)! < uses.get(best)!) best = color;
  return best;
}

/**
 * Brings each group's tabs together where its first tab is, keeping every
 * other order as it was — a group never shows split in two along a strip.
 * Returns the same array when nothing had to move.
 */
export function gatherGroups<T extends Pick<SessionTab, "groupId">>(sessions: T[]): T[] {
  const result: T[] = [];
  const placed = new Set<string>();
  for (const session of sessions) {
    const group = session.groupId;
    if (!group) {
      result.push(session);
      continue;
    }
    if (placed.has(group)) continue;
    placed.add(group);
    result.push(...sessions.filter((other) => other.groupId === group));
  }
  return result.every((session, index) => session === sessions[index]) ? sessions : result;
}

/** The group a tab dropped at `index` (between `sessions[index - 1]` and
 * `sessions[index]`, the dropped tab not among them) lands inside: one whose
 * tabs are on both sides of it. Undefined at either end or between groups. */
export function joinedGroupAt(sessions: readonly Pick<SessionTab, "groupId">[], index: number): string | undefined {
  const before = sessions[index - 1]?.groupId;
  const after = sessions[index]?.groupId;
  return before && before === after ? before : undefined;
}

/** The states of a tab's agent worth an alert on its group's chip — the ones
 * Modo IA notifies about — most urgent first. */
const ALERTS = ["error", "awaiting", "finished"] as const;
export type GroupAlert = (typeof ALERTS)[number];

/** A tab's agent state, and whether it changed since the tab was last on
 * screen. */
export interface TabNews {
  status?: string | null;
  unseen?: boolean;
}

/** The most urgent alert among a group's tabs, or null when none has one.
 * Waiting on you alerts until it's answered; an error or a finish only
 * until the tab has been looked at. */
export function groupAlert(tabs: readonly TabNews[]): GroupAlert | null {
  const pending = (tab: TabNews) => tab.status === "awaiting" || !!tab.unseen;
  return ALERTS.find((alert) => tabs.some((tab) => tab.status === alert && pending(tab))) ?? null;
}

/** Whether a group's tabs are due to be put to rest: it has been collapsed
 * for at least `minutes`. An expanded group never is. */
export function restIsDue(
  group: Pick<TabGroup, "collapsed" | "collapsedAt">,
  now: number,
  minutes: number,
): boolean {
  return group.collapsed && group.collapsedAt !== undefined && now - group.collapsedAt >= minutes * 60_000;
}

type Placed = Pick<SessionTab, "id" | "groupId">;

/**
 * Inserts a tab ahead of `beforeSessionId` (or at the end). Placed on a strip
 * by hand, it joins the group it lands inside, and leaves its own when it
 * lands clear of it; dropped on a tile as a whole, it keeps its group. The
 * groups are gathered afterwards, so none ends up split in two.
 */
export function insertTab<T extends Placed>(
  sessions: T[],
  moved: T,
  beforeSessionId: string | undefined,
  onStrip: boolean,
  /** The group whose tab it was dropped on, which it joins wherever it
   * lands beside it — what the drag showed it would do. */
  joinGroupId?: string,
): T[] {
  const found = beforeSessionId ? sessions.findIndex((s) => s.id === beforeSessionId) : -1;
  const insertAt = found === -1 ? sessions.length : found;
  let tab = moved;
  if (joinGroupId) {
    tab = { ...moved, groupId: joinGroupId };
  } else if (onStrip) {
    const joined = joinedGroupAt(sessions, insertAt);
    const own = moved.groupId;
    const besideOwn = !!own && (sessions[insertAt - 1]?.groupId === own || sessions[insertAt]?.groupId === own);
    const ownHere = !!own && sessions.some((s) => s.groupId === own);
    if (joined) {
      tab = { ...moved, groupId: joined };
    } else if (ownHere && !besideOwn) {
      const { groupId: _group, ...rest } = moved;
      tab = rest as T;
    }
  }
  return gatherGroups([...sessions.slice(0, insertAt), tab, ...sessions.slice(insertAt)]);
}

/** Inserts a group's tabs, together, ahead of `beforeSessionId` — or past the
 * group that tab is in, so one group never lands inside another. */
export function insertGroup<T extends Placed>(sessions: T[], moved: T[], beforeSessionId: string | undefined): T[] {
  const found = beforeSessionId ? sessions.findIndex((s) => s.id === beforeSessionId) : -1;
  let insertAt = found === -1 ? sessions.length : found;
  const inside = joinedGroupAt(sessions, insertAt);
  if (inside) while (insertAt < sessions.length && sessions[insertAt].groupId === inside) insertAt++;
  return gatherGroups([...sessions.slice(0, insertAt), ...moved, ...sessions.slice(insertAt)]);
}

/** What a tab strip shows: each group's chip ahead of its tabs, and those
 * tabs unless the group is folded — the active one shows anyway, as you
 * can't be looking at a tab that isn't there. */
export type StripItem<T> =
  | { kind: "chip"; group: GroupDef & { collapsed: boolean }; count: number }
  | { kind: "tab"; session: T; group?: GroupDef };

export function stripItems<T extends Pick<SessionTab, "id" | "groupId">>(
  sessions: readonly T[],
  groups: readonly (GroupDef & { collapsed: boolean })[],
  activeId: string,
): StripItem<T>[] {
  const byId = new Map(groups.map((group) => [group.id, group]));
  const items: StripItem<T>[] = [];
  let previous: string | undefined;
  for (const session of sessions) {
    // A tab whose group is gone (deleted from another window) shows as
    // ungrouped rather than behind a chip with no name or color.
    const group = session.groupId ? byId.get(session.groupId) : undefined;
    if (group && group.id !== previous) {
      const count = sessions.filter((other) => other.groupId === group.id).length;
      items.push({ kind: "chip", group, count });
    }
    previous = group?.id;
    if (group?.collapsed && session.id !== activeId) continue;
    items.push(group ? { kind: "tab", session, group } : { kind: "tab", session });
  }
  return items;
}
