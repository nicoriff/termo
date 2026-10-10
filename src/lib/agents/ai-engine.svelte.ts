import type { AgentEngine } from "$lib/core/agents-client";
import { newTabOptions } from "$lib/layout/new-tab-options.svelte";

/** Every engine that can answer, the order they're offered in. */
const ENGINES: AgentEngine[] = ["claude", "codex", "gemini", "copilot", "grok"];

/** The ones whose CLI holds the model to the shape asked for. The others get
 * it in the prompt, and Termo checks what comes back (see agents/draft.rs). */
const HOLDS_THE_SHAPE: AgentEngine[] = ["claude", "codex"];

const STORAGE_KEY = "termo.aiEngine";

function remembered(): AgentEngine | null {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return ENGINES.find((engine) => engine === saved) ?? null;
  } catch {
    return null;
  }
}

/**
 * Whose CLI Termo asks when it drafts something with AI: an agent, a team,
 * an orchestration's plan. The user's pick, kept from one time to the next;
 * without one, the first installed — Claude Code when it's there.
 */
class AiEngine {
  private picked = $state<AgentEngine | null>(remembered());

  /** The engines installed here; all of them until the first check answers. */
  get installed(): AgentEngine[] {
    const clis = newTabOptions.agentClis;
    return ENGINES.filter((engine) => clis?.[engine] !== false);
  }

  /** Null when no agent CLI is installed at all. */
  get value(): AgentEngine | null {
    const installed = this.installed;
    return this.picked && installed.includes(this.picked) ? this.picked : (installed[0] ?? null);
  }

  /** Whether this engine's answers come checked by Termo rather than held
   * to the shape by its own CLI. */
  isChecked(engine: AgentEngine): boolean {
    return !HOLDS_THE_SHAPE.includes(engine);
  }

  pick(engine: AgentEngine): void {
    this.picked = engine;
    try {
      localStorage.setItem(STORAGE_KEY, engine);
    } catch {
      // Not remembered, then: it still holds for this window.
    }
  }
}

export const aiEngine = new AiEngine();
