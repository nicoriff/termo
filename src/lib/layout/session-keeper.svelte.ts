import { resumeInTab } from "$lib/ai-mode/restore-session";
import { tabAgentSessions } from "$lib/ai-mode/tab-sessions.svelte";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { saveWindowSession, windowSession } from "$lib/core/layouts-client";
import { keptConsoles, shownHere } from "$lib/core/pty-client";
import { ptyIdsByTab } from "$lib/terminal/pty-registry";
import { tabCwd } from "$lib/terminal/tab-cwd.svelte";
import { hydrateLayout, snapshotLayout, type LayoutNode } from "./layout-snapshot";
import { workspaceStore } from "./workspace-store.svelte";

/** How long after a change to the tree it's recorded. */
const SAVE_DELAY_MS = 300;
/** And every so often regardless: a console's folder changes without the
 * tree changing. */
const SAVE_EVERY_MS = 5000;

/**
 * With "Keep consoles running" on, consoles outlive their window (see
 * src-tauri/src/mux/). This keeps a record of what the window has open —
 * its tabs and splits, each console's id, folder and conversation — so that
 * opening Termo again puts every console still running back where it was.
 */
class SessionKeeper {
  private last = "";
  private timer: ReturnType<typeof setTimeout> | null = null;

  /** Records the window's tree as it changes. Returns a teardown. */
  start(): () => void {
    const stopEffects = $effect.root(() => {
      $effect(() => {
        void workspaceStore.root;
        void appConfigStore.persistentSessions;
        this.schedule();
      });
    });
    const interval = setInterval(() => void this.save(), SAVE_EVERY_MS);
    return () => {
      stopEffects();
      clearInterval(interval);
      if (this.timer) clearTimeout(this.timer);
    };
  }

  private schedule(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.save();
    }, SAVE_DELAY_MS);
  }

  /** Records the tree now, if it changed. Also what quitting calls, so a tab
   * closed just before isn't brought back. */
  async save(): Promise<void> {
    const root = workspaceStore.root;
    if (!root || !appConfigStore.loaded || !appConfigStore.persistentSessions) return;
    const snapshot = snapshotLayout(root, {
      agentByTab: tabAgentSessions.byTab,
      cwdByTab: tabCwd.byTab,
      withSessions: true,
      consoleByTab: ptyIdsByTab(),
    });
    const text = JSON.stringify(snapshot);
    if (text === this.last) return;
    this.last = text;
    await saveWindowSession(snapshot).catch(() => {});
  }

  /**
   * At startup: if consoles this window had open are still running, puts its
   * tree back with them in it — the ones that ended start anew in their
   * folder, reopening their conversation. False when there was nothing to
   * bring back, and the window starts as usual.
   */
  async restore(): Promise<boolean> {
    const running = await keptConsoles().catch(() => []);
    // Not one another window shows already; one a phone on the tailnet is
    // watching still comes back here.
    const free = new Set(running.filter((console) => !shownHere(console)).map((console) => console.id));
    if (free.size === 0) return false;
    const saved = await windowSession().catch(() => null);
    if (!saved || !mentionsAny(saved, free)) return false;
    const { root, resumes } = hydrateLayout(saved, undefined, free);
    workspaceStore.replaceRoot(root);
    for (const resume of resumes) {
      void resumeInTab(resume.tabId, resume.tool, resume.agentSessionId);
    }
    return true;
  }
}

function mentionsAny(node: LayoutNode, ids: ReadonlySet<string>): boolean {
  if (node.kind === "leaf") return node.sessions.some((session) => !!session.consoleId && ids.has(session.consoleId));
  return mentionsAny(node.children[0], ids) || mentionsAny(node.children[1], ids);
}

export const sessionKeeper = new SessionKeeper();
