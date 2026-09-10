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
}
