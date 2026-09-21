import { listen } from "@tauri-apps/api/event";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { appConfigStore } from "../core/app-config.svelte";
import { t } from "../i18n/i18n.svelte";
import { collectSessions } from "../layout/split-tree";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { getTabId } from "../terminal/pty-registry";
import { tabTitleStore } from "../terminal/tab-title-store.svelte";
import { aiStatusStore, type AiSessionEntry } from "./ai-status-store.svelte";
import { agentStatusStore, type AgentStatus } from "./agent-status-store.svelte";
import { TOOL_DISPLAY_NAMES } from "./tool-names";
import { agentSessionsStore } from "./agent-sessions-store.svelte";
import { tabAgentSessions } from "./tab-sessions.svelte";
import type { AgentSession } from "$lib/core/agent-sessions-client";
import type { AiSessionStatus } from "./classifier";
import type { ToolId } from "./pattern-table";

interface AgentEventPayload {
  /** PTY id, as set in the console's TERMINI_SESSION_ID. */
  sessionId: string;
  tool: ToolId;
  status: AiSessionStatus | null;
  /** The agent's own conversation id, when the hook reported one. */
  agentSessionId?: string | null;
  agentCwd?: string | null;
}

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
      return appConfigStore.aiNotifyAwaiting ? t("aiMode.notify.awaiting") : null;
    case "error":
      return appConfigStore.aiNotifyError ? t("aiMode.notify.error") : null;
    case "finished":
      // A turn that actually ran, not a session first detected as idle.
      return appConfigStore.aiNotifyFinished && prev?.status === "running" ? t("aiMode.notify.finished") : null;
    default:
      return null;
  }
}

async function maybeNotify(sessionId: string, entry: AiSessionEntry, prev: AiSessionEntry | undefined) {
  const text = notificationText(entry, prev);
  if (!text || !entry.tool) return;
  if (document.hasFocus() && workspaceStore.isSessionVisible(sessionId)) return;
  if (!(await canNotify())) return;
  sendNotification({ title: `${TOOL_DISPLAY_NAMES[entry.tool]}: ${text}`, body: tabTitle(sessionId) });
}

/** Feeds hook events into Modo IA and notifies on status transitions.
 * Returns a teardown function. */
export function startAgentAttention(): () => void {
  const stopNotifying = aiStatusStore.onStatusChange((sessionId, entry, prev) => {
    void maybeNotify(sessionId, entry, prev);
  });
  const unlisten = listen<AgentEventPayload>("agent-event", ({ payload }) => {
    const tabId = getTabId(payload.sessionId);
    if (!tabId) return;
    // Recorded whether or not Modo IA is on: it says which conversation this
    // console has open, which is what saving a layout with sessions needs.
    if (payload.agentSessionId) {
      tabAgentSessions.set(tabId, payload.tool, payload.agentSessionId, payload.agentCwd ?? undefined);
    }
    if (appConfigStore.aiModeEnabled) aiStatusStore.setFromHook(tabId, payload.tool, payload.status);
  });
  // Plan usage / context / cost, from Claude Code's status line or from
  // Codex's rollout. Independent of Modo IA being on: it's just information
  // about the console's session.
  const unlistenStatus = listen<
    {
      sessionId: string;
      tool?: ToolId | null;
      agentSessionId?: string | null;
      cwd?: string | null;
      projectDir?: string | null;
    } & AgentStatus
  >("agent-status", ({ payload }) => {
    // cwd/projectDir/agentSessionId/tool are about the conversation, not plan
    // usage, so they're routed to tabAgentSessions instead of the status store.
    const { sessionId, tool, agentSessionId, cwd, projectDir, ...status } = payload;
    const tabId = getTabId(sessionId);
    if (!tabId) return;
    agentStatusStore.set(tabId, status);
    // Snapshots written before Codex reported any usage carry no tool, and
    // the status line that wrote them is Claude's alone.
    if (agentSessionId) {
      tabAgentSessions.set(tabId, tool ?? "claude", agentSessionId, cwd ?? projectDir ?? undefined);
    }
  });
  // Past conversations Modo IA can reopen, recorded by both channels.
  void agentSessionsStore.load();
  const unlistenSessions = listen<AgentSession[]>("agent-sessions", ({ payload }) => {
    agentSessionsStore.set(payload);
  });
  return () => {
    stopNotifying();
    void unlisten.then((stop) => stop());
    void unlistenStatus.then((stop) => stop());
    void unlistenSessions.then((stop) => stop());
  };
}
