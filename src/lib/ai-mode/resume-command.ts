import type { ToolId } from "./pattern-table";

/** Session ids are UUIDs (Codex also accepts a session name). Anything outside
 * this set would have to be quoted, and quoting rules differ between PowerShell,
 * bash and cmd.exe — the command is typed into whichever shell the tab runs,
 * or handed to the agent on a command line — so those are simply not offered
 * for restoring. */
const SAFE_ID = /^[A-Za-z0-9._-]+$/;

/**
 * What follows the agent's command to reopen a past conversation, or null
 * when the id isn't safe to pass on. Every form comes from the tools' own
 * docs: `claude --resume <id>` (searches the current project, its worktrees,
 * then other projects), `codex resume <id>` (UUID or session name), `gemini
 * --resume <id>` (UUID, an index, or "latest") and `copilot --resume <id>`.
 */
export function resumeArgs(tool: ToolId, agentSessionId: string): string[] | null {
  if (!SAFE_ID.test(agentSessionId)) return null;
  return tool === "codex" ? ["resume", agentSessionId] : ["--resume", agentSessionId];
}

/** The same as one command line, for typing into a console. */
export function resumeCommand(tool: ToolId, agentSessionId: string): string | null {
  const args = resumeArgs(tool, agentSessionId);
  return args ? [tool, ...args].join(" ") : null;
}
