//! Remote access from the app: its settings (who may connect to this
//! computer's consoles over the tailnet, see mux/remote.rs), who is
//! connected now, and connecting to another computer's.
//!
//! The settings live in the sessions daemon's folder, where the daemon
//! reads them; after a change it's told to apply them at once.

use std::net::{IpAddr, SocketAddr};

use serde::Serialize;
use tauri::Manager;

use crate::mux::protocol::{AgentInfo, OpenChoice, SessionInfo};
use crate::mux::remote::{self, Access, Allowed, Pending, RemoteConfig, Viewer, WebLook};
use crate::mux::tailscale::{self, Machine, Status};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOverview {
    /// Whether Tailscale's CLI is installed.
    pub tailscale_installed: bool,
    /// What Tailscale says, when it answers.
    pub tailscale: Option<Status>,
    pub config: RemoteConfig,
    pub pending: Vec<Pending>,
    /// Connected now (none when the sessions daemon isn't running).
    pub viewers: Vec<Viewer>,
}

fn dir() -> Result<std::path::PathBuf, String> {
    crate::mux::dir().ok_or_else(|| "no folder for the sessions daemon".to_string())
}

/// Tells this computer's daemon, if it runs, to apply the settings now.
fn reload(app: &tauri::AppHandle) {
    if let Some(mux) = app.state::<AppState>().pty.local_daemon() {
        if mux.is_connected_or_running() {
            let _ = mux.reload_remote();
        }
    }
}

#[tauri::command]
pub async fn remote_overview(app: tauri::AppHandle) -> Result<RemoteOverview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = dir()?;
        let viewers = app
            .state::<AppState>()
            .pty
            .local_daemon()
            .filter(|mux| mux.is_connected_or_running())
            .and_then(|mux| mux.viewers().ok())
            .unwrap_or_default();
        Ok(RemoteOverview {
            tailscale_installed: tailscale::cli().is_some(),
            tailscale: tailscale::status(),
            config: remote::read_config(&dir),
            pending: remote::read_pending(&dir),
            viewers,
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

/// What the status bar keeps an eye on, without asking Tailscale: whether
/// remote access is on, who's waiting and who's connected.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteActivity {
    pub enabled: bool,
    pub pending: Vec<Pending>,
    pub viewers: Vec<Viewer>,
}

#[tauri::command]
pub async fn remote_activity(app: tauri::AppHandle) -> Result<RemoteActivity, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = dir()?;
        let config = remote::read_config(&dir);
        if !config.enabled {
            return Ok(RemoteActivity { enabled: false, pending: Vec::new(), viewers: Vec::new() });
        }
        let viewers = app
            .state::<AppState>()
            .pty
            .local_daemon()
            .filter(|mux| mux.is_connected_or_running())
            .and_then(|mux| mux.viewers().ok())
            .unwrap_or_default();
        Ok(RemoteActivity { enabled: true, pending: remote::read_pending(&dir), viewers })
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Turns remote access on or off. Turned on for the first time, the user
/// themselves (their Tailscale login, from any of their computers) is who
/// may connect.
#[tauri::command]
pub async fn remote_set_enabled(app: tauri::AppHandle, enabled: bool) -> Result<RemoteConfig, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = dir()?;
        let mut config = remote::read_config(&dir);
        config.enabled = enabled;
        if enabled && config.allowed.is_empty() {
            if let Some(login) = tailscale::status().and_then(|status| status.me.login) {
                config.allowed.push(Allowed { login, node: None, access: Access::Full });
            }
        }
        remote::write_config(&dir, &config).map_err(|err| err.to_string())?;
        reload(&app);
        // The consoles the app runs itself, shown from there too.
        app.state::<AppState>().pty.share(enabled);
        Ok(config)
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Replaces the list of who may connect.
#[tauri::command]
pub async fn remote_set_allowed(app: tauri::AppHandle, allowed: Vec<Allowed>) -> Result<RemoteConfig, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = dir()?;
        let mut config = remote::read_config(&dir);
        config.allowed = allowed;
        remote::write_config(&dir, &config).map_err(|err| err.to_string())?;
        reload(&app);
        Ok(config)
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Whether whoever has full access may also create, change and delete
/// agents from there (see mux/agents.rs).
#[tauri::command]
pub async fn remote_set_agents_edit(app: tauri::AppHandle, enabled: bool) -> Result<RemoteConfig, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = dir()?;
        let mut config = remote::read_config(&dir);
        config.agents_edit = enabled;
        remote::write_config(&dir, &config).map_err(|err| err.to_string())?;
        reload(&app);
        Ok(config)
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Answers someone turned away: let in from that computer with `access`,
/// or (none) just forgotten.
#[tauri::command]
pub async fn remote_answer_pending(
    app: tauri::AppHandle,
    login: String,
    node: String,
    access: Option<Access>,
) -> Result<RemoteOverview, String> {
    {
        let dir = dir()?;
        let mut pending = remote::read_pending(&dir);
        pending.retain(|entry| !(entry.login == login && entry.node == node));
        remote::write_pending(&dir, &pending).map_err(|err| err.to_string())?;
        if let Some(access) = access {
            let mut config = remote::read_config(&dir);
            config.allowed.retain(|allowed| !(allowed.login == login && allowed.node.as_deref() == Some(node.as_str())));
            config.allowed.push(Allowed { login, node: Some(node), access });
            remote::write_config(&dir, &config).map_err(|err| err.to_string())?;
            reload(&app);
        }
    }
    remote_overview(app).await
}

/// Ends every connection from the tailnet.
#[tauri::command]
pub async fn remote_disconnect_all(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || match app.state::<AppState>().pty.local_daemon() {
        Some(mux) if mux.is_connected_or_running() => mux.disconnect_remote(),
        _ => Ok(()),
    })
    .await
    .map_err(|err| err.to_string())?
}

