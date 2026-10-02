//! Agent conversations the user marked as favorites in Modo IA, to reopen
//! them later from the "+" menu or the command palette, under a name of
//! their own if they gave one.
//!
//! Unlike the recent sessions (agent_sessions.rs), nothing here comes or goes
//! on its own: the user adds, renames and removes them, so there is no cap
//! beyond a sanity one. Kept in its own file under app data, like the recent
//! ones, and every window hears about changes through `agent-favorites`.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::i18n::{tr, Msg};

/// Far more than anyone keeps by hand; it only bounds a corrupted file.
const MAX_FAVORITES: usize = 200;
/// Longer than any sensible name, short enough for a tab.
const MAX_NAME_CHARS: usize = 80;
const TOOLS: &[&str] = &["claude", "codex", "gemini", "copilot"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentFavorite {
    /// "claude", "codex", "gemini" or "copilot", matching ToolId.
    pub tool: String,
    /// The agent's own session id — what its resume command takes.
    pub agent_session_id: String,
    /// The user's name for it; None shows the folder, as recent sessions do.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub project_dir: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Unix seconds.
    #[serde(default)]
    pub added_at: i64,
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

/// The same ids recent sessions can be reopened with: anything else would
/// have to be quoted on a command line (see resume-command.ts).
fn is_safe_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 200 && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// What a favorite may hold, whoever sent it; the name trimmed, and empty
/// meaning none.
fn checked(mut favorite: AgentFavorite) -> Result<AgentFavorite, String> {
    if !TOOLS.contains(&favorite.tool.as_str()) || !is_safe_session_id(&favorite.agent_session_id) {
        return Err(tr(Msg::AgentFavoriteInvalid));
    }
    favorite.name = favorite
        .name
        .map(|name| name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME_CHARS).collect::<String>())
        .filter(|name| !name.is_empty());
    Ok(favorite)
}

/// Adds the favorite, or updates the one for that session in place (a rename
/// keeps its spot and when it was added).
fn upsert(mut favorites: Vec<AgentFavorite>, favorite: AgentFavorite) -> Vec<AgentFavorite> {
    match favorites
        .iter_mut()
        .find(|existing| existing.agent_session_id == favorite.agent_session_id)
    {
        Some(existing) => {
            *existing = AgentFavorite {
                added_at: existing.added_at,
                cwd: favorite.cwd.or(existing.cwd.take()),
                project_dir: favorite.project_dir.or(existing.project_dir.take()),
                model: favorite.model.or(existing.model.take()),
                ..favorite
            };
        }
        None => {
            favorites.insert(0, AgentFavorite { added_at: now_seconds(), ..favorite });
            favorites.truncate(MAX_FAVORITES);
        }
    }
    favorites
}

fn file(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_data_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("agent-favorites.json"))
}

pub fn list(app: &tauri::AppHandle) -> Vec<AgentFavorite> {
    let Ok(path) = file(app) else {
        return Vec::new();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<AgentFavorite>>(&text).ok())
        .map(|favorites| favorites.into_iter().filter_map(|favorite| checked(favorite).ok()).collect())
        .unwrap_or_default()
}

fn store(app: &tauri::AppHandle, favorites: &[AgentFavorite]) -> Result<(), String> {
    let write = || -> anyhow::Result<()> {
        let path = file(app)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(favorites)?)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    };
    write().map_err(|err| err.to_string())?;
    let _ = app.emit("agent-favorites", favorites);
    Ok(())
}

pub fn save(app: &tauri::AppHandle, favorite: AgentFavorite) -> Result<Vec<AgentFavorite>, String> {
    let favorites = upsert(list(app), checked(favorite)?);
    store(app, &favorites)?;
    Ok(favorites)
}

pub fn remove(app: &tauri::AppHandle, agent_session_id: &str) -> Result<Vec<AgentFavorite>, String> {
    let mut favorites = list(app);
    favorites.retain(|favorite| favorite.agent_session_id != agent_session_id);
    store(app, &favorites)?;
    Ok(favorites)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn favorite(id: &str, name: Option<&str>) -> AgentFavorite {
        AgentFavorite {
            tool: "claude".into(),
            agent_session_id: id.into(),
            name: name.map(String::from),
            cwd: Some("E:/proj".into()),
            project_dir: None,
            model: None,
            added_at: 0,
        }
    }

    #[test]
    fn a_rename_keeps_its_place_and_what_it_knew() {
        let list = upsert(Vec::new(), favorite("a", None));
        let list = upsert(list, favorite("b", None));
        let added = list[1].added_at;
        let renamed = AgentFavorite { cwd: None, ..favorite("a", Some("Refactor")) };
        let list = upsert(list, renamed);
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].agent_session_id, "a");
        assert_eq!(list[1].name.as_deref(), Some("Refactor"));
        assert_eq!(list[1].cwd.as_deref(), Some("E:/proj"));
        assert_eq!(list[1].added_at, added);
    }

    #[test]
    fn names_are_trimmed_and_an_empty_one_is_none() {
        assert_eq!(checked(favorite("a", Some("  Bug 12  "))).unwrap().name.as_deref(), Some("Bug 12"));
        assert_eq!(checked(favorite("a", Some("   "))).unwrap().name, None);
        assert_eq!(checked(favorite("a", Some("a\nb\u{7}"))).unwrap().name.as_deref(), Some("ab"));
        let long = "x".repeat(300);
        assert_eq!(checked(favorite("a", Some(&long))).unwrap().name.unwrap().chars().count(), MAX_NAME_CHARS);
    }

    /// The id goes on a command line when the favorite is reopened.
    #[test]
    fn refuses_unknown_tools_and_unsafe_ids() {
        assert!(checked(favorite("0b739325-ae54-4319-9ffa-8ff83f70114e", None)).is_ok());
        for id in ["", "a b", "x;calc", "$(id)", "--flag'"] {
            assert!(checked(favorite(id, None)).is_err(), "{id:?}");
        }
        let unknown = AgentFavorite { tool: "calc".into(), ..favorite("a", None) };
        assert!(checked(unknown).is_err());
    }
}
