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

/// The list with what a channel just saw merged in: newest first, capped,
/// keeping fields the other channel filled in (hooks know the cwd, the status
/// line knows the model and project dir). A session the list doesn't have yet
/// only joins it once `begun`, its conversation having something in it: an
/// agent opened and closed without a word leaves nothing to resume, and its
/// resume command only fails ("No conversation found"), closing the tab it
/// opened. `None` when nothing changes.
fn merged(mut sessions: Vec<AgentSession>, incoming: AgentSession, begun: bool) -> Option<Vec<AgentSession>> {
    if incoming.agent_session_id.is_empty() {
        return None;
    }
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
        None if begun => incoming,
        None => return None,
    };
    sessions.insert(0, merged);
    sessions.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
    sessions.truncate(MAX_SESSIONS);
    Some(sessions)
}

/// Records what a channel just saw (see `merged`).
pub fn record(app: &tauri::AppHandle, incoming: AgentSession, begun: bool) {
    let Some(sessions) = merged(list(app), incoming, begun) else {
        return;
    };
    if save(app, &sessions).is_ok() {
        let _ = app.emit("agent-sessions", &sessions);
    }
}

/// Where Claude Code keeps its conversations: a folder per project.
fn claude_projects() -> Option<PathBuf> {
    let base = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".claude")))?;
    Some(base.join("projects"))
}

/// Whether a Claude Code session left a conversation to resume; `None` when
/// that can't be told (no projects folder to look in).
fn claude_conversation_exists(projects: &std::path::Path, agent_session_id: &str) -> Option<bool> {
    let folders = fs::read_dir(projects).ok()?;
    let file = format!("{agent_session_id}.jsonl");
    Some(folders.flatten().any(|folder| folder.path().join(&file).is_file()))
}

/// The list without the Claude Code sessions that left no conversation —
/// recorded before `merged` waited for one, or deleted since: reopening them
/// only fails. Other agents keep theirs where Termo can't look as cheaply,
/// so their entries stay.
pub fn list_resumable(app: &tauri::AppHandle) -> Vec<AgentSession> {
    let mut sessions = list(app);
    let Some(projects) = claude_projects() else { return sessions };
    let before = sessions.len();
    sessions.retain(|session| {
        session.tool != "claude" || claude_conversation_exists(&projects, &session.agent_session_id) != Some(false)
    });
    if sessions.len() != before {
        let _ = save(app, &sessions);
    }
    sessions
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

    /// The merge/cap rules on a plain list: no app handle, no disk.
    fn merge(sessions: Vec<AgentSession>, incoming: AgentSession) -> Vec<AgentSession> {
        merged(sessions, incoming, true).unwrap()
    }

    #[test]
    fn a_session_joins_the_list_only_once_it_has_begun() {
        assert_eq!(merged(vec![at("a", 10)], at("b", 20), false), None);
        // One already listed is still brought up to date.
        let sessions = merged(vec![at("a", 10)], at("a", 30), false).unwrap();
        assert_eq!(sessions[0].last_seen, 30);
    }

    #[test]
    fn a_claude_session_without_a_conversation_is_told_apart() {
        let projects = std::env::temp_dir().join(format!("termo-sessions-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(projects.join("E--Proyectos-Booky")).unwrap();
        fs::write(projects.join("E--Proyectos-Booky").join("kept.jsonl"), "{}").unwrap();
        assert_eq!(claude_conversation_exists(&projects, "kept"), Some(true));
        assert_eq!(claude_conversation_exists(&projects, "empty"), Some(false));
        assert_eq!(claude_conversation_exists(&projects.join("missing"), "kept"), None);
        fs::remove_dir_all(&projects).unwrap();
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
