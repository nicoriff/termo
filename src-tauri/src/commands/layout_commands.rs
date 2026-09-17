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
