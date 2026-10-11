//! Where each agent CLI keeps its sessions, and how to read one into a
//! `SessionUsage`. These are other programs' own files, in formats nobody
//! promised to keep: a line that doesn't read is skipped and a session that
//! doesn't read at all is simply not there, never an error in the panel.

pub mod claude;

use std::path::PathBuf;

use serde::Serialize;

use super::SessionUsage;

/// A session found on disk, before it's read.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionFile {
    pub tool: String,
    pub session_id: String,
    #[serde(skip)]
    pub path: PathBuf,
    /// Unix milliseconds of its last change.
    pub modified_at: i64,
    pub bytes: u64,
}

/// Every agent's sessions changed since `since` (Unix milliseconds), newest
/// first.
pub fn list(since: i64) -> Vec<SessionFile> {
    let mut found = claude::list(since);
    found.sort_by_key(|file| std::cmp::Reverse(file.modified_at));
    found
}

/// One session of `tool`'s, by the agent's own id for it.
pub fn read(tool: &str, session_id: &str) -> Option<SessionUsage> {
    match tool {
        "claude" => claude::read(&claude::find(session_id)?),
        _ => None,
    }
}

/// Unix milliseconds of an RFC 3339 timestamp; 0 when it isn't one.
pub(crate) fn millis(timestamp: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(timestamp).map(|at| at.timestamp_millis()).unwrap_or(0)
}

pub(crate) fn modified_millis(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_millis() as i64)
        .unwrap_or(0)
}

/// The start of `text`, cut at a character boundary.
pub(crate) fn excerpt(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    match trimmed.char_indices().nth(max_chars) {
        Some((end, _)) => format!("{}…", &trimmed[..end]),
        None => trimmed.to_string(),
    }
}
