#[cfg(unix)]
mod foreground;
mod agent_events;
pub mod agent_hooks;
pub mod agent_launch;
mod bash_colors;
pub mod bin_search;
mod codex_usage;
mod grok_usage;
pub mod console_shims;
mod consoles;
mod sharing;
pub mod copilot_status;
pub mod cwd_report;
mod manager;
pub mod memory;
pub mod oh_my_posh;
pub mod powershell_keys;
pub mod utf8_console;
mod recorder;
mod session;
mod shell_detect;
pub mod status_line;
pub mod zsh_init;

pub use bash_colors::bash_color_env;
pub use consoles::Consoles;
pub use manager::PtyManager;
pub use memory::MemorySampler;
pub(crate) use session::contains_cursor_report;
pub use session::PtySession;
pub use shell_detect::{default_shell, list_shells, ShellInfo};

/// Everything needed to start a console, resolved: what the app works out
/// from its config (shell integrations, the agents' bridge, hooks) before
/// the console starts, so whoever starts it — this process or the sessions
/// daemon (see mux/) — needs nothing else.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpawnSpec {
    pub path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
    /// Keys that make the shell redraw its prompt (see PtySession::prompt_redraw).
    pub prompt_redraw: Option<Vec<u8>>,
    /// Folders appended to the console's PATH (see bin_search::extra_dirs).
    pub extra_path: Vec<std::path::PathBuf>,
}

/// What a console reports, as it happens.
#[derive(Clone, Debug)]
pub enum ConsoleEvent {
    Output(Vec<u8>),
    Exit(i32),
    /// See PtyEvent::ForegroundProcess.
    Foreground(Option<String>),
}

/// Where a console's events go. False once nobody is listening any more —
/// the console then stops forwarding, but keeps reading (see PtySession).
pub type EventSink = std::sync::Arc<dyn Fn(ConsoleEvent) -> bool + Send + Sync>;

/// A console's events as the webview receives them.
#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PtyEvent {
    Data { data: String },
    Exit { code: i32 },
    /// Full resolved executable path of the process currently in the
    /// foreground of this PTY — a confirming signal for Modo IA's tool
    /// detection. A full path rather than a bare name because Claude Code's
    /// installer symlinks a stable `claude` name to a version-pinned binary,
    /// so the basename alone is a version string, not "claude" (see
    /// pty/foreground.rs). Unix only: portable-pty's process_group_leader()
    /// has no ConPTY equivalent, so this variant is simply never sent on
    /// Windows.
    ForegroundProcess { path: Option<String> },
}

impl From<ConsoleEvent> for PtyEvent {
    fn from(event: ConsoleEvent) -> Self {
        use base64::{engine::general_purpose::STANDARD, Engine};
        match event {
            ConsoleEvent::Output(bytes) => PtyEvent::Data { data: STANDARD.encode(bytes) },
            ConsoleEvent::Exit(code) => PtyEvent::Exit { code },
            ConsoleEvent::Foreground(path) => PtyEvent::ForegroundProcess { path },
        }
    }
}

/// A sink that hands a console's events to the webview.
pub fn channel_sink(channel: tauri::ipc::Channel<PtyEvent>) -> EventSink {
    std::sync::Arc::new(move |event| channel.send(event.into()).is_ok())
}
