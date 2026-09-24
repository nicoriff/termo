mod agent_sessions;
mod commands;
mod config;
mod i18n;
mod layouts;
mod marketplace;
mod pty;
mod state;

use state::AppState;
use tauri::Manager;

/// Handles the two ways agents invoke this executable instead of launching the
/// app — `termini --agent-hook <tool> <event>` (pty/agent_hooks.rs) and
/// `termini --statusline` (pty/status_line.rs) — returning true when it did.
pub fn run_cli_mode_from_args() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag, tool, event, ..] if flag == pty::agent_hooks::HOOK_FLAG => {
            pty::agent_hooks::run_hook_cli(tool, event);
            true
        }
        [flag, ..] if flag == pty::status_line::STATUS_LINE_FLAG => {
            pty::status_line::run_status_line_cli();
            true
        }
        _ => false,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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
            pty::agent_hooks::refresh_installed(app.handle());
            pty::status_line::start_watcher(app.handle().clone());
            pty::status_line::refresh_installed(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pty_commands::pty_spawn,
            commands::pty_commands::pty_write,
            commands::pty_commands::pty_resize,
            commands::pty_commands::pty_prompt_redraw,
            commands::pty_commands::pty_kill,
            commands::pty_commands::pty_memory,
            commands::pty_commands::list_shells,
            commands::folder_commands::list_folder_roots,
            commands::folder_commands::list_folders,
            commands::window_commands::minimize_window,
            commands::window_commands::toggle_maximize,
            commands::window_commands::close_window,
            commands::window_commands::is_maximized,
            commands::window_commands::open_preferences_window,
            commands::window_commands::preferences_ready,
            commands::window_commands::set_ui_language,
            commands::window_commands::set_background_blur,
            commands::agent_hook_commands::agent_hooks_status,
            commands::agent_hook_commands::install_agent_hooks,
            commands::agent_hook_commands::uninstall_agent_hooks,
            commands::agent_session_commands::list_agent_sessions,
            commands::agent_session_commands::forget_agent_session,
            commands::window_commands::quit_app,
            commands::pty_commands::busy_consoles,
            commands::layout_commands::list_layouts,
            commands::layout_commands::save_layout,
            commands::layout_commands::delete_layout,
            commands::marketplace_commands::marketplace_status,
            commands::marketplace_commands::list_catalog,
            commands::marketplace_commands::install_item,
            commands::marketplace_commands::uninstall_item,
            commands::marketplace_commands::invalidate_catalog,
            commands::marketplace_commands::search_mcp_registry,
            commands::marketplace_commands::install_mcp_item,
            commands::marketplace_commands::uninstall_mcp_item,
            commands::agent_hook_commands::status_line_installed,
            commands::agent_hook_commands::install_status_line,
            commands::agent_hook_commands::uninstall_status_line,
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
