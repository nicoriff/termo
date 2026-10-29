use crate::config::{store, AppConfig};
use serde::Serialize;
use tauri::Emitter;

/// Off the main thread: every window asks for it first thing, while the main
/// thread is still busy with that window's start.
#[tauri::command]
pub async fn get_config(app: tauri::AppHandle) -> AppConfig {
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
    crate::marketplace::agent_install::apply(&config.agent_install_dirs);
    // Modo IA switched in Preferences: the agents and their server follow at once.
    crate::agents::console_orchestration::set_enabled(config.orchestrations_from_consoles);
    crate::ai_mode::set(&app, config.ai_mode_enabled);
    // Where the agents run in their own process, it follows too.
    crate::agents::host::client::config_saved(&app, &config);
    if config.background_agents {
        crate::agents::host::login::sync(&config);
    }
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
