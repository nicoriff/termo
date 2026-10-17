use tauri::ipc::Channel;
use tauri::Manager;
use uuid::Uuid;

use crate::config::store;
use crate::mux::protocol::SessionInfo;
use crate::pty::agent_launch::{self, AgentLaunch};
use crate::pty::{
    agent_hooks, bash_color_env, console_shims, cwd_report, default_shell, list_shells as detect_shells, oh_my_posh, powershell_keys,
    utf8_console, zsh_init,
    channel_sink, PtyEvent, ShellInfo, SpawnSpec,
};
use crate::state::AppState;

fn parse_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|_| format!("invalid session id: {id}"))
}

/// Async and off the main thread: starting a console takes around a tenth of
/// a second, and as a sync command (which runs on the main thread) a layout's
/// worth of them started one after another with the window frozen meanwhile.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn pty_spawn(
    app: tauri::AppHandle,
    // The console belongs to the window that opened it (see terminal_windows.rs).
    window: tauri::WebviewWindow,
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
    let owner = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        spawn_console(&app, &owner, shell_path, shell_args, cwd, cols, rows, agent, on_event)
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Getting a console ready writes files several consoles share (a shell's
/// init script, Oh My Posh's): one at a time, now that they start side by
/// side. Starting the process itself — the slow part — stays outside it.
static PREPARING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The shell an agent's command line goes through to get its PATH (see
/// agent_launch.rs). Windows needs none, and finding it there lists every
/// shell — which runs `wsl.exe` — for nothing.
fn agent_login_shell() -> String {
    #[cfg(windows)]
    {
        String::new()
    }
    #[cfg(not(windows))]
    {
        default_shell().path
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_console(
    app: &tauri::AppHandle,
    owner: &str,
    shell_path: Option<String>,
    shell_args: Option<Vec<String>>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    agent: Option<AgentLaunch>,
    on_event: Channel<PtyEvent>,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let persistent = store::load(app).persistent_sessions;
    let (spec, token) = prepare_console(app, shell_path, shell_args, cwd, cols, rows, agent)?;
    match state.pty.spawn(owner, &spec, channel_sink(on_event), persistent) {
        Ok(id) => {
            // The user's agents, reachable from this console (see
            // agents/console_bridge.rs).
            if let Some(token) = &token {
                crate::agents::console_bridge::bind(token, &id.to_string());
            }
            Ok(id.to_string())
        }
        Err(err) => {
            // Never ran: its token goes now.
            if let Some(token) = &token {
                crate::agents::console_bridge::discard(token);
            }
            Err(err.to_string())
        }
    }
}

/// Works out everything a console needs before it starts — the shell or
/// agent to run, its integrations, its environment — into a spec anyone can
/// start, plus the agents' bridge token to bind to it once it has an id.
#[allow(clippy::too_many_arguments)]
fn prepare_console(
    app: &tauri::AppHandle,
    shell_path: Option<String>,
    shell_args: Option<Vec<String>>,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
    agent: Option<AgentLaunch>,
) -> Result<(SpawnSpec, Option<String>), String> {
    let spec = |path: String, args: Vec<String>, env: Vec<(&str, String)>, prompt_redraw: Option<Vec<u8>>| SpawnSpec {
        path,
        args,
        env: env.into_iter().map(|(key, value)| (key.to_string(), value)).collect(),
        cwd: cwd.clone(),
        cols,
        rows,
        prompt_redraw,
        extra_path: crate::pty::bin_search::extra_dirs(),
    };
    if let Some(agent) = agent {
        let preparing = PREPARING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // The user's agents, reachable from this console (see
        // agents/console_bridge.rs) — unless the CLI has them in its own
        // settings already.
        let extras = crate::agents::store::root(app)
            .map(|root| {
                let global = store::load(app).agents_from_any_console;
                crate::agents::console_bridge::launch_extras(&agent.tool, &root, global)
            })
            .unwrap_or_default();
        let (path, args) = agent_launch::command_with(&agent, &agent_login_shell(), &extras.args)?;
        // None of the shell integrations below: there's no shell prompt.
        let mut env = extras.env;
        if let Ok(dir) = agent_hooks::events_dir(app) {
            env.push(("TERMO_EVENTS_DIR", dir.to_string_lossy().into_owned()));
        }
        drop(preparing);
        return Ok((spec(path, args, env, None), extras.token));
    }
    let preparing = PREPARING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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
    let config = store::load(app);
    let mut args = args;
    let mut env = bash_color_env(&path, config.bash_prompt_colors);
    // What a login shell would have as PATH, as Terminal.app's consoles do:
    // the shell here isn't one, so it would never read the profile files
    // that put `claude`, node or Homebrew on it.
    #[cfg(not(windows))]
    if let Some(login) = crate::pty::bin_search::login_path() {
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let dirs = std::env::split_paths(&login).chain(std::env::split_paths(&inherited));
        let mut seen = std::collections::HashSet::new();
        let dirs: Vec<_> = dirs.filter(|dir| seen.insert(dir.clone())).collect();
        if let Ok(joined) = std::env::join_paths(dirs) {
            env.push(("PATH", joined.to_string_lossy().into_owned()));
        }
    }
    if let Ok(dir) = agent_hooks::events_dir(app) {
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
        if let Some((binary, managed)) = oh_my_posh::find(app) {
            prompt_redraw = oh_my_posh::apply(
                app,
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
    // The user's agents, for the agent CLIs typed in this shell (see
    // pty/console_shims.rs).
    let console_token = crate::agents::store::root(app).ok().and_then(|root| {
        console_shims::apply(&root, config.agents_from_any_console, &path, &mut args, &mut env, &mut zsh_lines)
    });
    if zsh_init::is_zsh(&path) {
        // Always written for zsh, even with every zsh option off: this is
        // what makes the console report its directory (see CWD_REPORT_LINES).
        zsh_lines.extend(zsh_init::CWD_REPORT_LINES.iter().map(|line| line.to_string()));
        let _ = zsh_init::apply(app, &zsh_lines, &mut env);
    }
    utf8_console::apply(config.console_utf8, &path, &mut args);
    powershell_keys::apply(&path, &mut args);
    // Last: its prompt wrapper has to call whatever prompt the steps above
    // installed (see cwd_report).
    cwd_report::apply(&path, &mut args, &mut env);
    drop(preparing);
    Ok((spec(path, args, env, prompt_redraw), console_token))
}

/// Shows, in this window, a console the sessions daemon kept running: what
/// it printed so far, then everything new. With `host` (an address on the
/// tailnet), another computer's. Off the main thread: the first request may
/// have to wait for the daemon.
#[tauri::command]
pub async fn pty_attach(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
    host: Option<String>,
    on_event: Channel<PtyEvent>,
) -> Result<SessionInfo, String> {
    let id = parse_id(&id)?;
    let host = host.map(|host| host.parse::<std::net::SocketAddr>().map_err(|err| err.to_string())).transpose()?;
    // Another computer's console is only ever reached over the tailnet.
    if host.is_some_and(|host| !crate::mux::tailscale::is_tailnet_address(host.ip())) {
        return Err("not on the tailnet".into());
    }
    let owner = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().pty.attach(&owner, id, channel_sink(on_event), host).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Where the agent in a console the app just attached to stood (see
/// agent_hooks::restore), sent as the usual events.
#[tauri::command]
pub fn agent_events_restore(app: tauri::AppHandle, id: String) {
    agent_hooks::restore(&app, &id);
}

/// The consoles the sessions daemon runs, shown in some window or not.
/// None when it isn't running: asking doesn't start it.
#[tauri::command]
pub async fn pty_background(app: tauri::AppHandle) -> Result<Vec<SessionInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().pty.background())
        .await
        .map_err(|err| err.to_string())
}

/// Whether the sessions daemon answered the last time it was asked. The
/// `sessions-daemon` event says when that changes.
#[tauri::command]
pub fn sessions_daemon_answering(state: tauri::State<'_, AppState>) -> bool {
    state.pty.local_daemon().is_none_or(|mux| mux.is_answering())
}

/// Replaces a sessions daemon that stopped answering with a new one; the
/// consoles it ran end with it.
#[tauri::command]
pub async fn sessions_daemon_restart(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mux = state.pty.local_daemon().ok_or("no folder for the sessions daemon")?;
        mux.restart()
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Ends a console the sessions daemon runs, whether or not a tab shows it.
#[tauri::command]
pub async fn pty_kill_background(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let id = parse_id(&id)?;
    agent_hooks::forget_session(&app, id);
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().pty.kill_kept(id))
        .await
        .map_err(|err| err.to_string())?
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

/// The console back to its tab's size if someone on the tailnet sized it
/// meanwhile (see Consoles::claim_size): whether it changed. Waits on the
/// daemon, hence the thread.
#[tauri::command]
pub async fn pty_claim_size(app: tauri::AppHandle, id: String, cols: u16, rows: u16) -> Result<bool, String> {
    let id = parse_id(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().pty.claim_size(id, cols, rows).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
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

/// CPU and memory of the app, the machine and each console, for the
/// diagnostics panel (see pty/memory.rs). Off the main thread: it refreshes
/// the process table.
#[tauri::command]
pub async fn diagnostics_snapshot(app: tauri::AppHandle) -> Result<crate::pty::memory::Diagnostics, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let consoles: Vec<(String, u32)> =
            state.pty.roots().into_iter().map(|(id, pid)| (id.to_string(), pid)).collect();
        state.memory.diagnostics(&consoles)
    })
    .await
    .map_err(|err| err.to_string())
}

/// Ends everything running under a console's shell (not the shell: closing
/// its tab does that, through the usual `pty_kill`). What the diagnostics
/// panel's "Kill" does to a tab that is stuck; the number is how many
/// processes it ended.
#[tauri::command]
pub async fn pty_kill_tree(app: tauri::AppHandle, id: String) -> Result<usize, String> {
    let id = parse_id(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.pty.pid(id).ok().flatten().map_or(0, |pid| state.memory.kill_descendants(pid))
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
