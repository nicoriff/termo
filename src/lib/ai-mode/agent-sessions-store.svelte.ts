import {
  forgetAgentSession,
  listAgentSessions,
  type AgentSession,
} from "$lib/core/agent-sessions-client";

/** Recent agent conversations, newest first, as kept by the backend. Fed by the
 * `agent-sessions` event and by one load at startup (see ai-mode/attention.ts). */
class AgentSessionsStore {
  sessions = $state<AgentSession[]>([]);

  set(sessions: AgentSession[]): void {
    this.sessions = sessions;
  }

  async load(): Promise<void> {
    try {
      this.sessions = await listAgentSessions();
    } catch {
      // Nothing recorded yet, or the file is unreadable — an empty list is fine.
    }
  }

  async forget(agentSessionId: string): Promise<void> {
    this.sessions = await forgetAgentSession(agentSessionId);
  }
}

export const agentSessionsStore = new AgentSessionsStore();
