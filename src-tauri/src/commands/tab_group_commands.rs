use crate::tab_groups;

#[tauri::command]
pub fn list_tab_groups(app: tauri::AppHandle) -> Vec<serde_json::Value> {
    tab_groups::list(&app)
}

/// Returns the list after saving, so the caller can refresh without a second call.
#[tauri::command]
pub fn save_tab_group(app: tauri::AppHandle, group: serde_json::Value) -> Result<Vec<serde_json::Value>, String> {
    tab_groups::save(&app, group)
}

#[tauri::command]
pub fn delete_tab_group(app: tauri::AppHandle, id: String) -> Result<Vec<serde_json::Value>, String> {
    tab_groups::remove(&app, &id)
}
