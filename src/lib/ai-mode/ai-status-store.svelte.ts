import type { AiSessionStatus, ClassifyResult } from "./classifier";
import type { ToolId } from "./pattern-table";

export interface AiSessionEntry extends ClassifyResult {
  /** When `status` last changed (ms since epoch). */
  since: number;
  /** The status came from the tool's own lifecycle hooks. */
  fromHook: boolean;
}

export type StatusChangeListener = (
  sessionId: string,
  entry: AiSessionEntry,
  prev: AiSessionEntry | undefined,
) => void;

class AiStatusStore {
  byId = $state<Record<string, AiSessionEntry>>({});
  private listeners = new Set<StatusChangeListener>();

  /** Screen classification. Once a session's hooks report, they own its
   * status; the screen can still flag an error, or see work resume after a
   * permission prompt was answered — no hook fires for that until the
   * approved tool call finishes. It can't clear a hook-owned session: a null
   * tool only means the screen never recognized the tool (always the case on
   * Windows, which has no foreground-process signal), so clearing on it made
   * the status flicker back to nothing between hook events. The SessionEnd
   * hook or closing the tab clears it instead. */
  set(sessionId: string, result: ClassifyResult): void {
    const prev = this.byId[sessionId];
    if (prev?.fromHook) {
      const allowed =
        result.tool !== null &&
        (result.status === "error" || (prev.status === "awaiting" && result.status === "running"));
      if (!allowed) return;
    }
    this.apply(sessionId, result, false);
  }

  /** `status: null` means the agent session ended. */
  setFromHook(sessionId: string, tool: ToolId, status: AiSessionStatus | null): void {
    if (status === null) {
      this.remove(sessionId);
      return;
    }
    this.apply(sessionId, { tool, status }, true);
  }

  onStatusChange(listener: StatusChangeListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private apply(sessionId: string, result: ClassifyResult, fromHook: boolean): void {
    const prev = this.byId[sessionId];
    const keepsHook = result.tool !== null && (fromHook || (prev?.fromHook ?? false));
    if (prev?.tool === result.tool && prev?.status === result.status) {
      if (keepsHook && !prev.fromHook) this.byId = { ...this.byId, [sessionId]: { ...prev, fromHook: true } };
      return;
    }
    const since = prev && prev.status === result.status ? prev.since : Date.now();
    const entry: AiSessionEntry = { ...result, since, fromHook: keepsHook };
    this.byId = { ...this.byId, [sessionId]: entry };
    for (const listener of this.listeners) listener(sessionId, entry, prev);
  }

  remove(sessionId: string): void {
    if (!(sessionId in this.byId)) return;
    const next = { ...this.byId };
    delete next[sessionId];
    this.byId = next;
  }

  get counts(): Record<AiSessionStatus, number> {
    const counts: Record<AiSessionStatus, number> = { running: 0, awaiting: 0, error: 0, finished: 0 };
    for (const r of Object.values(this.byId)) {
      if (r.status) counts[r.status]++;
    }
    return counts;
  }

  /** Sessions currently in `status`, longest-standing first. */
  entriesWith(status: AiSessionStatus): { sessionId: string; since: number }[] {
    return Object.entries(this.byId)
      .filter(([, entry]) => entry.status === status)
      .map(([sessionId, entry]) => ({ sessionId, since: entry.since }))
      .sort((a, b) => a.since - b.since);
  }
}

export const aiStatusStore = new AiStatusStore();
