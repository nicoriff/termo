import { agentsStore } from "$lib/agents/agents-store.svelte";
import { resumeInTab } from "$lib/ai-mode/restore-session";
import { tabAgentSessions } from "$lib/ai-mode/tab-sessions.svelte";
import { deleteTabGroup, listTabGroups, onTabGroups, saveTabGroup } from "$lib/core/tab-groups-client";
import { explorerStore } from "$lib/explorer/explorer-store.svelte";
import { gitStore } from "$lib/git/git-store.svelte";
import { harnessStore } from "$lib/harness/harness-store.svelte";
import { hydrateSession, snapshotSession, type LayoutResume } from "$lib/layout/layout-snapshot";
import { collectSessions, collectTiles, type SessionLocation } from "$lib/layout/split-tree";
import { sessionFolder, workspaceStore } from "$lib/layout/workspace-store.svelte";
import { tabCwd } from "$lib/terminal/tab-cwd.svelte";
import { isGroupColor, nextColor, type GroupColor, type GroupDef, type TabGroup } from "./tab-groups";

/** How long after a change to the tree each group's tabs are recorded. */
const SAVE_DELAY_MS = 300;

export const TAB_GROUPS_WIDTH = { initial: 280, min: 220, max: 520 };
const WIDTH_KEY = "termo.tabGroups.width";

function loadWidth(): number {
  try {
    const saved = Number(localStorage.getItem(WIDTH_KEY));
    return saved ? clampWidth(saved) : TAB_GROUPS_WIDTH.initial;
  } catch {
    return TAB_GROUPS_WIDTH.initial;
  }
}

function clampWidth(width: number): number {
  return Math.round(Math.min(TAB_GROUPS_WIDTH.max, Math.max(TAB_GROUPS_WIDTH.min, width)));
}

/** A file edited by hand, or written by a later Termo, still reads. */
function checked(group: TabGroup): TabGroup {
  return {
    id: group.id,
    name: typeof group.name === "string" ? group.name : "",
    color: isGroupColor(group.color) ? group.color : "grey",
    collapsed: group.collapsed === true,
    tabs: Array.isArray(group.tabs) ? group.tabs : [],
  };
}

/**
 * The tab groups (see tab-groups.ts), as kept by the backend, plus the side
 * panel that manages them. Docked on the window's left edge like the
 * explorer, the Git panel, the Harness panel and the agents panel: one at a
 * time.
 *
 * What a group's tabs are is recorded as they change, so a group whose tabs
 * were all closed — or that outlived Termo — opens again as it last was.
 */
class TabGroupsStore {
  groups = $state<TabGroup[]>([]);
  open = $state(false);
  width = $state(loadWidth());
  /** The group whose editor (name, color, actions) is open, and where. */
  editing = $state<{ groupId: string; x: number; y: number } | null>(null);

  readonly byId = $derived(new Map(this.groups.map((group) => [group.id, group])));

  /** Every open tab of each group, in the order the strips show them. */
  readonly liveTabs = $derived.by(() => {
    const byGroup = new Map<string, SessionLocation[]>();
    const root = workspaceStore.root;
    if (!root) return byGroup;
    for (const loc of collectSessions(root)) {
      const group = loc.session.groupId;
      if (!group) continue;
      byGroup.set(group, [...(byGroup.get(group) ?? []), loc]);
    }
    return byGroup;
  });

  private timer: ReturnType<typeof setTimeout> | null = null;
  private lastSaved = new Map<string, string>();
  private saving = new Set<string>();
  /** The group folding or unfolding right now, for its tabs' animation. */
  folding: string | null = null;
  private foldingTimer: ReturnType<typeof setTimeout> | undefined;

  isOpen(groupId: string): boolean {
    return (this.liveTabs.get(groupId)?.length ?? 0) > 0;
  }

  async load(): Promise<void> {
    try {
      this.set(await listTabGroups());
    } catch {
      // Nothing saved yet, or the file is unreadable — no groups is fine.
    }
  }

  /** Takes the backend's list, except for groups this window is saving right
   * now: those are newer here than in what came back. */
  private set(groups: TabGroup[]): void {
    const incoming = groups.map(checked);
    const local = new Map(this.groups.map((group) => [group.id, group]));
    const merged = incoming.map((group) => (this.saving.has(group.id) ? (local.get(group.id) ?? group) : group));
    for (const id of this.saving) {
      if (!merged.some((group) => group.id === id) && local.has(id)) merged.push(local.get(id)!);
    }
    this.groups = merged;
    for (const group of incoming) {
      if (!this.saving.has(group.id)) this.lastSaved.set(group.id, JSON.stringify(group));
    }
  }

