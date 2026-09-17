use tauri::ipc::Channel;
use tauri::Manager;
use uuid::Uuid;

use crate::config::store;
use crate::pty::{
    agent_hooks, bash_color_env, cwd_report, default_shell, list_shells as detect_shells, oh_my_posh, zsh_init,
    PtyEvent, ShellInfo,
};
use crate::state::AppState;

fn parse_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|_| format!("invalid session id: {id}"))
}

#[tauri::command]
pub fn pty_spawn(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    shell_path: Option<String>,
    shell_args: Option<Vec<String>>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    on_event: Channel<PtyEvent>,
) -> Result<String, String> {
    let (path, args) = match shell_path {
        Some(path) => (path, shell_args.unwrap_or_default()),
        None => {
            let shell = default_shell();
            (shell.path, shell.args)
        }
    };
    // Read from the saved config here rather than passed in by the frontend,
    // so every spawn path (startup, new tab, split) picks it up without each
    // one having to carry it along.
    let config = store::load(&app);
    let mut args = args;
    let mut env = bash_color_env(&path, config.bash_prompt_colors);
    if let Ok(dir) = agent_hooks::events_dir(&app) {
        env.push(("TERMINI_EVENTS_DIR", dir.to_string_lossy().into_owned()));
    }
    let mut zsh_lines: Vec<String> = Vec::new();
    if config.zsh_menu_complete {
        zsh_lines.extend(zsh_init::MENU_COMPLETE_LINES.iter().map(|line| line.to_string()));
    }
    // A broken shell integration must never keep the shell itself from
    // opening — on failure the session just starts without it.
    if config.oh_my_posh_enabled {
        if let Some((binary, managed)) = oh_my_posh::find(&app) {
            let _ = oh_my_posh::apply(
                &app,
                &binary,
                managed,
                &config.oh_my_posh_theme,
                &path,
                &mut args,
                &mut env,
                &mut zsh_lines,
            );
        }
    }
    if zsh_init::is_zsh(&path) && !zsh_lines.is_empty() {
        let _ = zsh_init::apply(&app, &zsh_lines, &mut env);
    }
    // Last: its prompt wrapper has to call whatever prompt the steps above
    // installed (see cwd_report).
    cwd_report::apply(&path, &mut args, &mut env);

    state
        .pty
        .spawn(&path, &args, &env, cwd.as_deref(), cols, rows, on_event)
        .map(|id| id.to_string())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn pty_write(state: tauri::State<'_, AppState>, id: String, data: String) -> Result<(), String> {
    let id = parse_id(&id)?;
    state
        .pty
        .write(id, data.as_bytes())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn pty_resize(
    state: tauri::State<'_, AppState>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let id = parse_id(&id)?;
    state
        .pty
        .resize(id, cols, rows)
        .map_err(|err| err.to_string())
}

/// Async and off the main thread: dropping the session closes the PTY, and on
/// Windows ClosePseudoConsole blocks until the console's output is drained.
/// Run as a sync command (which executes on the main thread) that froze the
/// whole app whenever the drain stalled, e.g. when a frontend reload tore
/// down every terminal at once.
#[tauri::command]
pub async fn pty_kill(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = parse_id(&id)?;
    agent_hooks::forget_session(&app, id);
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().pty.kill(id))
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| err.to_string())
}

/// Refreshing the whole process table takes tens of ms, so it runs off the
/// main thread. An already-exited console reports `None` instead of an error.
#[tauri::command]
pub async fn pty_memory(app: tauri::AppHandle, id: String) -> Result<Option<u64>, String> {
    let id = parse_id(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let pid = state.pty.pid(id).ok().flatten()?;
        state.memory.tree_bytes(pid)
    })
    .await
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_shells() -> Vec<ShellInfo> {
    detect_shells()
}
