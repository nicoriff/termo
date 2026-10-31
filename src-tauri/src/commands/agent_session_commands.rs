use crate::agent_favorites::{self, AgentFavorite};
use crate::agent_sessions::{self, AgentSession};

/// Off the main thread, as the next: they read files, and every window asks
/// for them as it starts.
#[tauri::command]
pub async fn list_agent_sessions(app: tauri::AppHandle) -> Vec<AgentSession> {
    agent_sessions::list_resumable(&app)
}

#[tauri::command]
pub fn forget_agent_session(app: tauri::AppHandle, id: String) -> Vec<AgentSession> {
    agent_sessions::forget(&app, &id)
}

#[tauri::command]
pub async fn list_agent_favorites(app: tauri::AppHandle) -> Vec<AgentFavorite> {
    agent_favorites::list(&app)
}

/// Adds the favorite, or renames the one for that session.
#[tauri::command]
pub fn save_agent_favorite(app: tauri::AppHandle, favorite: AgentFavorite) -> Result<Vec<AgentFavorite>, String> {
    agent_favorites::save(&app, favorite)
}

#[tauri::command]
pub fn remove_agent_favorite(app: tauri::AppHandle, id: String) -> Result<Vec<AgentFavorite>, String> {
    agent_favorites::remove(&app, &id)
}
