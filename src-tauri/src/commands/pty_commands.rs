use tauri::ipc::Channel;
use tauri::Manager;
use uuid::Uuid;

use crate::config::store;
use crate::pty::agent_launch::{self, AgentLaunch};
use crate::pty::{
    agent_hooks, bash_color_env, cwd_report, default_shell, list_shells as detect_shells, oh_my_posh, powershell_keys,
    utf8_console, zsh_init,
    PtyEvent, ShellInfo,
};
use crate::state::AppState;

fn parse_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|_| format!("invalid session id: {id}"))
}

#[tauri::command]
pub fn pty_spawn(
    app: tauri::AppHandle,
    // The console belongs to the window that opened it (see terminal_windows.rs).
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
    shell_path: Option<String>,
    shell_args: Option<Vec<String>>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    // Start this agent CLI as the console's own process (see agent_launch.rs)
    // rather than the shell, which is then only what it falls back to.
    agent: Option<AgentLaunch>,
    on_event: Channel<PtyEvent>,
) -> Result<String, String> {
    if let Some(agent) = agent {
        // The user's agents, reachable from this console (see
        // agents/console_bridge.rs) — unless the CLI has them in its own
        // settings already.
        let extras = crate::agents::store::root(&app)
            .map(|root| {
                let global = store::load(&app).agents_from_any_console;
                crate::agents::console_bridge::launch_extras(&agent.tool, &root, global)
            })
            .unwrap_or_default();
        let (path, args) = agent_launch::command_with(&agent, &default_shell().path, &extras.args)?;
        // None of the shell integrations below: there's no shell prompt.
        let mut env = extras.env;
        if let Ok(dir) = agent_hooks::events_dir(&app) {
            env.push(("TERMO_EVENTS_DIR", dir.to_string_lossy().into_owned()));
        }
        let spawned = state.pty.spawn(window.label(), &path, &args, &env, cwd.as_deref(), cols, rows, None, on_event);
        return match spawned {
            Ok(id) => {
                if let Some(token) = &extras.token {
                    crate::agents::console_bridge::bind(token, &id.to_string());
                }
                Ok(id.to_string())
            }
            Err(err) => {
                // Never ran: its token goes now.
                if let Some(token) = &extras.token {
                    crate::agents::console_bridge::bind(token, "never-started");
                    crate::agents::console_bridge::release("never-started");
                }
                Err(err.to_string())
            }
        };
    }
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
        env.push(("TERMO_EVENTS_DIR", dir.to_string_lossy().into_owned()));
    }
    let mut zsh_lines: Vec<String> = Vec::new();
    if config.zsh_menu_complete {
        zsh_lines.extend(zsh_init::MENU_COMPLETE_LINES.iter().map(|line| line.to_string()));
    }
    // A broken shell integration must never keep the shell itself from
    // opening — on failure the session just starts without it.
    let mut prompt_redraw = None;
    if config.oh_my_posh_enabled {
        if let Some((binary, managed)) = oh_my_posh::find(&app) {
            prompt_redraw = oh_my_posh::apply(
                &app,
                &binary,
                managed,
                &config.oh_my_posh_theme,
                &path,
                &mut args,
                &mut env,
                &mut zsh_lines,
            )
            .unwrap_or(None)
            .map(<[u8]>::to_vec);
        }
    }
    if zsh_init::is_zsh(&path) {
        // Always written for zsh, even with every zsh option off: this is
        // what makes the console report its directory (see CWD_REPORT_LINES).
        zsh_lines.extend(zsh_init::CWD_REPORT_LINES.iter().map(|line| line.to_string()));
        let _ = zsh_init::apply(&app, &zsh_lines, &mut env);
    }
    utf8_console::apply(config.console_utf8, &path, &mut args);
    powershell_keys::apply(&path, &mut args);
    // Last: its prompt wrapper has to call whatever prompt the steps above
    // installed (see cwd_report).
    cwd_report::apply(&path, &mut args, &mut env);

    state
        .pty
        .spawn(window.label(), &path, &args, &env, cwd.as_deref(), cols, rows, prompt_redraw, on_event)
        .map(|id| id.to_string())
        .map_err(|err| err.to_string())
}

/// Asks the console's shell to redraw its prompt at the size it now has,
/// once a resize has settled. Nothing is sent while a command is running
/// under the shell (see PtySession::prompt_redraw), nor to shells that need
/// no nudge. Blocking work (it walks the process table), hence the thread.
#[tauri::command]
pub async fn pty_prompt_redraw(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = parse_id(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state
            .pty
            .prompt_redraw(id, |pid| state.memory.busy_roots(&[pid]).is_empty())
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
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

/// How many consoles have something running under their shell: every
/// window's (quitting), or with `this_window` only the asking window's
/// (closing it). Off the main thread for the same reason as pty_memory: it
/// refreshes the process table.
#[tauri::command]
pub async fn busy_consoles(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    this_window: Option<bool>,
) -> Result<usize, String> {
    let owner = this_window.unwrap_or(false).then(|| window.label().to_string());
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let pids = match &owner {
            Some(owner) => state.pty.pids_of(owner),
            None => state.pty.pids(),
        };
        state.memory.busy_roots(&pids).len()
    })
    .await
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_shells() -> Vec<ShellInfo> {
    detect_shells()
}

/// The Windows build, for how xterm reads a console's output (see
/// TerminalInstance.mount): consoles reach it through ConPTY, and the one
/// Windows 10 ships (before build 21376) reflows the screen itself on a
/// resize and measures characters with an older table than xterm's.
/// RtlGetVersion rather than GetVersionEx, which reports whatever the
/// application manifest claims. None outside Windows.
#[tauri::command]
pub fn windows_build() -> Option<u32> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct OsVersionInfoW {
            size: u32,
            major: u32,
            minor: u32,
            build: u32,
            platform: u32,
            service_pack: [u16; 128],
        }
        #[link(name = "ntdll")]
        extern "system" {
            fn RtlGetVersion(info: *mut OsVersionInfoW) -> i32;
        }
        let mut info = OsVersionInfoW {
            size: std::mem::size_of::<OsVersionInfoW>() as u32,
            major: 0,
            minor: 0,
            build: 0,
            platform: 0,
            service_pack: [0; 128],
        };
        // SAFETY: `info` is a correctly sized OSVERSIONINFOW whose size field
        // is set, as RtlGetVersion requires; it only writes into it.
        let status = unsafe { RtlGetVersion(&mut info) };
        (status == 0 && info.build > 0).then_some(info.build)
    }
    #[cfg(not(windows))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn reads_the_real_windows_build() {
        // Windows 10's first build was 10240; anything older isn't supported.
        assert!(windows_build().is_some_and(|build| build >= 10240));
    }

    #[cfg(not(windows))]
    #[test]
    fn has_no_windows_build_elsewhere() {
        assert_eq!(windows_build(), None);
    }
}
