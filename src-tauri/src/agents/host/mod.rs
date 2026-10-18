//! Agents that keep running with Termo closed: the agents' own process.
//!
//! With "Keep agents running when Termo closes" on (AppConfig.background_agents),
//! the agents don't run in the app but in `termo --agents`, a process of
//! their own that outlives it (like the sessions daemon does for consoles,
//! see mux/): their schedules, the folders they watch, the git hooks' and
//! other agents' requests, the local webhook and MCP server, the runs.
//!
//! The app is then a client of it. Their definitions and history are files
//! both read and write; what only the process that runs them can do (start
//! or stop a run, answer a question, drive an orchestration) the app asks
//! it for (see ops.rs), and what the agents say (a run's steps, a question)
//! it passes on to the app's windows. With no app to tell, it shows the
//! notifications itself (see notify.rs).
//!
//! It listens on 127.0.0.1, on a port of its own choosing, and only answers
//! a client that presents the token in its info file (`host.json`, in a
//! folder only this user can read). One per user: a second one started by
//! mistake sees the first and leaves.

pub mod client;
pub mod login;
mod notify;
pub mod protocol;
pub mod server;

pub use notify::OPEN_FLAG;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

/// `termo --agents <build> <folder>`: runs the agents' process instead of
/// the app (see lib::run_cli_mode_from_args).
pub const HOST_FLAG: &str = "--agents";

/// The same folder Tauri's `app_data_dir` names (tauri.conf.json's
/// identifier); the process has no AppHandle to ask.
pub const IDENTIFIER: &str = "com.ln.termo";

/// Whether this process is the one running the agents: the app, unless
/// they run in their own process; that process, always.
static HERE: AtomicBool = AtomicBool::new(true);

pub fn runs_here() -> bool {
    HERE.load(Ordering::SeqCst)
}

pub fn set_runs_here(here: bool) {
    HERE.store(here, Ordering::SeqCst);
}

/// What a running agents' process leaves for the app to find it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostInfo {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub version: String,
    /// Which build of Termo it runs (see mux::build_stamp).
    pub build: String,
}

/// Its folder. A dev build's is its own, as the sessions daemon's is.
/// TERMO_AGENTS_HOST_DIR puts it elsewhere (tests).
pub fn dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("TERMO_AGENTS_HOST_DIR") {
        return Some(PathBuf::from(dir));
    }
    let name = if cfg!(debug_assertions) { "agents-host-dev" } else { "agents-host" };
    Some(dirs::data_dir()?.join(IDENTIFIER).join(name))
}

/// The agents' folder, found without the app (Tauri's `app_data_dir`
/// joined with "agents", as store::root says).
pub fn agents_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("TERMO_AGENTS_ROOT") {
        return Some(PathBuf::from(dir));
    }
    Some(dirs::data_dir()?.join(IDENTIFIER).join("agents"))
}

/// The app's config file, found without the app (Tauri's `app_config_dir`).
pub fn config_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TERMO_CONFIG_PATH") {
        return Some(PathBuf::from(path));
    }
    Some(dirs::config_dir()?.join(IDENTIFIER).join("config.json"))
}

fn info_path(dir: &Path) -> PathBuf {
    dir.join("host.json")
}

pub fn read_info(dir: &Path) -> Option<HostInfo> {
    let text = std::fs::read_to_string(info_path(dir)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_info(dir: &Path, info: &HostInfo) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = info_path(dir);
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(info).map_err(io::Error::other)?;
    super::console_bridge::write_private(&tmp, &text)?;
    std::fs::rename(&tmp, &path)
}

/// Removes the info file, if it is still this process's.
fn remove_info(dir: &Path, pid: u32) {
    if read_info(dir).is_some_and(|info| info.pid == pid) {
        let _ = std::fs::remove_file(info_path(dir));
    }
}
