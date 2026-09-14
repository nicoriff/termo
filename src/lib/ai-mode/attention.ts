import { listen } from "@tauri-apps/api/event";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { appConfigStore } from "../core/app-config.svelte";
import { collectSessions } from "../layout/split-tree";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { getTabId } from "../terminal/pty-registry";
import { tabTitleStore } from "../terminal/tab-title-store.svelte";
import { aiStatusStore, type AiSessionEntry } from "./ai-status-store.svelte";
import type { AiSessionStatus } from "./classifier";
import type { ToolId } from "./pattern-table";

interface AgentEventPayload {
  /** PTY id, as set in the console's TERMINI_SESSION_ID. */
  sessionId: string;
  tool: ToolId;
  status: AiSessionStatus | null;
}

const TOOL_NAMES: Record<ToolId, string> = { claude: "Claude Code", codex: "Codex" };

let permission: Promise<boolean> | null = null;

function canNotify(): Promise<boolean> {
  permission ??= (async () => (await isPermissionGranted()) || (await requestPermission()) === "granted")().catch(
    () => false,
  );
  return permission;
}

function tabTitle(sessionId: string): string {
  const root = workspaceStore.root;
  const session = root ? collectSessions(root).find((loc) => loc.session.id === sessionId)?.session : undefined;
  return session ? tabTitleStore.titleFor(session.id, session.title) : "";
}

function notificationText(entry: AiSessionEntry, prev: AiSessionEntry | undefined): string | null {
  switch (entry.status) {
    case "awaiting":
      return appConfigStore.aiNotifyAwaiting ? "Necesita tu respuesta" : null;
    case "error":
      return appConfigStore.aiNotifyError ? "Terminó con un error" : null;
    case "finished":
      // A turn that actually ran, not a session first detected as idle.
      return appConfigStore.aiNotifyFinished && prev?.status === "running" ? "Terminó" : null;
    default:
      return null;
  }
}

async function maybeNotify(sessionId: string, entry: AiSessionEntry, prev: AiSessionEntry | undefined) {
  const text = notificationText(entry, prev);
  if (!text || !entry.tool) return;
  if (document.hasFocus() && workspaceStore.isSessionVisible(sessionId)) return;
  if (!(await canNotify())) return;
  sendNotification({ title: `${TOOL_NAMES[entry.tool]}: ${text}`, body: tabTitle(sessionId) });
}

/** Feeds hook events into Modo IA and notifies on status transitions.
 * Returns a teardown function. */
export function startAgentAttention(): () => void {
  const stopNotifying = aiStatusStore.onStatusChange((sessionId, entry, prev) => {
    void maybeNotify(sessionId, entry, prev);
  });
  const unlisten = listen<AgentEventPayload>("agent-event", ({ payload }) => {
    if (!appConfigStore.aiModeEnabled) return;
    const tabId = getTabId(payload.sessionId);
    if (tabId) aiStatusStore.setFromHook(tabId, payload.tool, payload.status);
  });
  return () => {
    stopNotifying();
    void unlisten.then((stop) => stop());
  };
}
