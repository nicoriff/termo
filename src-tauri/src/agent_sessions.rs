//! Recent Claude Code / Codex / Gemini CLI sessions, so Modo IA can list them
//! and reopen one later: `claude --resume <id>`, `codex resume <id>` and
//! `gemini --resume <id>` all take the
//! agent's own session id, which arrives in the hook payload's common fields
//! and in the status line's JSON (see pty/agent_hooks.rs, pty/status_line.rs).
//!
//! Kept in its own file under app data rather than in `config.json`: this list
//! grows and changes on its own, and the frontend rewrites the config on every
//! setting change, which would make it easy to drop entries.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

/// Plenty to get back to the last few days of work without the list needing
/// its own search.
const MAX_SESSIONS: usize = 20;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    /// "claude", "codex" or "gemini", matching ToolId on the frontend.
    pub tool: String,
    /// The agent's own session id — what its resume command takes.
    pub agent_session_id: String,
    /// Where the agent was running, used to reopen the console there.
    pub cwd: Option<String>,
    /// The project root when the agent reported one (status line only).
    pub project_dir: Option<String>,
    /// Model display name, when the status line reported it.
    pub model: Option<String>,
    /// Unix seconds of the last event seen for this session.
    pub last_seen: i64,
}

impl AgentSession {
    pub fn new(tool: &str, agent_session_id: &str) -> Self {
        Self {
            tool: tool.to_string(),
            agent_session_id: agent_session_id.to_string(),
            cwd: None,
            project_dir: None,
            model: None,
            last_seen: now_seconds(),
        }
    }

    pub fn with_cwd(mut self, cwd: Option<String>) -> Self {
        self.cwd = cwd.filter(|value| !value.is_empty());
        self
    }

    pub fn with_project_dir(mut self, project_dir: Option<String>) -> Self {
        self.project_dir = project_dir.filter(|value| !value.is_empty());
        self
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model.filter(|value| !value.is_empty());
        self
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

fn file(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_data_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("agent-sessions.json"))
}

pub fn list(app: &tauri::AppHandle) -> Vec<AgentSession> {
    let Ok(path) = file(app) else {
        return Vec::new();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(app: &tauri::AppHandle, sessions: &[AgentSession]) -> anyhow::Result<()> {
    let path = file(app)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(sessions)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Merges what a channel just saw into the list: newest first, capped, and
/// keeping fields the other channel filled in (hooks know the cwd, the status
/// line knows the model and project dir).
pub fn record(app: &tauri::AppHandle, incoming: AgentSession) {
    if incoming.agent_session_id.is_empty() {
        return;
    }
    let mut sessions = list(app);
    let merged = match sessions
        .iter()
        .position(|session| session.agent_session_id == incoming.agent_session_id)
    {
        Some(index) => {
            let existing = sessions.remove(index);
            AgentSession {
                cwd: incoming.cwd.or(existing.cwd),
                project_dir: incoming.project_dir.or(existing.project_dir),
                model: incoming.model.or(existing.model),
                ..incoming
            }
        }
        None => incoming,
    };

    sessions.insert(0, merged);
    sessions.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
    sessions.truncate(MAX_SESSIONS);

    if save(app, &sessions).is_ok() {
        let _ = app.emit("agent-sessions", &sessions);
    }
}

pub fn forget(app: &tauri::AppHandle, agent_session_id: &str) -> Vec<AgentSession> {
    let mut sessions = list(app);
    sessions.retain(|session| session.agent_session_id != agent_session_id);
    if save(app, &sessions).is_ok() {
        let _ = app.emit("agent-sessions", &sessions);
    }
    sessions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(id: &str, seconds: i64) -> AgentSession {
        AgentSession {
            last_seen: seconds,
            ..AgentSession::new("claude", id)
        }
    }

    /// The merge/cap rules, exercised on a plain list so no app handle (and no
    /// disk) is needed — `record` is the same logic plus load/save/emit.
    fn merge(mut sessions: Vec<AgentSession>, incoming: AgentSession) -> Vec<AgentSession> {
        let merged = match sessions
            .iter()
            .position(|session| session.agent_session_id == incoming.agent_session_id)
        {
            Some(index) => {
                let existing = sessions.remove(index);
                AgentSession {
                    cwd: incoming.cwd.or(existing.cwd),
                    project_dir: incoming.project_dir.or(existing.project_dir),
                    model: incoming.model.or(existing.model),
                    ..incoming
                }
            }
            None => incoming,
        };
        sessions.insert(0, merged);
        sessions.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
        sessions.truncate(MAX_SESSIONS);
        sessions
    }

    #[test]
    fn keeps_one_entry_per_session_and_the_newest_first() {
        let sessions = merge(vec![at("a", 10), at("b", 20)], at("a", 30));
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].agent_session_id, "a");
        assert_eq!(sessions[0].last_seen, 30);
    }

    #[test]
    fn a_channel_without_a_field_does_not_erase_it() {
        let stored = vec![AgentSession::new("claude", "a")
            .with_cwd(Some("E:/proj".into()))
            .with_model(Some("Sonnet 5".into()))];
        // The hook channel knows the cwd but never the model.
        let sessions = merge(stored, AgentSession::new("claude", "a").with_cwd(Some("E:/proj".into())));
        assert_eq!(sessions[0].model.as_deref(), Some("Sonnet 5"));
        assert_eq!(sessions[0].cwd.as_deref(), Some("E:/proj"));
    }

    #[test]
    fn drops_the_oldest_past_the_cap() {
        let mut sessions: Vec<AgentSession> = (0..MAX_SESSIONS)
            .map(|index| at(&format!("s{index}"), index as i64))
            .collect();
        sessions = merge(sessions, at("newest", 999));
        assert_eq!(sessions.len(), MAX_SESSIONS);
        assert_eq!(sessions[0].agent_session_id, "newest");
        assert!(!sessions.iter().any(|session| session.agent_session_id == "s0"));
    }

    #[test]
    fn empty_values_never_become_fields() {
        let session = AgentSession::new("codex", "a").with_cwd(Some(String::new()));
        assert_eq!(session.cwd, None);
    }
}
