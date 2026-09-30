use serde::Serialize;

use crate::pty::agent_hooks::{self, AgentTool};
use crate::pty::status_line;

#[derive(Serialize)]
pub struct AgentHooksStatus {
    claude: bool,
    codex: bool,
    gemini: bool,
}

#[tauri::command]
pub fn agent_hooks_status() -> AgentHooksStatus {
    AgentHooksStatus {
        claude: agent_hooks::is_installed(AgentTool::Claude),
        codex: agent_hooks::is_installed(AgentTool::Codex),
        gemini: agent_hooks::is_installed(AgentTool::Gemini),
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

#[tauri::command]
pub fn status_line_installed() -> bool {
    status_line::is_installed()
}

#[tauri::command]
pub fn install_status_line(app: tauri::AppHandle) -> Result<(), String> {
    status_line::install(&app).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn uninstall_status_line(app: tauri::AppHandle) -> Result<(), String> {
    status_line::uninstall(&app).map_err(|err| err.to_string())
}
