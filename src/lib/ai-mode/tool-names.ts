import type { ToolId } from "./pattern-table";

/** Product names, shown as-is in both languages (see CLAUDE.md's i18n rule:
 * brands are the exception). Shared by the notifications and the status bar. */
export const TOOL_DISPLAY_NAMES: Record<ToolId, string> = {
  claude: "Claude Code",
  codex: "Codex",
  gemini: "Gemini CLI",
};

/** Which agents report their plan's usage: Claude Code through its status
 * line, Codex through its rollout. Gemini CLI has no such source, so the
 * status bar doesn't wait for a reading that would never come. */
export const REPORTS_PLAN_USAGE: Record<ToolId, boolean> = {
  claude: true,
  codex: true,
  gemini: false,
};
