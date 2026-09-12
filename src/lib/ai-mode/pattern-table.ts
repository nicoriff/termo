/**
 * Data only — no control flow here. This is the one file to edit when
 * tuning detection for either tool, especially Codex (see confidence
 * below): add/adjust regexes, don't touch classifier.ts.
 */

export type ToolId = "claude" | "codex";

export interface ToolPatternSet {
  tool: ToolId;
  /** Documents how much to trust this tool's rules — not read at runtime. */
  confidence: "high" | "low";
  /** Confirms "this session is running `tool`" once matched. */
  identity: RegExp[];
  running: RegExp[];
  awaiting: RegExp[];
  error: RegExp[];
  /**
   * Case-insensitive substrings checked against the foreground process's
   * full resolved executable path (unix only) — not an exact basename
   * match. Claude Code's installer symlinks a stable `claude` name to a
   * version-pinned binary (e.g. `~/.local/share/claude/versions/2.1.269`),
   * so the resolved path's basename is a version string, not "claude"; the
   * substring "claude" still shows up earlier in that same path, which is
   * what this actually matches against.
   */
  processPathHints: string[];
}

export const TOOL_PATTERNS: ToolPatternSet[] = [
  {
    tool: "claude",
    confidence: "high",
    processPathHints: ["claude"],
    // Not anchored to line start: the banner's first few rows share their
    // line with the mascot art rendered to its left, so "Claude Code v#.#.#"
    // starts at whatever column the art block ends at, not column 0.
    identity: [/Claude Code v\d+\.\d+\.\d+/],
    // The word before "…" is a randomized whimsical verb ("Twisting…",
    // "Caramelizing…") — anchor on the surrounding duration+token structure
    // instead, which is stable regardless of vocabulary. Deliberately does
    // NOT match on "esc to interrupt" alone: that hint sits in the footer
    // whether or not a turn is actively running, so by itself it can't
    // distinguish running from idle.
    running: [/\([\d.]+[a-z]?\s+\d+[ms]?\s*·\s*[↓↑]/, /·\s*[↓↑]\s*[\d.]+k?\s*tokens?\)/],
    // A numbered choice list with a selection marker, e.g. "❯ 1." then "2.".
    awaiting: [/❯\s*1\./],
    error: [/API Error/],
  },
  {
    tool: "codex",
    // Placeholder — not observed from a real Codex session. Tune this once
    // real screen captures of each state are available.
    confidence: "low",
    processPathHints: ["codex"],
    identity: [/codex/i],
    running: [/[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]/],
    awaiting: [/\(y\/n\)/i, /\[y\/N\]/, /\bAllow\b/, /\bApprove\b/, /proceed\?/i],
    error: [/\bError\b/, /\bTraceback\b/, /\bfailed\b/i],
  },
];
