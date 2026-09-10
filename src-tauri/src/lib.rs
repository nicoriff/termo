mod commands;
mod config;
mod pty;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                commands::window_shape::remove_nonclient_frame(&window);
                commands::window_shape::apply(&window);
                let shape_window = window.clone();
                window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Resized(_)) {
                        commands::window_shape::apply(&shape_window);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pty_commands::pty_spawn,
            commands::pty_commands::pty_write,
            commands::pty_commands::pty_resize,
            commands::pty_commands::pty_kill,
            commands::pty_commands::list_shells,
            commands::window_commands::minimize_window,
            commands::window_commands::toggle_maximize,
            commands::window_commands::close_window,
            commands::window_commands::is_maximized,
            commands::config_commands::get_config,
            commands::config_commands::save_config,
            commands::font_commands::list_google_monospace_fonts,
            commands::font_commands::install_google_font,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
