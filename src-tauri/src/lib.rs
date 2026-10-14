mod agent_favorites;
mod agent_sessions;
mod ai_mode;
mod agents;
mod commands;
mod companion;
mod config;
mod copilot_quota;
mod git;
mod harness;
mod i18n;
mod keep_awake;
mod layouts;
mod marketplace;
mod migration;
mod pty;
mod state;
mod terminal_windows;

use state::AppState;
use tauri::Manager;

/// Handles the two ways agents invoke this executable instead of launching the
/// app — `termo --agent-hook <tool> <event>` (pty/agent_hooks.rs) and
/// `termo --statusline` (pty/status_line.rs) — returning true when it did.
pub fn run_cli_mode_from_args() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag, tool, event, ..] if flag == pty::agent_hooks::HOOK_FLAG => {
            pty::agent_hooks::run_hook_cli(tool, event);
            true
        }
        [flag, agent, event, ..] if flag == agents::git_hooks::RUN_FLAG => {
            agents::run_agent_cli(agent, event);
            true
        }
        [flag, tool, rest @ ..]
            if flag == pty::status_line::STATUS_LINE_FLAG && tool == pty::copilot_status::COPILOT_ARG =>
        {
            pty::copilot_status::run_cli(rest.first().map(String::as_str));
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
    // Before anything reads the config folders (see migration.rs).
    migration::migrate_from_termini();
    let builder = tauri::Builder::default();
    // One Termo process at a time: opening it again opens one more window of
    // the running one instead (see terminal_windows.rs). Two processes would
    // each run the agents over the same data — both taking from one queue,
    // each blind to the other's runs. First, as the plugin asks. Left out of
    // debug builds, so a dev build can run beside the installed one.
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        // Called on the main thread, where building a window deadlocks on
        // Windows.
        let app = app.clone();
        std::thread::spawn(move || {
            let _ = terminal_windows::open_new(&app);
        });
    }));
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::default())
        .on_page_load(commands::window_commands::on_page_load)
        // Any console window closing takes its consoles along, and the last
        // one the app: the preloaded Preferences window only hides on close
        // and would otherwise keep it running.
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) && terminal_windows::is_terminal(window.label()) {
                terminal_windows::on_destroyed(window.app_handle(), window.label());
            }
        })
        .setup(|app| {
            if let Some(window) = app.get_webview_window(terminal_windows::MAIN) {
                commands::window_shape::attach(&window);
            }
            pty::agent_hooks::start_watcher(app.handle().clone());
            pty::status_line::start_watcher(app.handle().clone());
            // Before any run can start: files a run killed with Termo left.
            agents::runner::remove_leftovers(app.handle());
            // Consoles' token files from before: their tokens died with it.
            if let Ok(root) = agents::store::root(app.handle()) {
                agents::console_bridge::remove_leftovers(&root);
            }
            agents::orchestration::executor::recover(app.handle());
            // The agents, their local server and refreshing the hooks: only
            // with Modo IA on (see ai_mode.rs).
            let loaded = config::store::load(app.handle());
            // Before anything looks for an agent's CLI.
            marketplace::agent_install::apply(&loaded.agent_install_dirs);
            let ai_mode_on = loaded.ai_mode_enabled;
            ai_mode::init(app.handle(), &app.state::<AppState>().agents, ai_mode_on);
            // Every console's CLI settings point at this Termo (its port and
            // token), like the hooks do.
            agents::console_bridge::set_global(ai_mode_on && loaded.agents_from_any_console);
            if ai_mode_on && loaded.agents_from_any_console {
                if let Ok(root) = agents::store::root(app.handle()) {
                    std::thread::spawn(move || {
                        agents::console_bridge::register_global(&root, true);
                    });
                }
            }
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
            commands::pty_commands::windows_build,
            commands::folder_commands::list_folder_roots,
            commands::folder_commands::list_folders,
            commands::folder_commands::create_folder,
            commands::explorer_commands::explorer_root,
            commands::explorer_commands::list_entries,
            commands::explorer_commands::git_status,
            commands::explorer_commands::read_text_file,
            commands::explorer_commands::write_text_file,
            commands::git_commands::git_snapshot,
            commands::harness_commands::harness_load,
            commands::harness_commands::harness_preview,
            commands::harness_commands::harness_apply,
            commands::harness_commands::harness_detect_checks,
            commands::git_commands::git_blob,
            commands::git_commands::git_branches,
            commands::git_commands::git_graph,
            commands::git_commands::git_commit_detail,
            commands::git_commands::git_run,
            commands::git_commands::git_cancel,
            commands::git_commands::git_busy,
            commands::git_commands::git_pr,
            commands::git_commands::git_pr_update,
            commands::git_commands::git_pr_create,
            commands::git_commands::git_tools,
            commands::git_commands::git_review_start,
            commands::git_commands::git_review_cancel,
            commands::git_commands::git_review_busy,
            commands::git_commands::git_review_load,
            commands::git_commands::git_review_dismiss,
            commands::git_commands::git_review_clear,
            commands::git_commands::git_review_set_note,
            commands::git_commands::git_review_comment,
            commands::git_commands::git_review_publish,
            commands::git_commands::git_review_fix,
            commands::window_commands::minimize_window,
            commands::window_commands::toggle_maximize,
            commands::window_commands::close_window,
            commands::window_commands::is_maximized,
            commands::window_commands::open_preferences_window,
            commands::window_commands::preferences_ready,
            commands::window_commands::open_new_window,
            commands::window_commands::is_primary_window,
            commands::window_commands::report_pending,
            commands::notification_commands::notify,
            commands::window_commands::set_ui_language,
            commands::window_commands::set_background_blur,
            commands::window_commands::decoration_layout,
            commands::agent_hook_commands::agent_hooks_status,
            commands::agent_hook_commands::install_agent_hooks,
            commands::agent_hook_commands::uninstall_agent_hooks,
            commands::agent_session_commands::list_agent_sessions,
            commands::agent_session_commands::forget_agent_session,
            commands::agent_session_commands::list_agent_favorites,
            commands::agent_session_commands::save_agent_favorite,
            commands::agent_session_commands::remove_agent_favorite,
            commands::window_commands::quit_app,
            commands::pty_commands::busy_consoles,
            commands::pty_commands::diagnostics_snapshot,
            commands::companion_commands::companion_session,
            commands::companion_commands::companion_recent,
            commands::companion_commands::companion_overview,
            commands::companion_commands::companion_live,
            commands::companion_commands::companion_advise,
            commands::companion_commands::companion_quick_ask,
            commands::companion_commands::companion_last_exchange,
            commands::pty_commands::pty_kill_tree,
            commands::layout_commands::list_layouts,
            commands::layout_commands::save_layout,
            commands::layout_commands::delete_layout,
            commands::agent_commands::agents_list,
            commands::agent_commands::agents_save,
            commands::agent_commands::agents_delete,
            commands::agent_commands::agent_run,
            commands::agent_commands::agent_stop,
            commands::agent_commands::agents_running,
            commands::agent_commands::agent_runs,
            commands::agent_commands::agent_run_log,
            commands::agent_commands::agent_questions,
            commands::agent_commands::agent_answer_question,
            commands::agent_commands::agent_draft,
            commands::agent_commands::agent_mcp_server,
            commands::agent_commands::agent_forget_secrets,
            commands::agent_commands::agents_next_runs,
            commands::agent_commands::agent_github_preview,
            commands::agent_commands::agent_github_write,
            commands::agent_commands::agent_custom_mcp,
            commands::agent_commands::mcp_downloads,
            commands::agent_commands::agents_export,
            commands::agent_commands::agents_import,
            commands::agent_commands::agent_missing_secrets,
            commands::agent_commands::agent_set_secret,
            commands::agent_commands::agents_webhook,
            commands::agent_commands::agent_conversation,
            commands::agent_commands::agent_models,
            commands::keep_awake_commands::keep_awake_state,
            commands::keep_awake_commands::keep_awake_set,
            commands::agent_commands::agents_board,
            commands::agent_commands::agents_board_remove,
            commands::project_commands::projects_list,
            commands::project_commands::project_save,
            commands::project_commands::project_delete,
            commands::project_commands::agent_move,
            commands::project_commands::project_set_enabled,
            commands::project_commands::project_run,
            commands::project_commands::project_stop,
            commands::project_commands::project_stats,
            commands::project_commands::project_runs,
            commands::project_commands::project_export,
            commands::project_commands::project_import,
            commands::orchestration_commands::orchestration_catalog,
            commands::orchestration_commands::orchestration_list,
            commands::orchestration_commands::orchestration_plan,
            commands::orchestration_commands::orchestration_approve,
            commands::orchestration_commands::orchestration_stop,
            commands::orchestration_commands::orchestration_edit_plan,
            commands::orchestration_commands::orchestration_revise,
            commands::orchestration_commands::orchestration_report,
            commands::orchestration_commands::orchestration_clarify,
            commands::orchestration_commands::orchestration_answer,
            commands::agent_commands::agents_console_global,
            commands::agent_commands::agents_console_rotate,
            commands::orchestration_commands::orchestration_resume,
            commands::orchestration_commands::orchestration_resolve_revision,
            commands::orchestration_commands::orchestration_delete,
            commands::project_commands::project_draft,
            commands::project_commands::project_create,
            commands::project_commands::project_improve,
            commands::project_commands::project_versions,
            commands::project_commands::project_restore,
            commands::marketplace_commands::marketplace_status,
            commands::marketplace_commands::agent_cli_install_info,
            commands::marketplace_commands::install_agent_cli,
            commands::marketplace_commands::cancel_agent_cli_install,
            commands::marketplace_commands::node_version,
            commands::marketplace_commands::list_catalog,
            commands::marketplace_commands::install_item,
            commands::marketplace_commands::uninstall_item,
            commands::marketplace_commands::invalidate_catalog,
            commands::marketplace_commands::search_mcp_registry,
            commands::marketplace_commands::search_mcp,
            commands::marketplace_commands::mcp_detail,
            commands::marketplace_commands::marketplace_sources,
            commands::marketplace_commands::marketplace_set_key,
            commands::marketplace_commands::marketplace_clear_key,
            commands::marketplace_commands::marketplace_test_key,
            commands::marketplace_commands::install_mcp_item,
            commands::marketplace_commands::uninstall_mcp_item,
            commands::agent_hook_commands::status_line_installed,
            commands::agent_hook_commands::install_status_line,
            commands::agent_hook_commands::uninstall_status_line,
            commands::agent_hook_commands::copilot_status_line_installed,
            commands::agent_hook_commands::install_copilot_status_line,
            commands::agent_hook_commands::uninstall_copilot_status_line,
            copilot_quota::copilot_plan_usage,
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
