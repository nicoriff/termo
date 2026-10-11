import { aiStatusStore } from "../ai-mode/ai-status-store.svelte";
import { tabAgentSessions } from "../ai-mode/tab-sessions.svelte";
import { appConfigStore } from "../core/app-config.svelte";
import { companionLive, type Live } from "../core/companion-client";
import { nudgeFor, type Nudge } from "./companion";

/** How often the clock the cache's expiry is read against moves on. */
const TICK_MS = 20_000;

interface TabReading {
  /** The agent's own id for the session the reading is of. */
  sessionId: string;
  live: Live;
}

/**
 * The Companion's quiet notices: one per tab at most, on the status bar.
 * A tab's session is read as each of its agent's turns ends — that's when
 * its file has something new, and when a word about the next turn is of any
 * use — and never while the agent is working.
 */
class CompanionNudges {
  private byTab = $state<Record<string, TabReading>>({});
  /** Notices already looked at, by their key: each is shown until then. */
  private seen = $state<Record<string, true>>({});
  private now = $state(Date.now());

  /** Watches the agents' turns. Returns a teardown function. */
  start(): () => void {
    const stop = aiStatusStore.onStatusChange((tabId, entry, prev) => {
      if (entry.status === "running") return;
      // The agent is gone from the tab, and its notice with it.
      if (entry.tool === null) return this.forget(tabId);
      if (prev?.status === "running") void this.read(tabId);
    });
    const timer = setInterval(() => (this.now = Date.now()), TICK_MS);
    return () => {
      stop();
      clearInterval(timer);
    };
  }

  private forget(tabId: string): void {
    if (!(tabId in this.byTab)) return;
    const next = { ...this.byTab };
    delete next[tabId];
    this.byTab = next;
  }

  private async read(tabId: string): Promise<void> {
    if (!appConfigStore.aiModeEnabled || !appConfigStore.companionNudges) return;
    const session = tabAgentSessions.byTab[tabId];
    if (!session) return;
    try {
      const live = await companionLive(session.tool, session.agentSessionId);
      if (live) this.byTab = { ...this.byTab, [tabId]: { sessionId: session.agentSessionId, live } };
    } catch {
      // No notice this turn; the next one asks again.
    }
  }

  /** What there is to say about `tabId`'s session now, if anything. */
  for(tabId: string | null): Nudge | null {
    if (!tabId || !appConfigStore.aiModeEnabled || !appConfigStore.companionNudges) return null;
    const reading = this.byTab[tabId];
    if (!reading) return null;
    // A notice about the turn before is out of date while the next one runs.
    if (aiStatusStore.byId[tabId]?.status === "running") return null;
    return nudgeFor(reading.sessionId, reading.live, this.now, appConfigStore.companionMuted, this.seen);
  }

  /** Looked at: it isn't shown again. */
  markSeen(nudge: Nudge): void {
    this.seen = { ...this.seen, [nudge.key]: true };
  }
}

export const companionNudges = new CompanionNudges();
