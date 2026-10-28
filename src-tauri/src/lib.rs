mod agent_favorites;
mod agent_sessions;
mod ai_mode;
mod agents;
mod browser;
mod commands;
mod companion;
mod config;
mod copilot_quota;
mod git;
mod harness;
mod i18n;
mod keep_awake;
mod layouts;
mod tab_groups;
mod marketplace;
mod migration;
mod mux;
mod pty;
mod startup;
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
        // The sessions daemon (see mux/): before anything of the app's own
        // startup — the single-instance check would hand it to the running
        // Termo, and nothing else of it (hooks, watchers) is the daemon's.
        [flag, rest @ ..] if flag == mux::MUX_FLAG => {
            mux::server::run(rest.first().map_or("", String::as_str), rest.get(1).map(String::as_str));
            true
        }
        // The agents' own process (see agents/host/): the same, for agents
        // that keep running with Termo closed.
        [flag, rest @ ..] if flag == agents::host::HOST_FLAG => {
            agents::host::server::run(rest.first().map_or("", String::as_str), rest.get(1).map(String::as_str));
            true
        }
        _ => false,
    }
}

/// What a notification of the agents' process was about, when Termo was
/// started from one (`--open-notification <target>`).
fn notification_target(args: impl Iterator<Item = String>) -> Option<serde_json::Value> {
    let mut args = args.skip_while(|arg| arg != agents::host::OPEN_FLAG);
    args.next()?;
    serde_json::from_str(&args.next()?).ok().filter(serde_json::Value::is_object)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    startup::begin();
    // Before anything reads the config folders (see migration.rs).
    migration::migrate_from_termini();
    let builder = tauri::Builder::default();
    // One Termo process at a time: opening it again opens one more window of
    // the running one instead (see terminal_windows.rs). Two processes would
    // each run the agents over the same data — both taking from one queue,
    // each blind to the other's runs. First, as the plugin asks. Left out of
    // debug builds, so a dev build can run beside the installed one.
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        // Started by a click on a notification of the agents' process:
        // the main window shows what it was about, as for its own.
        if let Some(target) = notification_target(args.into_iter()) {
            commands::notification_commands::open_target(app, terminal_windows::MAIN, target);
            return;
        }
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
            startup::mark("setup");
            let loaded = config::store::load(app.handle());
            // Built hidden: it shows once its page has its tabs in place.
            if let Some(window) = app.get_webview_window(terminal_windows::MAIN) {
                startup::hold(&window, &loaded.window);
                commands::window_shape::attach(&window);
            }
            // The status bar says when the sessions daemon stops answering
            // (new consoles then open in the app) and offers to restart it.
            {
                let handle = app.handle().clone();
                app.state::<AppState>().pty.on_daemon_health(move |answering| {
                    let _ = tauri::Emitter::emit(&handle, "sessions-daemon", answering);
                });
            }
            // A new console asked for from the tailnet: the window Termo
            // opened with opens it as a tab (see remote-open.svelte.ts).
            {
                let handle = app.handle().clone();
                app.state::<AppState>().pty.on_open(move |request, choice, cwd| {
                    let asked = serde_json::json!({ "request": request, "choice": choice, "cwd": cwd });
                    let _ = tauri::Emitter::emit(&handle, "remote-open", asked);
                });
                // And one closed from there: its tab closes.
                let handle = app.handle().clone();
                app.state::<AppState>().pty.on_close(move |session| {
                    let _ = tauri::Emitter::emit(&handle, "remote-close", session.to_string());
                });
            }
            // The agents, from the tailnet (see mux/agents.rs): what's asked
            // of them is run where they run, as the windows' own calls are;
            // what they say goes there while someone watches; and an agent
            // saved or deleted from there shows in the sidebar.
            {
                let handle = app.handle().clone();
                app.state::<AppState>().pty.on_agents_call(move |request, name, args| {
                    let handle = handle.clone();
                    std::thread::spawn(move || {
                        let result = if mux::agents::RUNTIME_CALLS.contains(&name.as_str()) {
                            agents::host::client::call_where_they_run(&handle, &name, args)
                        } else if name == "agent_save" {
                            // Saved here: its git hook names this program.
                            commands::agent_commands::save_from_tailnet(&handle, args)
                        } else {
                            Err(format!("unknown call: {name}"))
                        };
                        handle.state::<AppState>().pty.agents_answer(request, result);
                    });
                });
                let handle = app.handle().clone();
                app.state::<AppState>().pty.on_agents_changed(move || {
                    let _ = tauri::Emitter::emit(&handle, "agents-changed", ());
                });
                for event in ["agent-run", "agent-run-event", "agent-question"] {
                    let handle = app.handle().clone();
                    tauri::Listener::listen_any(app.handle(), event, move |heard| {
                        if let Ok(payload) = serde_json::from_str(heard.payload()) {
                            handle.state::<AppState>().pty.agent_event(event, payload);
                        }
                    });
                }
            }
            // With remote access on, the consoles the app runs itself are
            // shown from the tailnet too (see pty/sharing.rs).
            if let Some(dir) = mux::dir() {
                if mux::remote::read_config(&dir).enabled {
                    app.state::<AppState>().pty.share(true);
                }
            }
            // The consoles of an earlier run that the sessions daemon still
            // runs (see mux/): what belongs to them stays.
            let kept: std::collections::HashSet<String> =
                app.state::<AppState>().pty.background().into_iter().map(|session| session.id.to_string()).collect();
            startup::mark("daemon");
            pty::agent_hooks::start_watcher(app.handle().clone(), kept.clone());
            pty::status_line::start_watcher(app.handle().clone());
            // With "Keep agents running when Termo closes", the agents run in
            // their own process (see agents/host/): it cleans up after itself,
            // and may be running some right now.
            agents::host::set_runs_here(!loaded.background_agents);
            if let Ok(root) = agents::store::root(app.handle()) {
                if !loaded.background_agents {
                    // Before any run can start: files a run killed with Termo left.
                    agents::runner::remove_leftovers(&root);
                    agents::orchestration::executor::recover(&root);
                }
                // Consoles' token files from before: their tokens died with it,
                // but for those of consoles still running.
                agents::console_bridge::remove_leftovers(&root, &kept);
            }
            startup::mark("leftovers");
            // The agents, their local server and refreshing the hooks: only
            // with Modo IA on (see ai_mode.rs).
            // Before anything looks for an agent's CLI.
            marketplace::agent_install::apply(&loaded.agent_install_dirs);
            let ai_mode_on = loaded.ai_mode_enabled;
            ai_mode::init(app.handle(), ai_mode_on);
            if loaded.background_agents && ai_mode_on {
                agents::host::client::start(app.handle());
            }
            startup::mark("ai_mode");
            // The entry that starts them at sign-in points at this build.
            {
                let loaded = loaded.clone();
                std::thread::spawn(move || agents::host::login::sync(&loaded));
            }
            // Opened by a notification the agents' process showed (see
            // agents/host/notify.rs): what it was about, once a window asks.
            if let Some(target) = notification_target(std::env::args()) {
                *app.state::<AppState>().opened_by.lock().unwrap() = Some(target);
            }
            // Every console's CLI settings point at this Termo (its port and
            // token), like the hooks do.
            agents::console_bridge::set_global(ai_mode_on && loaded.agents_from_any_console);
            agents::console_orchestration::set_enabled(loaded.orchestrations_from_consoles);
            if ai_mode_on && loaded.agents_from_any_console {
                if let Ok(root) = agents::store::root(app.handle()) {
                    std::thread::spawn(move || {
                        agents::console_bridge::register_global(&root, true);
                    });
                }
            }
            startup::mark("setup_done");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::pty_commands::pty_spawn,
            commands::pty_commands::pty_write,
            commands::pty_commands::pty_resize,
            commands::pty_commands::pty_claim_size,
            commands::pty_commands::pty_prompt_redraw,
            commands::pty_commands::pty_kill,
            commands::pty_commands::pty_attach,
            commands::pty_commands::pty_background,
            commands::pty_commands::pty_kill_background,
            commands::pty_commands::sessions_daemon_answering,
            commands::pty_commands::sessions_daemon_restart,
            commands::pty_commands::agent_events_restore,
            commands::remote_commands::remote_overview,
            commands::remote_commands::remote_activity,
            commands::remote_commands::remote_set_enabled,
            commands::remote_commands::remote_set_allowed,
            commands::remote_commands::remote_set_agents_edit,
            commands::remote_commands::remote_answer_pending,
            commands::remote_commands::remote_disconnect_all,
            commands::remote_commands::remote_machines,
            commands::remote_commands::remote_consoles,
            commands::remote_commands::remote_share_look,
            commands::remote_commands::remote_share_agents,
            commands::remote_commands::remote_set_open_choices,
            commands::remote_commands::remote_opened,
            commands::remote_commands::tailscale_connect,
            commands::remote_commands::tailscale_disconnect,
            commands::agent_commands::agents_background_set,
            commands::agent_commands::agents_host_state,
            commands::notification_commands::notification_take_opened,
            commands::link_commands::open_link,
            commands::link_commands::list_browsers,
            commands::zellij_commands::zellij_sessions,
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
            commands::harness_commands::harness_propose,
            commands::harness_commands::harness_unify,
            commands::harness_commands::harness_assist_cancel,
            commands::harness_commands::harness_write_instructions,
            commands::git_commands::git_blob,
            commands::git_commands::git_file_info,
            commands::git_commands::git_blame,
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
            commands::window_commands::window_ready,
            commands::window_commands::customize_status_bar,
            commands::window_commands::hide_preferences_window,
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
            commands::pty_commands::pty_release,
            commands::pty_commands::pty_busy,
            commands::layout_commands::list_layouts,
            commands::layout_commands::save_layout,
            commands::layout_commands::delete_layout,
            commands::layout_commands::save_window_session,
            commands::layout_commands::window_session,
            commands::tab_group_commands::list_tab_groups,
            commands::tab_group_commands::save_tab_group,
            commands::tab_group_commands::delete_tab_group,
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
            commands::starship_commands::starship_status,
            commands::starship_commands::install_starship,
            commands::starship_commands::install_starship_preset,
            commands::prompt_preview_commands::prompt_preview,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
