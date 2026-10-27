import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
import { resumeInTab } from "$lib/ai-mode/restore-session";
import { tabAgentSessions } from "$lib/ai-mode/tab-sessions.svelte";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { keptConsoles, ptyBusy } from "$lib/core/pty-client";
import { collectTiles, isConsoleTab, type SessionLocation } from "$lib/layout/split-tree";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { getPtyId } from "$lib/terminal/pty-registry";
import { markReleasing, setPreface } from "$lib/terminal/releasing";
import { tabCwd } from "$lib/terminal/tab-cwd.svelte";
import { getTerminal } from "$lib/terminal/terminal-registry";
import { restIsDue } from "./tab-groups";
import { tabGroupsStore } from "./tab-groups-store.svelte";

/** How often collapsed groups are looked at. */
const SWEEP_EVERY_MS = 30_000;
/** How much of a discarded console's screen is kept, in lines. */
const SCREEN_LINES = 200;

/**
 * Saves memory on the tabs of tab groups collapsed for a while (Preferences
 * → Appearance → Tabs), as Chrome's Memory Saver does:
 *
 * - "sleep": the console's view goes (its xterm, its share of the web view)
 *   while the console keeps running in the sessions daemon; the tab attaches
 *   to it again, as it was, when it comes into view. Only for consoles the
 *   daemon runs ("Keep consoles running").
 * - "discard": a console sitting idle at its prompt ends, its shell process
 *   too; it starts again in its folder when the tab comes into view, its
 *   last screen above and its agent conversation reopened. A busy one is
 *   put to sleep instead, when it can be.
 *
 * Never a tab in view, nor one whose agent is working or waiting on you.
 */
class TabSaver {
  private sweeping = false;

  /** The main window calls it once; returns what stops it. */
  start(): () => void {
    const timer = setInterval(() => void this.sweep(), SWEEP_EVERY_MS);
    const stopEffects = $effect.root(() => {
      // A tab at rest wakes as it becomes its tile's tab in view.
      $effect(() => {
        const root = workspaceStore.root;
        if (!root) return;
        for (const tile of collectTiles(root)) {
          const active = tile.sessions.find((session) => session.id === tile.activeSessionId);
          if (active?.dormant) this.wake(active.id);
        }
      });
    });
    return () => {
      clearInterval(timer);
      stopEffects();
    };
  }

  private async sweep(): Promise<void> {
    const mode = appConfigStore.tabSaver;
    const root = workspaceStore.root;
    if (mode === "off" || !root || this.sweeping) return;
    this.sweeping = true;
    try {
      const now = Date.now();
      const due = tabGroupsStore.groups.filter((group) => restIsDue(group, now, appConfigStore.tabSaverMinutes));
      if (due.length === 0) return;
      const inView = new Set(collectTiles(root).map((tile) => tile.activeSessionId));
      const candidates = due
        .flatMap((group) => tabGroupsStore.liveTabs.get(group.id) ?? [])
        .filter((loc) => this.canRest(loc, inView));
      if (candidates.length === 0) return;
      // The consoles the sessions daemon runs: only those can sleep.
      const kept = new Set(
        appConfigStore.persistentSessions ? (await keptConsoles().catch(() => [])).map((console) => console.id) : [],
      );
      for (const loc of candidates) {
        const ptyId = getPtyId(loc.session.id);
        if (!ptyId) continue;
        if (mode === "discard" && !(await ptyBusy(ptyId).catch(() => true))) this.discard(loc);
        else if (kept.has(ptyId)) this.sleep(loc, ptyId);
      }
    } finally {
      this.sweeping = false;
    }
  }

  /** A console, out of view, not at rest already, and whose agent isn't
   * working or waiting on you. */
  private canRest(loc: SessionLocation, inView: Set<string>): boolean {
    const session = loc.session;
    if (!isConsoleTab(session) || session.dormant || inView.has(session.id) || session.attachHost) return false;
    const status = aiStatusStore.byId[session.id]?.status;
    return status !== "running" && status !== "awaiting";
  }

  private sleep(loc: SessionLocation, ptyId: string): void {
    // Its <Terminal> unmounts on this, and lets go of the console rather
    // than ending it.
    markReleasing(loc.session.id);
    workspaceStore.setDormant(loc.session.id, { kind: "asleep" }, { attach: ptyId });
  }

  private discard(loc: SessionLocation): void {
    const id = loc.session.id;
    // What the tab knows, before its console and all of that go with it.
    const screen = getTerminal(id)?.recentText(SCREEN_LINES);
    const agent = tabAgentSessions.byTab[id];
    const cwd = tabCwd.byTab[id] ?? agent?.cwd ?? loc.session.cwd;
    workspaceStore.setDormant(
      id,
      {
        kind: "discarded",
        ...(screen ? { screen } : {}),
        ...(agent ? { resume: { tool: agent.tool, agentSessionId: agent.agentSessionId } } : {}),
      },
      // A new console then: in its folder, a shell (the agent comes back by
      // its conversation), and not the one it was attached to.
      { cwd, agent: undefined, attach: undefined, attachHost: undefined },
    );
  }

  private wake(id: string): void {
    const loc = workspaceStore.root
      ? collectTiles(workspaceStore.root)
          .flatMap((tile) => tile.sessions)
          .find((session) => session.id === id)
      : undefined;
    const dormant = loc?.dormant;
    if (!dormant) return;
    if (dormant.kind === "discarded" && dormant.screen) setPreface(id, dormant.screen);
    workspaceStore.setDormant(id, undefined);
    if (dormant.kind === "discarded" && dormant.resume) {
      void resumeInTab(id, dormant.resume.tool, dormant.resume.agentSessionId);
    }
  }
}

export const tabSaver = new TabSaver();
