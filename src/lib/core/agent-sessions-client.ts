import { invoke } from "@tauri-apps/api/core";
import type { ToolId } from "$lib/ai-mode/pattern-table";

/** A past agent conversation Modo IA can reopen (see
 * src-tauri/src/agent_sessions.rs). */
export interface AgentSession {
  tool: ToolId;
  /** The agent's own session id — what its resume command takes. */
  agentSessionId: string;
  cwd: string | null;
  projectDir: string | null;
  model: string | null;
  /** Unix seconds. */
  lastSeen: number;
}

export function listAgentSessions(): Promise<AgentSession[]> {
  return invoke("list_agent_sessions");
}

/** Returns the remaining sessions. */
export function forgetAgentSession(id: string): Promise<AgentSession[]> {
  return invoke("forget_agent_session", { id });
}
