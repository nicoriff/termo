use crate::browser::{self, Browser};
use crate::config::store;

/// Opens a link with the browser chosen in Preferences, or the system's.
/// Read from the saved config each time, so a change applies right away,
/// in every window.
#[tauri::command]
pub async fn open_link(app: tauri::AppHandle, url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let chosen = store::load(&app).link_browser;
        browser::open(&url, chosen.as_deref())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// The browsers installed on this computer, for Preferences to offer.
#[tauri::command]
pub async fn list_browsers() -> Result<Vec<Browser>, String> {
    tauri::async_runtime::spawn_blocking(browser::installed).await.map_err(|err| err.to_string())
}
