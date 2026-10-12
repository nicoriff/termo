import { TOOL_IDS, type ToolId } from "../ai-mode/pattern-table";

/** Which Node.js each agent's CLI was last found under, by agent
 * (AppConfig's `agent_clis_known`). */
export type KnownAgents = Record<string, string>;

export interface MissingCheck {
  /** The agents that were there and went with a change of Node.js. */
  missing: ToolId[];
  /** `known`, brought up to date. */
  known: KnownAgents;
}

/**
 * Tells an agent's CLI that went missing with a change of Node.js from one
 * the user took off themselves.
 *
 * npm keeps what's installed globally apart for each version of Node.js that
 * a version manager (nvm, fnm, Volta) holds, so switching version leaves
 * every agent installed that way behind. An agent found before, not found
 * now, and last found under another Node.js than the one running is that
 * case. Not found under the same Node.js, it was uninstalled, and is
 * forgotten. Without Node.js there's no telling, nor any npm to put it back
 * with, so nothing is said and nothing forgotten.
 */
export function checkMissingAgents(known: KnownAgents, installed: Record<ToolId, boolean>, node: string | null): MissingCheck {
  const next: KnownAgents = {};
  const missing: ToolId[] = [];
  for (const tool of TOOL_IDS) {
    const before = known[tool];
    if (installed[tool]) {
      if (node) next[tool] = node;
      else if (before !== undefined) next[tool] = before;
      continue;
    }
    if (before === undefined) continue;
    if (!node) next[tool] = before;
    else if (before !== node) {
      next[tool] = before;
      missing.push(tool);
    }
  }
  return { missing, known: next };
}

/** `known` without `tools`: what "don't ask again" leaves. */
export function forgetAgents(known: KnownAgents, tools: readonly ToolId[]): KnownAgents {
  return Object.fromEntries(Object.entries(known).filter(([tool]) => !tools.includes(tool as ToolId)));
}

export function sameKnown(a: KnownAgents, b: KnownAgents): boolean {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((key) => a[key] === b[key]);
}
