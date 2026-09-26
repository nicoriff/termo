import { uiState } from "$lib/core/ui-state.svelte";
import { collectTiles } from "$lib/layout/split-tree";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { tabCwd } from "$lib/terminal/tab-cwd.svelte";
import type { ToolId } from "$lib/ai-mode/pattern-table";
import type { LaunchTarget } from "./launch-target";

/**
 * Asks where to open `target` (see AgentLaunchDialog), offering the folder
 * the tile's active console is in right now. The tile defaults to the
 * focused one, which is where the shortcuts and the command palette put the
 * tab; the "+" menu passes its own.
 */
export function requestLaunch(target: LaunchTarget, tileId = workspaceStore.focusedTileId): void {
  const root = workspaceStore.root;
  if (!root || !tileId) return;
  const tile = collectTiles(root).find((candidate) => candidate.id === tileId);
  if (!tile) return;
  const active = tile.sessions.find((session) => session.id === tile.activeSessionId);
  const here = active ? (tabCwd.byTab[active.id] ?? active.cwd ?? null) : null;
  uiState.openAgentLaunch(target, tile.id, here);
}

export function requestAgentLaunch(tool: ToolId, tileId = workspaceStore.focusedTileId): void {
  requestLaunch({ kind: "agent", tool }, tileId);
}
