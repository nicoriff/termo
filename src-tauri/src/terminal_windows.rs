//! Several Termo windows in one process. Each is a full console window —
//! its own tabs and panes — while the rest is shared: one agents engine, one
//! Preferences window, one config. The first is `main` (tauri.conf.json);
//! the ones opened later are `main-2`, `main-3`… built from the same config.
//!
//! What belongs to a window is its consoles (see pty::manager's owners):
//! closing or reloading it ends them, and nobody else's. The app ends with
//! the last console window; Preferences alone doesn't keep it running.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use tauri::Manager;

use crate::state::AppState;

pub const MAIN: &str = "main";

/// A console window's label, as opposed to Preferences.
pub fn is_terminal(label: &str) -> bool {
    label == MAIN || label.starts_with("main-")
}

/// Its place among the windows: `main` first, then in the order they were
/// opened.
fn order(label: &str) -> u32 {
    label.strip_prefix("main-").and_then(|n| n.parse().ok()).unwrap_or(1)
}

/// The console windows open now, `main` first.
pub fn labels(app: &tauri::AppHandle) -> Vec<String> {
    let mut labels: Vec<String> = app.webview_windows().into_keys().filter(|label| is_terminal(label)).collect();
    labels.sort_by_key(|label| order(label));
    labels
}

/// The one window that speaks for all of them where only one should — the
/// notifications for background agent runs, say: the first one still open.
pub fn is_primary(app: &tauri::AppHandle, label: &str) -> bool {
    labels(app).first().is_some_and(|first| first == label)
}

/// Opens one more console window, built like `main` and in front of the
/// others. Not on the main thread: building a webview window there
/// deadlocks on Windows (see window_commands::open_preferences_window).
pub fn open_new(app: &tauri::AppHandle) -> Result<(), String> {
    static NEXT: AtomicU32 = AtomicU32::new(2);
    let mut config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN)
        .cloned()
        .ok_or_else(|| "no main window config".to_string())?;
    // Labels aren't reused within a run: a closed window's late events can't
    // land on a new one by mistake.
    config.label = loop {
        let label = format!("main-{}", NEXT.fetch_add(1, Ordering::SeqCst));
        if app.get_webview_window(&label).is_none() {
            break label;
        }
    };
    let window = tauri::WebviewWindowBuilder::from_config(app, &config)
        .map_err(|err| err.to_string())?
        .build()
        .map_err(|err| err.to_string())?;
    let shaped = window.clone();
    window
        .run_on_main_thread(move || crate::commands::window_shape::attach(&shaped))
        .map_err(|err| err.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

/// A console window is gone: its consoles end with it (the ones the
/// sessions daemon runs keep running), its count leaves the badge, and with
/// the last one goes the app.
pub fn on_destroyed(app: &tauri::AppHandle, label: &str) {
    app.state::<AppState>().pty.release_owned_by(label);
    set_pending(app, label, 0);
    if labels(app).iter().all(|other| other == label) {
        app.exit(0);
    }
}

/// Sessions waiting on the user, per window (Modo IA's count, see
/// MainWindow.svelte). The app icon's badge is the app's, not a window's, so
/// it shows the sum: each window setting its own would overwrite the others.
pub fn set_pending(app: &tauri::AppHandle, label: &str, count: u32) {
    static PENDING: Mutex<Option<HashMap<String, u32>>> = Mutex::new(None);
    let total: u32 = {
        let mut pending = PENDING.lock().unwrap();
        let map = pending.get_or_insert_with(HashMap::new);
        if count == 0 {
            map.remove(label);
        } else {
            map.insert(label.to_string(), count);
        }
        map.values().sum()
    };
    if let Some(window) = labels(app).first().and_then(|first| app.get_webview_window(first)) {
        let _ = window.set_badge_count((total > 0).then_some(i64::from(total)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_windows_are_told_from_preferences() {
        assert!(is_terminal("main"));
        assert!(is_terminal("main-2"));
        assert!(!is_terminal("preferences"));
        assert!(!is_terminal("mainland"));
    }

    #[test]
    fn main_comes_first_then_the_order_they_opened_in() {
        let mut labels = vec!["main-10", "main-3", "main", "main-2"];
        labels.sort_by_key(|label| order(label));
        assert_eq!(labels, ["main", "main-2", "main-3", "main-10"]);
    }
}
