import { resolveDefaultShell } from "$lib/core/default-shell";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import type { ToolId } from "$lib/ai-mode/pattern-table";

/**
 * Opens a console in `cwd` running the agent as its own process — no shell
 * starting first and no command typed in (see SessionTab.agent). The default
 * shell is what the tab falls back to: leaving the agent drops you at a prompt
 * in that folder instead of closing the tab.
 */
export async function launchAgent(tool: ToolId, tileId: string, cwd: string | undefined): Promise<void> {
  const shell = await resolveDefaultShell();
  workspaceStore.addTab(tileId, shell.path, shell.args, shell.title, cwd, { tool });
}
