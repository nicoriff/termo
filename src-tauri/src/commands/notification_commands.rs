//! Notifications that lead somewhere. The notification plugin shows one and
//! forgets it: on the desktop it has no word on a click, so clicking did
//! nothing. On Windows the toast is built here instead, with the click heard:
//! it brings forward the window that sent it and hands it back the `target`
//! it sent along (a tab, an agent's run, an orchestration — the window knows
//! what to do with it, see src/lib/app/notification-clicks.ts).

use tauri::{Emitter, Manager};

/// What a window hears when one of its notifications is clicked.
const OPENED: &str = "notification-opened";

/// The window `label` in front, told what was clicked. Gone by now,
/// there's nowhere to go.
pub fn open_target(app: &tauri::AppHandle, label: &str, target: serde_json::Value) {
    let Some(window) = app.get_webview_window(label) else { return };
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit_to(label, OPENED, target);
}

/// What the notification Termo was started from was about, if it was:
/// asked once by the main window when it's ready.
#[tauri::command]
pub fn notification_take_opened(state: tauri::State<'_, crate::state::AppState>) -> Option<serde_json::Value> {
    state.opened_by.lock().unwrap().take()
}

/// Shows a system notification on behalf of the asking window. Async, so the
/// toast isn't built on the main thread.
#[tauri::command]
pub async fn notify(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    title: String,
    body: String,
    target: serde_json::Value,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        toast::show(&app, window.label().to_string(), &title, &body, target)
    }
    // Elsewhere the click isn't heard yet: the plugin's own notification.
    #[cfg(not(windows))]
    {
        use tauri_plugin_notification::NotificationExt;
        let _ = (window, target);
        app.notification().builder().title(title).body(body).show().map_err(|err| err.to_string())
    }
}

#[cfg(windows)]
mod toast {
    use std::path::MAIN_SEPARATOR as SEP;

    use tauri_winrt_notification::{Duration, Toast};

    /// The toast the plugin would have shown (same id, texts and silence:
    /// the sounds are Termo's own), plus what a click does.
    pub fn show(
        app: &tauri::AppHandle,
        label: String,
        title: &str,
        body: &str,
        target: serde_json::Value,
    ) -> Result<(), String> {
        let clicked = app.clone();
        Toast::new(&app_id(app))
            .title(title)
            .text1("")
            .text2(body)
            .sound(None)
            .duration(Duration::Short)
            .on_activated(move |_| {
                super::open_target(&clicked, &label, target.clone());
                Ok(())
            })
            .show()
            .map_err(|err| err.to_string())
    }

    /// Termo's own id only when installed: a build run from `target` has no
    /// shortcut registering it, and Windows drops toasts from an unknown id.
    fn app_id(app: &tauri::AppHandle) -> String {
        let dir = tauri::utils::platform::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.display().to_string()))
            .unwrap_or_default();
        let from_target = dir.ends_with(&format!("{SEP}target{SEP}debug")) || dir.ends_with(&format!("{SEP}target{SEP}release"));
        if from_target {
            Toast::POWERSHELL_APP_ID.to_string()
        } else {
            app.config().identifier.clone()
        }
    }

}
