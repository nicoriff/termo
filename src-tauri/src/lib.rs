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
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::default())
        .on_page_load(commands::window_commands::on_page_load)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                commands::window_shape::attach(&window);
                // The preloaded Preferences window only hides on close and
                // would otherwise keep the app running without a main window.
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Destroyed) {
                        handle.exit(0);
                    }
                });
            }
            pty::agent_hooks::start_watcher(app.handle().clone());
            // Keeps already-installed hooks running this build's script.
            let _ = pty::agent_hooks::write_script(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pty_commands::pty_spawn,
            commands::pty_commands::pty_write,
            commands::pty_commands::pty_resize,
            commands::pty_commands::pty_kill,
            commands::pty_commands::pty_memory,
            commands::pty_commands::list_shells,
            commands::window_commands::minimize_window,
            commands::window_commands::toggle_maximize,
            commands::window_commands::close_window,
            commands::window_commands::is_maximized,
            commands::window_commands::open_preferences_window,
            commands::window_commands::preferences_ready,
            commands::window_commands::set_background_blur,
            commands::agent_hook_commands::agent_hooks_status,
            commands::agent_hook_commands::install_agent_hooks,
            commands::agent_hook_commands::uninstall_agent_hooks,
            commands::config_commands::get_config,
            commands::config_commands::save_config,
            commands::font_commands::list_google_monospace_fonts,
            commands::font_commands::install_google_font,
            commands::oh_my_posh_commands::oh_my_posh_status,
            commands::oh_my_posh_commands::install_oh_my_posh,
            commands::oh_my_posh_commands::install_oh_my_posh_theme,
            commands::oh_my_posh_commands::install_nerd_font,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
