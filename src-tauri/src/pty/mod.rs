#[cfg(unix)]
mod foreground;
mod manager;
mod session;
mod shell_detect;

pub use manager::PtyManager;
pub use session::PtySession;
pub use shell_detect::{default_shell, list_shells, ShellInfo};

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
