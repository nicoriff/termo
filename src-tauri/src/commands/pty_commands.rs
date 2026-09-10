use tauri::ipc::Channel;
use uuid::Uuid;

use crate::pty::{default_shell, list_shells as detect_shells, PtyEvent, ShellInfo};
use crate::state::AppState;

fn parse_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|_| format!("invalid session id: {id}"))
}

#[tauri::command]
pub fn pty_spawn(
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

    state
        .pty
        .spawn(&path, &args, cwd.as_deref(), cols, rows, on_event)
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

#[tauri::command]
pub fn pty_kill(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let id = parse_id(&id)?;
    state.pty.kill(id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_shells() -> Vec<ShellInfo> {
    detect_shells()
}
