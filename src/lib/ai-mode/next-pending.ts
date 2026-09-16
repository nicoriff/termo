import { collectSessions } from "../layout/split-tree";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { aiStatusStore } from "./ai-status-store.svelte";

/** Focuses the agent session that has been waiting on the user the longest
 * (awaiting first, then errored); repeated calls walk through them. Shared by
 * the Modo IA panel's shortcut and the command palette. */
export function focusNextPending(): void {
  const alive = new Set(
    (workspaceStore.root ? collectSessions(workspaceStore.root) : []).map((loc) => loc.session.id),
  );
  const pending = [...aiStatusStore.entriesWith("awaiting"), ...aiStatusStore.entriesWith("error")]
    .map((entry) => entry.sessionId)
    .filter((id) => alive.has(id));
  if (pending.length === 0) return;
  const current = pending.indexOf(workspaceStore.focusedSessionId ?? "");
  workspaceStore.focusSession(pending[(current + 1) % pending.length]);
}
