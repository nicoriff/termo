#[tauri::command]
pub fn minimize_window(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn toggle_maximize(window: tauri::Window) -> Result<(), String> {
    let maximized = window.is_maximized().map_err(|err| err.to_string())?;
    if maximized {
        window.unmaximize().map_err(|err| err.to_string())
    } else {
        window.maximize().map_err(|err| err.to_string())
    }
}

#[tauri::command]
pub fn close_window(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn is_maximized(window: tauri::Window) -> Result<bool, String> {
    window.is_maximized().map_err(|err| err.to_string())
}
