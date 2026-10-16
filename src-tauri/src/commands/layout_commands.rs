use crate::layouts::{self, SavedLayout};

#[tauri::command]
pub fn list_layouts(app: tauri::AppHandle) -> Vec<SavedLayout> {
    layouts::list(&app)
}

/// Returns the list after saving, so the caller can refresh without a second call.
#[tauri::command]
pub fn save_layout(app: tauri::AppHandle, name: String, root: serde_json::Value) -> Vec<SavedLayout> {
    layouts::save(&app, &name, root)
}

#[tauri::command]
pub fn delete_layout(app: tauri::AppHandle, name: String) -> Vec<SavedLayout> {
    layouts::delete(&app, &name)
}

/// What the asking window has open, for consoles that outlive it (see
/// layouts::save_window_session). `None` forgets it.
#[tauri::command]
pub fn save_window_session(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    root: Option<serde_json::Value>,
) -> Result<(), String> {
    layouts::save_window_session(&app, window.label(), root).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn window_session(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Option<serde_json::Value> {
    layouts::window_session(&app, window.label())
}
