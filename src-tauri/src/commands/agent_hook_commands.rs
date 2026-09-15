use serde::Serialize;

use crate::pty::agent_hooks::{self, AgentTool};

#[derive(Serialize)]
pub struct AgentHooksStatus {
    /// The hook script is POSIX sh, so the integration is macOS/Linux only.
    supported: bool,
    claude: bool,
    codex: bool,
}

#[tauri::command]
pub fn agent_hooks_status() -> AgentHooksStatus {
    AgentHooksStatus {
        supported: cfg!(unix),
        claude: agent_hooks::is_installed(AgentTool::Claude),
        codex: agent_hooks::is_installed(AgentTool::Codex),
    }
}

#[tauri::command]
pub fn install_agent_hooks(app: tauri::AppHandle, tool: AgentTool) -> Result<(), String> {
    if !cfg!(unix) {
        return Err(crate::i18n::tr(crate::i18n::Msg::HooksUnsupportedPlatform));
    }
    agent_hooks::install(&app, tool).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn uninstall_agent_hooks(tool: AgentTool) -> Result<(), String> {
    agent_hooks::uninstall(tool).map_err(|err| err.to_string())
}
