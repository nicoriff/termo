import { invoke } from "@tauri-apps/api/core";
import type { ToolId } from "$lib/ai-mode/pattern-table";

/** An agent conversation the user marked as a favorite (see
 * src-tauri/src/agent_favorites.rs). */
export interface AgentFavorite {
  tool: ToolId;
  /** The agent's own session id — what its resume command takes. */
  agentSessionId: string;
  /** The user's name for it; null shows the folder instead. */
  name: string | null;
  cwd: string | null;
  projectDir: string | null;
  model: string | null;
  /** Unix seconds. */
  addedAt: number;
}

export function listAgentFavorites(): Promise<AgentFavorite[]> {
  return invoke("list_agent_favorites");
}

/** Adds it, or renames the favorite for that session. Returns them all. */
export function saveAgentFavorite(favorite: AgentFavorite): Promise<AgentFavorite[]> {
  return invoke("save_agent_favorite", { favorite });
}

export function removeAgentFavorite(id: string): Promise<AgentFavorite[]> {
  return invoke("remove_agent_favorite", { id });
}
