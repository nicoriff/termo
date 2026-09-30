use crate::config::{store, AppConfig};
use crate::state::AppState;
use serde::Serialize;
use tauri::{Emitter, Manager};

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
    // Modo IA switched in Preferences: the agents and their server follow at once.
    crate::ai_mode::set(&app, &app.state::<AppState>().agents, config.ai_mode_enabled);
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
