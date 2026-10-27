/**
 * Turns the live split tree into something worth storing, and back.
 *
 * Pure — no runes, no Tauri — so it can be unit-tested in plain Node, same
 * approach as `commands.ts` and `core/shortcuts.ts`.
 *
 * Ids are deliberately NOT stored: a saved id would come back while the
 * session that owned it is still alive, putting the same id in two tiles and
 * confusing everything keyed by it (the PTY registry, Modo IA's per-session
 * state, tab titles). Restoring mints fresh ones instead.
 */

import {
  makeAgentRunSession,
  makeFileSession,
  makeOrchestrationSession,
  makeProjectSession,
  makeDiffSession,
  makeGitGraphSession,
  makeSession,
  makeTile,
  type DiffTarget,
  type SessionTab,
  type SplitDirection,
  type SplitNode,
} from "./split-tree";
import type { ToolId } from "$lib/ai-mode/pattern-table";
import type { GroupDef } from "$lib/tab-groups/tab-groups";

export interface LayoutSession {
  title: string;
  shellPath?: string;
  shellArgs?: string[];
  cwd?: string;
  /** A file tab: reopened in the editor rather than as a console. */
  filePath?: string;
  /** An agent's runs tab. */
  agentRun?: { agentId: string; runId: string };
  /** A project's tab. */
  projectView?: { projectId: string };
  /** An orchestration's tab, or its console's. */
  orchestrationView?: { orchestrationId: string; console?: boolean };
  /** A file's changes, or a repository's history. */
  diff?: DiffTarget;
  gitGraph?: { root: string };
  /** The agent conversation this console had open, when the layout was saved
   * with sessions. Restoring reopens it with the agent's own resume command. */
  agentTool?: ToolId;
  agentSessionId?: string;
  /** The console's id, in what a window records for consoles that outlive
   * it (see session-keeper.svelte.ts): if the sessions daemon still runs
   * it, restoring shows it again instead of starting a new one. */
  consoleId?: string;
  /** With `consoleId`: it's another computer's console (see SessionTab.attachHost). */
  remoteHost?: { ip: string; name: string };
  /** The tab group it was in, whole: restoring brings the group back with it
   * even if it was deleted since. */
  group?: GroupDef;
}

export type LayoutNode =
  | { kind: "leaf"; sessions: LayoutSession[]; activeIndex: number }
  | { kind: "split"; direction: SplitDirection; ratio: number; children: [LayoutNode, LayoutNode] };

/** Which conversation each tab has open, keyed by tab id (tabAgentSessions). */
export type AgentSessionsByTab = Record<string, { tool: ToolId; agentSessionId: string; cwd?: string }>;

export interface SnapshotOptions {
  /** What each console is running and where, from tabAgentSessions. */
  agentByTab?: AgentSessionsByTab;
  /** Where each console actually is, as the shell reported it (tabCwd). The
   * most reliable of the three sources, since it comes from the shell itself. */
  cwdByTab?: Record<string, string>;
  /** Store the conversation ids too, so restoring reopens the conversations
   * and not just the consoles. The folder is stored either way: a console that
   * moved should come back where it was, not where the app was launched. */
  withSessions?: boolean;
  /** Each console's id (pty-registry), to record with it. */
  consoleByTab?: Record<string, string>;
  /** The tab groups by id (tabGroupsStore), to record each tab's with it. */
  groups?: Record<string, GroupDef>;
}

/** A console to reopen a conversation in, once the restored tree is live. */
export interface LayoutResume {
  tabId: string;
  tool: ToolId;
  agentSessionId: string;
}

export interface HydratedLayout {
  root: SplitNode;
  resumes: LayoutResume[];
  /** Every group the layout's tabs were in, once each. */
  groups: GroupDef[];
}

export function snapshotLayout(root: SplitNode, options: SnapshotOptions = {}): LayoutNode {
  if (root.kind === "leaf") {
    const sessions = root.sessions.map((session) => snapshotSession(session, options));
    // Stored as a position, since the id it refers to isn't stored either.
    const activeIndex = Math.max(
      0,
      root.sessions.findIndex((session) => session.id === root.activeSessionId),
    );
    return { kind: "leaf", sessions, activeIndex };
  }
  return {
    kind: "split",
    direction: root.direction,
    ratio: root.ratio,
    children: [snapshotLayout(root.children[0], options), snapshotLayout(root.children[1], options)],
  };
}

/** One tab, as a layout stores it — also what a tab group keeps of its tabs. */
export function snapshotSession(session: SessionTab, options: SnapshotOptions = {}): LayoutSession {
  const stored: LayoutSession = { title: session.title };
  const group = session.groupId ? options.groups?.[session.groupId] : undefined;
  if (group) stored.group = { id: group.id, name: group.name, color: group.color };
  if (session.filePath !== undefined) {
    stored.filePath = session.filePath;
    return stored;
  }
  if (session.agentRun !== undefined) {
    stored.agentRun = { ...session.agentRun };
    return stored;
  }
  if (session.projectView !== undefined) {
    stored.projectView = { ...session.projectView };
    return stored;
  }
  if (session.orchestrationView !== undefined) {
    stored.orchestrationView = { ...session.orchestrationView };
    return stored;
  }
  if (session.diff !== undefined) {
    stored.diff = { ...session.diff };
    return stored;
  }
  if (session.gitGraph !== undefined) {
    stored.gitGraph = { ...session.gitGraph };
    return stored;
  }
  if (session.shellPath !== undefined) stored.shellPath = session.shellPath;
  if (session.shellArgs !== undefined) stored.shellArgs = [...session.shellArgs];
  if (session.cwd !== undefined) stored.cwd = session.cwd;
  // SessionTab.cwd is only where the console was *opened*. Both the agent
  // and the shell itself say where it ended up, and the shell (OSC 7) is
  // the more reliable of the two, so it has the last word.
  const agent = options.agentByTab?.[session.id];
  if (agent?.cwd) stored.cwd = agent.cwd;
  const reported = options.cwdByTab?.[session.id];
  if (reported) stored.cwd = reported;
  if (agent && options.withSessions) {
    stored.agentTool = agent.tool;
    stored.agentSessionId = agent.agentSessionId;
  }
  const consoleId = options.consoleByTab?.[session.id];
  if (consoleId) stored.consoleId = consoleId;
  if (consoleId && session.attachHost) stored.remoteHost = { ...session.attachHost };
  return stored;
}

