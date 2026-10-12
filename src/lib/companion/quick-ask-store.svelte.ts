import { listen } from "@tauri-apps/api/event";
import {
  companionQuickAsk,
  type CompanionTool,
  type Exchange,
  type QuickStep,
} from "../core/companion-client";
import type { PlanStep } from "../core/orchestration-client";

/** A question asked in the popup, and how it came out. */
export interface QuickEntry {
  id: string;
  question: string;
  /** Null while it's being answered, and when it failed. */
  answer: string | null;
  /** The CLI's own words for why it failed. */
  error: string | null;
  costUsd: number | null;
  /** Whose CLI answered, and on which model when it says. */
  engine: CompanionTool;
  model: string | null;
}

/**
 * The quick question popup's conversation. It's one for the whole window and
 * lasts while the app runs: closing the popup doesn't lose what was asked,
 * and "new" starts it over.
 */
class QuickAskStore {
  entries = $state<QuickEntry[]>([]);
  /** The id of the question being answered. */
  asking = $state<string | null>(null);
  /** What the CLI is doing on the way to the answer. */
  steps = $state<PlanStep[]>([]);

  /** Listens for the steps of whatever is being asked. Returns a teardown
   * function. */
  start(): () => void {
    const unlisten = listen<QuickStep>("companion-quick-step", ({ payload }) => {
      if (payload.id === this.asking) this.steps = [...this.steps, payload.step];
    });
    return () => void unlisten.then((stop) => stop());
  }

  clear(): void {
    if (this.asking) return;
    this.entries = [];
  }

  /** Asks `question`, with the answered ones before it for a follow-up to
   * make sense, and `exchange` when the session's last one goes along. */
  async ask(question: string, engine: CompanionTool, cwd: string | null, exchange: Exchange | null): Promise<void> {
    if (this.asking) return;
    const id = crypto.randomUUID();
    const history = this.entries.flatMap((entry) => (entry.answer ? [{ question: entry.question, answer: entry.answer }] : []));
    this.entries = [...this.entries, { id, question, answer: null, error: null, costUsd: null, engine, model: null }];
    this.asking = id;
    this.steps = [];
    const settle = (change: Partial<QuickEntry>) => {
      this.entries = this.entries.map((entry) => (entry.id === id ? { ...entry, ...change } : entry));
    };
    try {
      const answered = await companionQuickAsk(id, { engine, cwd, question, history, exchange });
      settle({ answer: answered.answer, costUsd: answered.costUsd, model: answered.model });
    } catch (reason) {
      settle({ error: String(reason) });
    } finally {
      this.asking = null;
      this.steps = [];
    }
  }
}

export const quickAskStore = new QuickAskStore();
