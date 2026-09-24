import { resolveDefaultShell } from "$lib/core/default-shell";
import { writePty } from "$lib/core/pty-client";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { waitForConsole } from "$lib/ai-mode/restore-session";
import type { ToolId } from "$lib/ai-mode/pattern-table";

/**
 * Opens a console in `cwd` and starts the agent in it, the same way a
 * recent session is reopened: the default shell, with the command typed in.
 * Typed rather than run as the tab's own process, so leaving the agent drops
 * you back at a prompt in that folder instead of closing the tab, and so
 * what ran is right there on screen.
 */
export async function launchAgent(tool: ToolId, tileId: string, cwd: string | undefined): Promise<void> {
  const shell = await resolveDefaultShell();
  const tabId = workspaceStore.addTab(tileId, shell.path, shell.args, shell.title, cwd);
  if (!tabId) return;
  const ptyId = await waitForConsole(tabId);
  if (ptyId) await writePty(ptyId, `${tool}\r`);
}
