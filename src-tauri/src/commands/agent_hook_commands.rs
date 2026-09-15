use serde::Serialize;

use crate::pty::agent_hooks::{self, AgentTool};

#[derive(Serialize)]
pub struct AgentHooksStatus {
    claude: bool,
    codex: bool,
}

#[tauri::command]
pub fn agent_hooks_status() -> AgentHooksStatus {
    AgentHooksStatus {
        claude: agent_hooks::is_installed(AgentTool::Claude),
        codex: agent_hooks::is_installed(AgentTool::Codex),
    }
}

#[tauri::command]
pub fn install_agent_hooks(tool: AgentTool) -> Result<(), String> {
    agent_hooks::install(tool).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn uninstall_agent_hooks(tool: AgentTool) -> Result<(), String> {
    agent_hooks::uninstall(tool).map_err(|err| err.to_string())
}
