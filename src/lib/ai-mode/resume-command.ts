import type { ToolId } from "./pattern-table";

/** Session ids are UUIDs (Codex also accepts a session name). Anything outside
 * this set would have to be quoted, and quoting rules differ between PowerShell
 * and bash — the command is typed into whichever shell the tab runs — so those
 * are simply not offered for restoring. */
const SAFE_ID = /^[A-Za-z0-9._-]+$/;

/**
 * The command that reopens a past conversation, or null when the id isn't safe
 * to type into a shell. Both forms come from the tools' own docs: `claude
 * --resume <id>` (searches the current project, its worktrees, then other
 * projects) and `codex resume <id>` (UUID or session name).
 */
export function resumeCommand(tool: ToolId, agentSessionId: string): string | null {
  if (!SAFE_ID.test(agentSessionId)) return null;
  return tool === "claude" ? `claude --resume ${agentSessionId}` : `codex resume ${agentSessionId}`;
}
