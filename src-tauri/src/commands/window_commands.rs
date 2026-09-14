use tauri::Manager;

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

/// Opens the Preferences window, or focuses it if already open — this app
/// is a single-page SPA (adapter-static, `ssr = false`), so rather than a
/// second SvelteKit route (which adapter-static's prerendering doesn't
/// cleanly produce as a standalone second HTML entry point), this points at
/// the app root with a `#preferences` fragment; the frontend branches on
/// `location.hash` to decide what to render. Note: the path must be `/`, not
/// `index.html` — in `tauri dev` this resolves against the Vite dev server,
/// which serves `/` but has no literal `/index.html` route (that file only
/// exists post-build), so `index.html#preferences` 404s in dev.
#[tauri::command]
pub fn open_preferences_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("preferences") {
        return window.set_focus().map_err(|err| err.to_string());
    }

    let builder = tauri::WebviewWindowBuilder::new(
        &app,
        "preferences",
        tauri::WebviewUrl::App("/#preferences".into()),
    )
    .title("Preferencias")
    .inner_size(760.0, 480.0)
    .min_inner_size(640.0, 420.0)
    .resizable(true)
    .maximizable(false)
    .transparent(true);

    // macOS gets the native window controls drawn over the web title bar,
    // matching the main window's tauri.macos.conf.json (keep the traffic
    // light position in sync with it); elsewhere the web title bar draws
    // its own controls on an undecorated window.
    #[cfg(target_os = "macos")]
    let builder = builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(14.0, 16.0));
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);

    let window = builder.build().map_err(|err| err.to_string())?;

    crate::commands::window_shape::attach(&window);
    Ok(())
}
