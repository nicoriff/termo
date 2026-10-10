import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ToolId } from "$lib/ai-mode/pattern-table";

/** What installing an agent's CLI would run (see
 * src-tauri/src/marketplace/agent_install.rs). */
export interface AgentInstallInfo {
  /** npm itself, or null when Node.js isn't installed. */
  npm: string | null;
  /** npm's global folder: where it goes unless another is chosen. */
  defaultDir: string | null;
  package: string;
}

export type AgentInstallStage = "checking" | "installing" | "verifying";

export interface AgentInstallProgress {
  tool: ToolId;
  stage: AgentInstallStage;
  /** A line npm printed, when this is one. */
  line: string | null;
}

export function agentInstallInfo(tool: ToolId): Promise<AgentInstallInfo> {
  return invoke("agent_cli_install_info", { tool });
}

/** Installs it with npm, into `dir` when that isn't npm's own folder.
 * Resolves with where the CLI ended up; rejects with why it didn't. */
export function installAgentCli(tool: ToolId, dir: string | null): Promise<{ path: string }> {
  return invoke("install_agent_cli", { tool, dir });
}

export function cancelAgentCliInstall(): Promise<void> {
  return invoke("cancel_agent_cli_install");
}

export function onAgentInstallProgress(handler: (progress: AgentInstallProgress) => void): Promise<UnlistenFn> {
  return listen<AgentInstallProgress>("agent-cli-install", (event) => handler(event.payload));
}

/** The command line as it's run, to show before and to copy after a failure. */
export function installCommand(packageName: string, dir: string | null): string {
  return dir ? `npm install -g --prefix "${dir}" ${packageName}` : `npm install -g ${packageName}`;
}
