import {
  companionOverview,
  companionRecent,
  companionSession,
  type SessionReport,
  type SessionSummary,
} from "../core/companion-client";

/** The stretches the period view offers, in days. */
export const PERIODS = [7, 30] as const;
export type PeriodDays = (typeof PERIODS)[number];

/** The docked panel's width: a default, and how far it can be dragged. */
export const PANEL_WIDTH = { initial: 400, min: 320, max: 680 };
const WIDTH_KEY = "termo.companion.width";

export function clampPanelWidth(width: number): number {
  return Math.round(Math.min(PANEL_WIDTH.max, Math.max(PANEL_WIDTH.min, width)));
}

/** Only a convenience: a browser storage that's missing or throws just means
 * the default width. */
function loadWidth(): number {
  try {
    const saved = Number(localStorage.getItem(WIDTH_KEY));
    return saved ? clampPanelWidth(saved) : PANEL_WIDTH.initial;
  } catch {
    return PANEL_WIDTH.initial;
  }
}

/** Which session to show: an agent's own id for it. */
export interface SessionRef {
  tool: string;
  sessionId: string;
}

/**
 * The session the Companion panel is showing. Reading one goes through a
 * file that can be tens of megabytes, so it's asked for when the panel opens,
 * when what it should show changes, and when the user asks — never on a timer.
 */
class CompanionStore {
  width = $state(loadWidth());
  report = $state<SessionReport | null>(null);
  loading = $state(false);
  failed = $state(false);
  /** No tab's session was asked for: this is just the latest one on disk. */
  latest = $state(false);
  /** There are no sessions at all. */
  empty = $state(false);

  /** A session picked from the period's list, shown instead of the tab's. */
  picked = $state<SessionRef | null>(null);

  days = $state<PeriodDays>(7);
  /** The period's sessions in brief; null until they've been read. */
  overview = $state<SessionSummary[] | null>(null);
  overviewLoading = $state(false);
  overviewFailed = $state(false);

  /** The last request made, so an answer that arrives late for a session
   * since moved on from is dropped. */
  private asked = 0;
  private askedOverview = 0;

  /** `persist` once a drag is let go, not on every pixel of it. */
  setWidth(width: number, persist = false): void {
    this.width = clampPanelWidth(width);
    if (!persist) return;
    try {
      localStorage.setItem(WIDTH_KEY, String(this.width));
    } catch {
      // Not remembered; it still applies.
    }
  }

  /** Shows `ref`'s session, or the latest one of any agent's without one.
   * What's on screen stays until the new reading arrives. */
  async load(ref: SessionRef | null): Promise<void> {
    const asked = ++this.asked;
    this.loading = true;
    try {
      let target = ref;
      if (!target) {
        const [latest] = await companionRecent();
        target = latest ?? null;
      }
      const report = target ? await companionSession(target.tool, target.sessionId) : null;
      if (asked !== this.asked) return;
      this.report = report;
      this.latest = !ref && report !== null;
      this.empty = report === null;
      this.failed = false;
    } catch {
      if (asked !== this.asked) return;
      this.failed = true;
    } finally {
      if (asked === this.asked) this.loading = false;
    }
  }

  /** Reads the last `days` of every agent's sessions. What's on screen
   * stays until the new reading arrives, unless the stretch changed. */
  async loadOverview(days: PeriodDays): Promise<void> {
    const asked = ++this.askedOverview;
    if (days !== this.days) this.overview = null;
    this.days = days;
    this.overviewLoading = true;
    try {
      const overview = await companionOverview(days);
      if (asked !== this.askedOverview) return;
      this.overview = overview;
      this.overviewFailed = false;
    } catch {
      if (asked !== this.askedOverview) return;
      this.overviewFailed = true;
    } finally {
      if (asked === this.askedOverview) this.overviewLoading = false;
    }
  }
}

export const companionStore = new CompanionStore();