/// The other computers on the tailnet that are online.
#[tauri::command]
pub async fn remote_machines() -> Result<Vec<Machine>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let status = tailscale::status().ok_or_else(|| "Tailscale".to_string())?;
        Ok(status.peers.into_iter().filter(|peer| peer.online && peer.ip.is_some()).collect())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// The consoles another computer's daemon runs, asked over the tailnet.
#[tauri::command]
pub async fn remote_consoles(app: tauri::AppHandle, host: String, port: Option<u16>) -> Result<Vec<SessionInfo>, String> {
    let ip: IpAddr = host.parse().map_err(|_| format!("not an address: {host}"))?;
    if !tailscale::is_tailnet_address(ip) {
        return Err(format!("not on the tailnet: {host}"));
    }
    let addr = SocketAddr::new(ip, port.unwrap_or(remote::DEFAULT_PORT));
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().pty.remote(addr).list())
        .await
        .map_err(|err| err.to_string())?
}

/// One console's agent, as a window's status bar knows it.
#[derive(serde::Deserialize)]
pub struct ConsoleAgent {
    id: String,
    agent: Option<AgentInfo>,
}

/// What the windows' status bars know of the agents in their consoles,
/// for the pages showing them from the tailnet. Only the ones that changed.
#[tauri::command]
pub fn remote_share_agents(app: tauri::AppHandle, agents: Vec<ConsoleAgent>) {
    let consoles = &app.state::<AppState>().pty;
    for entry in agents {
        if let Ok(id) = uuid::Uuid::parse_str(&entry.id) {
            consoles.set_agent(id, entry.agent);
        }
    }
}

/// The theme and console font the app shows now, for the web client to
/// look the same (see mux::remote::WebLook). The daemon reads it on each
/// page load, so it's saved even with remote access off.
#[tauri::command]
pub async fn remote_share_look(look: WebLook) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || remote::write_look(&dir()?, look).map_err(|err| err.to_string()))
        .await
        .map_err(|err| err.to_string())?
}

/// What the new-tab menu offers, to open from the tailnet (see
/// pty/sharing.rs), and the folders the agents ran in lately.
#[tauri::command]
pub fn remote_set_open_choices(app: tauri::AppHandle, choices: Vec<OpenChoice>, recent: Vec<String>) {
    app.state::<AppState>().pty.set_open_choices(choices, recent);
}

/// What came of a new console the tailnet asked for (the `remote-open`
/// event): the console's id, or why not.
#[tauri::command]
pub async fn remote_opened(app: tauri::AppHandle, request: u64, console: Option<String>, error: Option<String>) {
    let result = match console.as_deref().map(uuid::Uuid::parse_str) {
        Some(Ok(id)) => Ok(id),
        _ => Err(error.unwrap_or_else(|| "failed".to_string())),
    };
    let _ = tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().pty.opened(request, result)).await;
}

/// Connects this computer to the tailnet: done, or the page to sign in at
/// first (the app opens it), or still at it.
#[tauri::command]
pub async fn tailscale_connect() -> Result<tailscale::Connecting, String> {
    tauri::async_runtime::spawn_blocking(tailscale::connect).await.map_err(|err| err.to_string())?
}

/// Disconnects this computer from the tailnet — every program using it here.
#[tauri::command]
pub async fn tailscale_disconnect() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(tailscale::disconnect).await.map_err(|err| err.to_string())?
}