  /** Follows the backend's list (every window's changes) and records each
   * group's tabs as the tree changes. The main window calls it once;
   * returns what stops it. */
  start(): () => void {
    let stopped = false;
    let unlisten: (() => void) | null = null;
    void onTabGroups((groups) => this.set(groups)).then((stop) => {
      if (stopped) stop();
      else unlisten = stop;
    });
    const stopEffects = $effect.root(() => {
      $effect(() => {
        void workspaceStore.root;
        void tabCwd.byTab;
        this.schedule();
      });
    });
    return () => {
      stopped = true;
      unlisten?.();
      stopEffects();
      if (this.timer) clearTimeout(this.timer);
    };
  }

  private schedule(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.flush();
    }, SAVE_DELAY_MS);
  }

  /** Records, for each group with tabs open, what they are now. A group
   * with none keeps what it last had: that's what reopening it opens. Also
   * what quitting calls. */
  async flush(): Promise<void> {
    const defs = this.defs();
    for (const group of this.groups) {
      const live = this.liveTabs.get(group.id);
      if (!live?.length) continue;
      const tabs = live.map((loc) =>
        snapshotSession(loc.session, {
          agentByTab: tabAgentSessions.byTab,
          cwdByTab: tabCwd.byTab,
          withSessions: true,
          groups: defs,
        }),
      );
      // The group's own entry is already the group: its tabs needn't repeat it.
      for (const tab of tabs) delete tab.group;
      await this.persist({ ...group, tabs });
    }
  }

  /** The groups by id, as a layout records them with each tab. */
  defs(): Record<string, GroupDef> {
    return Object.fromEntries(this.groups.map((group) => [group.id, { id: group.id, name: group.name, color: group.color }]));
  }

  private async persist(group: TabGroup): Promise<void> {
    const text = JSON.stringify(group);
    this.groups = this.groups.map((other) => (other.id === group.id ? group : other));
    if (this.lastSaved.get(group.id) === text) return;
    this.lastSaved.set(group.id, text);
    this.saving.add(group.id);
    let saved: TabGroup[] | null = null;
    try {
      saved = await saveTabGroup(group);
    } catch {
      // Kept in memory; the next change tries again.
      this.lastSaved.delete(group.id);
    }
    // Only once nothing newer of it is on its way.
    if (!saved || this.lastSaved.get(group.id) === text) this.saving.delete(group.id);
    if (saved) this.set(saved);
  }

  private update(groupId: string, change: Partial<TabGroup>): void {
    const group = this.byId.get(groupId);
    if (group) void this.persist({ ...group, ...change });
  }

  /** A new group, with the next free color. Its tabs, if any, join it. */
  create(sessionIds: string[] = [], name = "", color?: GroupColor): string {
    const group: TabGroup = {
      id: crypto.randomUUID(),
      name,
      color: color ?? nextColor(this.groups),
      collapsed: false,
      tabs: [],
    };
    this.groups = [...this.groups, group];
    void this.persist(group);
    for (const id of sessionIds) workspaceStore.setSessionGroup(id, group.id);
    return group.id;
  }

  /** A group a layout's tabs were in, brought back if it's gone. One that
   * still exists stays as the user has it now. */
  ensure(defs: GroupDef[]): void {
    for (const def of defs) {
      if (this.byId.has(def.id)) continue;
      const group: TabGroup = {
        id: def.id,
        name: def.name ?? "",
        color: isGroupColor(def.color) ? def.color : nextColor(this.groups),
        collapsed: false,
        tabs: [],
      };
      this.groups = [...this.groups, group];
      void this.persist(group);
    }
  }

  rename(groupId: string, name: string): void {
    this.update(groupId, { name: name.trim() });
  }

  setColor(groupId: string, color: GroupColor): void {
    this.update(groupId, { color });
  }

  toggleCollapsed(groupId: string): void {
    const group = this.byId.get(groupId);
    if (!group) return;
    // Only this fold animates its tabs (see TileChrome's foldTab): tabs that
    // open, close or move come and go at once, as they always have.
    this.folding = groupId;
    clearTimeout(this.foldingTimer);
    this.foldingTimer = setTimeout(() => (this.folding = null), 250);
    this.update(groupId, { collapsed: !group.collapsed });
  }

  /** Puts a tab in a group, or with null takes it out of its own. */
  assign(sessionId: string, groupId: string | null): void {
    workspaceStore.setSessionGroup(sessionId, groupId);
  }

  /** A new console in the group, next to its tabs — where its first tab is,
   * or the focused tile when it has none open — like the one it last had. */
  newTabInGroup(groupId: string): void {
    const root = workspaceStore.root;
    if (!root) return;
    const live = this.liveTabs.get(groupId) ?? [];
    const tileId = live[0]?.tileId ?? workspaceStore.focusedTileId ?? collectTiles(root)[0]?.id;
    if (!tileId) return;
    const like = [...live].reverse().find((loc) => loc.session.shellPath)?.session;
    const folder = live.length ? sessionFolder(live[live.length - 1].session) : undefined;
    workspaceStore.addTab(tileId, like?.shellPath, like?.shellArgs, like?.title, folder, undefined, groupId);
    if (this.byId.get(groupId)?.collapsed) this.update(groupId, { collapsed: false });
  }

  /** Brings the tab in front, wherever it is. */
  focusTab(sessionId: string): void {
    const root = workspaceStore.root;
    if (!root) return;
    const loc = collectSessions(root).find((candidate) => candidate.session.id === sessionId);
    if (!loc) return;
    workspaceStore.setActiveSession(loc.tileId, sessionId);
    // Behind another tile's zoom it wouldn't be seen.
    if (workspaceStore.zoomedTileId && workspaceStore.zoomedTileId !== loc.tileId) workspaceStore.unzoom();
  }

  /** Opens the group's tabs as they last were, in the focused tile, with the
   * conversations they had open. `focusIndex` is the tab to show. A group
   * that never had tabs gets a new console. */
  reopen(groupId: string, focusIndex = 0): void {
    const group = this.byId.get(groupId);
    const root = workspaceStore.root;
    if (!group || !root || this.isOpen(groupId)) return;
    if (group.tabs.length === 0) {
      this.newTabInGroup(groupId);
      return;
    }
    const tileId = workspaceStore.focusedTileId ?? collectTiles(root)[0]?.id;
    if (!tileId) return;
    const resumes: LayoutResume[] = [];
    const def = { id: group.id, name: group.name, color: group.color };
    // Plain copies: the group's tabs are this store's state, and a proxy of
    // it inside the workspace tree would tie the two together.
    const sessions = $state
      .snapshot(group.tabs)
      .map((tab) => hydrateSession({ ...tab, group: def }, resumes));
    workspaceStore.addSessions(tileId, sessions, (sessions[focusIndex] ?? sessions[0]).id);
    if (group.collapsed) this.update(groupId, { collapsed: false });
    for (const resume of resumes) void resumeInTab(resume.tabId, resume.tool, resume.agentSessionId);
  }

  /** Closes the group's tabs; the group stays, to be opened again. */
  async closeGroup(groupId: string): Promise<void> {
    // What they are right now, not as of the last change.
    await this.flush();
    workspaceStore.closeGroupTabs(groupId);
  }

  /** Its tabs stay open, in no group; the group itself goes. */
  ungroup(groupId: string): void {
    workspaceStore.clearGroup(groupId);
    void this.remove(groupId);
  }

  /** Deletes the group. Its open tabs stay, in no group. */
  async remove(groupId: string): Promise<void> {
    workspaceStore.clearGroup(groupId);
    this.groups = this.groups.filter((group) => group.id !== groupId);
    this.lastSaved.delete(groupId);
    if (this.editing?.groupId === groupId) this.editing = null;
    try {
      this.set(await deleteTabGroup(groupId));
    } catch {
      // Gone from this window; the file catches up on the next change.
    }
  }

  openEditor(groupId: string, x: number, y: number): void {
    this.editing = { groupId, x, y };
  }

  closeEditor(): void {
    this.editing = null;
  }

  toggle(): void {
    this.setOpen(!this.open);
  }

  setOpen(open: boolean): void {
    // One side panel at a time: they're all docked on the same edge.
    if (open) {
      agentsStore.setOpen(false);
      explorerStore.open = false;
      gitStore.open = false;
      harnessStore.setOpen(false);
    }
    this.open = open;
  }

  setWidth(width: number, persist = false): void {
    this.width = clampWidth(width);
    if (!persist) return;
    try {
      localStorage.setItem(WIDTH_KEY, String(this.width));
    } catch {
      // Private mode or storage off: it just won't be remembered.
    }
  }
}

export const tabGroupsStore = new TabGroupsStore();
