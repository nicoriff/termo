import { TOOL_PATTERNS, type ToolId } from "./pattern-table";

export type AiSessionStatus = "running" | "awaiting" | "error" | "finished";

export interface ClassifyResult {
  /** null = not (yet, or ever) recognized as an AI-tool session — no dot shown. */
  tool: ToolId | null;
  status: AiSessionStatus | null;
}

export interface ClassifyState {
  /**
   * Sticky once matched: the identity banner is usually only visible near
   * session start and scrolls out of the ~20-row window this is fed almost
   * immediately, so "was this ever detected" has to persist across calls
   * for a given session rather than being re-derived from the current
   * screen alone.
   */
  detectedTool: ToolId | null;
}

/**
 * Classifies a terminal session's current state from its most recently
 * rendered screen lines (plain text, ANSI already stripped by xterm.js).
 * Pure function — no xterm/Tauri/DOM imports — so it stays testable against
 * plain-text fixtures without spinning up a real terminal.
 */
export function classify(lines: string[], state: ClassifyState, processPath: string | null): ClassifyResult {
  const text = lines.join("\n");

  if (!state.detectedTool) {
    // Process path is a confirming/bonus signal (and Codex's primary one,
    // since its text patterns are the least trustworthy part of this
    // table) — check it first, then fall back to a text-banner match.
    const pathLower = processPath?.toLowerCase() ?? null;
    const byProcess = TOOL_PATTERNS.find((p) => pathLower && p.processPathHints.some((hint) => pathLower.includes(hint)));
    const byText = TOOL_PATTERNS.find((p) => p.identity.some((re) => re.test(text)));
    state.detectedTool = byProcess?.tool ?? byText?.tool ?? null;
  }
  if (!state.detectedTool) return { tool: null, status: null };

  const rules = TOOL_PATTERNS.find((p) => p.tool === state.detectedTool)!;

  if (rules.awaiting.some((re) => re.test(text))) return { tool: rules.tool, status: "awaiting" };
  if (rules.running.some((re) => re.test(text))) return { tool: rules.tool, status: "running" };
  // Scoped to an already-identified session to cut down (not eliminate)
  // false positives from ordinary shell output containing "error"/"failed".
  if (rules.error.some((re) => re.test(text))) return { tool: rules.tool, status: "error" };
  return { tool: rules.tool, status: "finished" };
}
