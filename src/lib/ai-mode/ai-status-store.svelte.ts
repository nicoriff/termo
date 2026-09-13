import type { AiSessionStatus, ClassifyResult } from "./classifier";

export interface AiSessionEntry extends ClassifyResult {
  /** When `status` last changed (ms since epoch). */
  since: number;
}

class AiStatusStore {
  byId = $state<Record<string, AiSessionEntry>>({});

  set(sessionId: string, result: ClassifyResult): void {
    const prev = this.byId[sessionId];
    if (prev?.tool === result.tool && prev?.status === result.status) return;
    const since = prev && prev.status === result.status ? prev.since : Date.now();
    this.byId = { ...this.byId, [sessionId]: { ...result, since } };
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
