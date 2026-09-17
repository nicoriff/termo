import { resolveDefaultShell } from "$lib/core/default-shell";
import { writePty } from "$lib/core/pty-client";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { getPtyId } from "$lib/terminal/pty-registry";
import type { AgentSession } from "$lib/core/agent-sessions-client";
import type { ToolId } from "./pattern-table";
import { resumeCommand } from "./resume-command";
import { TOOL_DISPLAY_NAMES } from "./tool-names";

/** How long to keep looking for the new tab's console: spawning it is a round
 * trip to the backend, so its PTY id doesn't exist the moment the tab does. */
const WAIT_FOR_CONSOLE_MS = 5000;
const POLL_MS = 100;

/** The folder name, which is what identifies a session at a glance. */
export function sessionLabel(session: AgentSession): string {
  const path = session.cwd ?? session.projectDir;
  const folder = path?.split(/[\\/]/).filter(Boolean).pop();
  return folder ? `${TOOL_DISPLAY_NAMES[session.tool]} · ${folder}` : TOOL_DISPLAY_NAMES[session.tool];
}

/** Whether this session can be reopened: it needs an id safe to type and a
 * console to type it into. */
export function canRestore(session: AgentSession): boolean {
  return resumeCommand(session.tool, session.agentSessionId) !== null;
}

/**
 * Opens a console in the session's folder and types the agent's resume command
 * into it. Typed rather than passed as shell arguments so it works the same in
 * PowerShell and bash, and so the user sees exactly what ran.
 */
export async function restoreSession(session: AgentSession, targetTileId?: string): Promise<void> {
  const command = resumeCommand(session.tool, session.agentSessionId);
  const tileId = targetTileId ?? workspaceStore.focusedTileId;
  if (!command || !tileId) return;

  const shell = await resolveDefaultShell();
  const tabId = workspaceStore.addTab(
    tileId,
    shell.path,
    shell.args,
    TOOL_DISPLAY_NAMES[session.tool],
    session.cwd ?? session.projectDir ?? undefined,
  );
  if (!tabId) return;

  const ptyId = await waitForConsole(tabId);
  if (ptyId) await writePty(ptyId, `${command}\r`);
}

/** Reopens a conversation in a console that already exists — what restoring a
 * layout saved with sessions does, tab by tab. */
export async function resumeInTab(tabId: string, tool: ToolId, agentSessionId: string): Promise<void> {
  const command = resumeCommand(tool, agentSessionId);
  if (!command) return;
  const ptyId = await waitForConsole(tabId);
  if (ptyId) await writePty(ptyId, `${command}\r`);
}

function waitForConsole(tabId: string): Promise<string | null> {
  return new Promise((resolve) => {
    const deadline = Date.now() + WAIT_FOR_CONSOLE_MS;
    const check = () => {
      const ptyId = getPtyId(tabId);
      if (ptyId) return resolve(ptyId);
      if (Date.now() > deadline) return resolve(null);
      setTimeout(check, POLL_MS);
    };
    check();
  });
}
