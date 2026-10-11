//! What the Companion panel asks for (see companion/mod.rs). Reading a
//! session means going through a file that can be tens of megabytes, so
//! every command here runs off the main thread.

use serde::Serialize;

use crate::companion::rules::{self, Finding};
use crate::companion::sources::{self, SessionFile};
use crate::companion::{pricing, SessionUsage};

/// How many of the latest sessions the panel offers to look at.
const RECENT: usize = 30;

/// A session, what's worth pointing out about it, and the day the prices
/// its costs were worked out with were last checked.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReport {
    pub session: SessionUsage,
    pub findings: Vec<Finding>,
    pub prices_as_of: &'static str,
}

async fn off_main<T: Send + 'static>(read: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(read).await.map_err(|err| err.to_string())
}

/// One session of an agent's, by the agent's own id for it. None when the
/// agent has no such session on disk, or when it isn't one that's read yet.
#[tauri::command]
pub async fn companion_session(tool: String, session_id: String) -> Result<Option<SessionReport>, String> {
    off_main(move || {
        let session = sources::read(&tool, &session_id)?;
        let findings = rules::findings(&session);
        Some(SessionReport { session, findings, prices_as_of: pricing::as_of() })
    })
    .await
}

/// The sessions last worked in, newest first.
#[tauri::command]
pub async fn companion_recent() -> Result<Vec<SessionFile>, String> {
    off_main(|| {
        let mut recent = sources::list(0);
        recent.truncate(RECENT);
        recent
    })
    .await
}
