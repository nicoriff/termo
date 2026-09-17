use crate::agent_sessions::{self, AgentSession};

#[tauri::command]
pub fn list_agent_sessions(app: tauri::AppHandle) -> Vec<AgentSession> {
    agent_sessions::list(&app)
}

#[tauri::command]
pub fn forget_agent_session(app: tauri::AppHandle, id: String) -> Vec<AgentSession> {
    agent_sessions::forget(&app, &id)
}