/**
 * Rebuilds a live tree from a stored one, plus the list of consoles whose
 * conversation should be reopened. `newId` exists for tests; everything else
 * lets it default to the same uuid source the tree normally uses.
 *
 * Tolerant of hand-edited files: a leaf with no tabs still yields one console,
 * and an out-of-range active tab falls back to the first.
 *
 * A console whose `consoleId` is in `running` (the sessions daemon still
 * runs it) is shown again rather than started: its conversation is still
 * open in it, so there's nothing to resume.
 */
export function hydrateLayout(
  node: LayoutNode,
  newId?: () => string,
  running: ReadonlySet<string> = new Set(),
): HydratedLayout {
  const resumes: LayoutResume[] = [];
  const root = build(node, resumes, newId, running);
  return { root, resumes, groups: layoutGroups(node) };
}

function build(node: LayoutNode, resumes: LayoutResume[], newId: (() => string) | undefined, running: ReadonlySet<string>): SplitNode {
  if (node.kind === "leaf") {
    const stored = node.sessions.length > 0 ? node.sessions : [{ title: "Shell" } as LayoutSession];
    const sessions = stored.map((session) => hydrateSession(session, resumes, newId, running));
    const active = sessions[node.activeIndex] ?? sessions[0];
    const tile = withId(makeTile(sessions[0]), newId);
    return { ...tile, sessions, activeSessionId: active.id };
  }
  return {
    kind: "split",
    id: newId ? newId() : crypto.randomUUID(),
    direction: node.direction,
    ratio: node.ratio,
    children: [build(node.children[0], resumes, newId, running), build(node.children[1], resumes, newId, running)],
  };
}

/** One stored tab, live again: a fresh id, in its group if it had one. A
 * console whose conversation should be reopened is added to `resumes`. */
export function hydrateSession(
  session: LayoutSession,
  resumes: LayoutResume[],
  newId?: () => string,
  running: ReadonlySet<string> = new Set(),
): SessionTab {
  const live = hydrateTab(session, resumes, newId, running);
  return session.group ? { ...live, groupId: session.group.id } : live;
}

function hydrateTab(
  session: LayoutSession,
  resumes: LayoutResume[],
  newId: (() => string) | undefined,
  running: ReadonlySet<string>,
): SessionTab {
  if (session.filePath) return withId(makeFileSession(session.filePath), newId);
  if (session.agentRun)
    return withId(makeAgentRunSession(session.title, session.agentRun.agentId, session.agentRun.runId), newId);
  if (session.projectView) return withId(makeProjectSession(session.title, session.projectView.projectId), newId);
  if (session.orchestrationView)
    return withId(
      makeOrchestrationSession(
        session.title,
        session.orchestrationView.orchestrationId,
        session.orchestrationView.console === true,
      ),
      newId,
    );
  if (session.diff) return withId(makeDiffSession(session.diff), newId);
  if (session.gitGraph) return withId(makeGitGraphSession(session.title, session.gitGraph.root), newId);
  const live = withId(makeSession(session.title, session.shellPath, session.shellArgs, session.cwd), newId);
  if (session.consoleId && running.has(session.consoleId)) return { ...live, attach: session.consoleId };
  // Another computer's: whether it still runs is only known by asking it,
  // which the tab does as it opens (and falls back to a shell here).
  if (session.consoleId && session.remoteHost)
    return { ...live, attach: session.consoleId, attachHost: { ...session.remoteHost } };
  if (session.agentTool && session.agentSessionId) {
    resumes.push({ tabId: live.id, tool: session.agentTool, agentSessionId: session.agentSessionId });
  }
  return live;
}

/** The groups a stored tree's tabs were in, once each. */
function layoutGroups(node: LayoutNode, found = new Map<string, GroupDef>()): GroupDef[] {
  if (node.kind === "leaf") {
    for (const session of node.sessions) {
      if (session.group && !found.has(session.group.id)) found.set(session.group.id, { ...session.group });
    }
  } else {
    layoutGroups(node.children[0], found);
    layoutGroups(node.children[1], found);
  }
  return [...found.values()];
}

/** Whether this layout carries agent conversations — what the "IA" pill in the
 * layout lists means. */
export function layoutHasSessions(node: LayoutNode): boolean {
  if (node.kind === "leaf") {
    return node.sessions.some((session) => !!session.agentSessionId);
  }
  return layoutHasSessions(node.children[0]) || layoutHasSessions(node.children[1]);
}

function withId<T extends { id: string }>(value: T, newId?: () => string): T {
  return newId ? { ...value, id: newId() } : value;
}
