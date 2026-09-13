use crate::config::{store, AppConfig};
use serde::Serialize;
use tauri::Emitter;

#[tauri::command]
pub fn get_config(app: tauri::AppHandle) -> AppConfig {
    store::load(&app)
}

#[derive(Serialize, Clone)]
struct ConfigUpdated<'a> {
    source: &'a str,
    config: &'a AppConfig,
}

#[tauri::command]
pub fn save_config(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    config: AppConfig,
) -> Result<(), String> {
    store::save(&app, &config).map_err(|err| err.to_string())?;
    // Lets every other open window (e.g. Preferences vs. the main window)
    // pick up the change live. `source` lets the saving window drop its own
    // echo: re-applying it can race a newer local edit and revert it.
    let _ = app.emit(
        "config-updated",
        ConfigUpdated {
            source: window.label(),
            config: &config,
        },
    );
    Ok(())
}
