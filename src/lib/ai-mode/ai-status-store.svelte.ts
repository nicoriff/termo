import type { AiSessionStatus, ClassifyResult } from "./classifier";

class AiStatusStore {
  byId = $state<Record<string, ClassifyResult>>({});

  set(sessionId: string, result: ClassifyResult): void {
    const prev = this.byId[sessionId];
    if (prev?.tool === result.tool && prev?.status === result.status) return;
    this.byId = { ...this.byId, [sessionId]: result };
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
}

export const aiStatusStore = new AiStatusStore();
