import type { ToolId } from "./pattern-table";

/** Product names, shown as-is in both languages (see CLAUDE.md's i18n rule:
 * brands are the exception). Shared by the notifications and the status bar. */
export const TOOL_DISPLAY_NAMES: Record<ToolId, string> = {
  claude: "Claude Code",
  codex: "Codex",
};
