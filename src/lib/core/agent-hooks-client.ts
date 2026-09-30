import { invoke } from "@tauri-apps/api/core";

export type AgentTool = "claude" | "codex" | "gemini";

export interface AgentHooksStatus {
  claude: boolean;
  codex: boolean;
  gemini: boolean;
}

export function getAgentHooksStatus(): Promise<AgentHooksStatus> {
  return invoke("agent_hooks_status");
}

export function installAgentHooks(tool: AgentTool): Promise<void> {
  return invoke("install_agent_hooks", { tool });
}

export function uninstallAgentHooks(tool: AgentTool): Promise<void> {
  return invoke("uninstall_agent_hooks", { tool });
}
