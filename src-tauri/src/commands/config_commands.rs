use crate::config::{store, AppConfig};

#[tauri::command]
pub fn get_config(app: tauri::AppHandle) -> AppConfig {
    store::load(&app)
}

#[tauri::command]
pub fn save_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    store::save(&app, &config).map_err(|err| err.to_string())
}
