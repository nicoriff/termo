//! Named console layouts: the split tree the user had open, saved under a name
//! so it can be reopened later (see src/lib/layout/layout-snapshot.ts).
//!
//! The tree itself is stored as opaque JSON. Rust never looks inside it, so the
//! frontend can add fields to a tile or a tab without a matching change here.
//!
//! Kept in its own file under app data rather than in `config.json`, for the
//! same reason as agent-sessions.json: the frontend rewrites the config on
//! every setting change, which would make it easy to drop entries.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedLayout {
    pub name: String,
    /// Unix seconds of the last save under this name.
    pub saved_at: i64,
    /// The frontend's serialized split tree, stored verbatim.
    pub root: serde_json::Value,
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
    Ok(dir.join("layouts.json"))
}

pub fn list(app: &tauri::AppHandle) -> Vec<SavedLayout> {
    let Ok(path) = file(app) else {
        return Vec::new();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_all(app: &tauri::AppHandle, layouts: &[SavedLayout]) -> anyhow::Result<()> {
    let path = file(app)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(layouts)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Saving under a name that already exists replaces that layout — that is how
/// the user updates one — and the list stays sorted by name so the menu order
/// never depends on when something was saved.
fn merge(mut layouts: Vec<SavedLayout>, incoming: SavedLayout) -> Vec<SavedLayout> {
    let key = incoming.name.to_lowercase();
    layouts.retain(|layout| layout.name.to_lowercase() != key);
    layouts.push(incoming);
    layouts.sort_by_key(|layout| layout.name.to_lowercase());
    layouts
}

/// Returns the full list, so the caller doesn't need a second round trip.
pub fn save(app: &tauri::AppHandle, name: &str, root: serde_json::Value) -> Vec<SavedLayout> {
    let name = name.trim();
    if name.is_empty() {
        return list(app);
    }
    let layouts = merge(
        list(app),
        SavedLayout {
            name: name.to_string(),
            saved_at: now_seconds(),
            root,
        },
    );
    let _ = save_all(app, &layouts);
    layouts
}

pub fn delete(app: &tauri::AppHandle, name: &str) -> Vec<SavedLayout> {
    let key = name.trim().to_lowercase();
    let mut layouts = list(app);
    layouts.retain(|layout| layout.name.to_lowercase() != key);
    let _ = save_all(app, &layouts);
    layouts
}

/// What each window has open, kept up to date while consoles run in the
/// sessions daemon (see mux/), so that opening Termo again puts every
/// console that is still running back in its tab — the tree as the
/// frontend serialized it, with each console's daemon id. By window label;
/// its own file, like the named layouts.
fn window_sessions_file(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_data_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("window-sessions.json"))
}

/// Every window writes its own entry: one read-modify-write at a time.
static WINDOW_SESSIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn read_window_sessions(path: &std::path::Path) -> serde_json::Map<String, serde_json::Value> {
    fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// Records what `window` has open; `None` forgets it.
pub fn save_window_session(app: &tauri::AppHandle, window: &str, root: Option<serde_json::Value>) -> anyhow::Result<()> {
    let _guard = WINDOW_SESSIONS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = window_sessions_file(app)?;
    let mut all = read_window_sessions(&path);
    match root {
        Some(root) => all.insert(window.to_string(), root),
        None => all.remove(window),
    };
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&all)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn window_session(app: &tauri::AppHandle, window: &str) -> Option<serde_json::Value> {
    let _guard = WINDOW_SESSIONS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    read_window_sessions(&window_sessions_file(app).ok()?).remove(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(name: &str) -> SavedLayout {
        SavedLayout {
            name: name.to_string(),
            saved_at: 0,
            root: serde_json::json!({ "kind": "leaf" }),
        }
    }

    #[test]
    fn saving_under_an_existing_name_replaces_it() {
        let mut updated = layout("work");
        updated.saved_at = 99;
        let layouts = merge(vec![layout("work"), layout("home")], updated);
        assert_eq!(layouts.len(), 2);
        let work = layouts.iter().find(|l| l.name == "work").unwrap();
        assert_eq!(work.saved_at, 99);
    }

    /// Otherwise "Work" and "work" would sit in the menu as two entries that
    /// look the same.
    fn names(layouts: &[SavedLayout]) -> Vec<&str> {
        layouts.iter().map(|l| l.name.as_str()).collect()
    }

    #[test]
    fn a_name_differing_only_in_case_is_the_same_layout() {
        let layouts = merge(vec![layout("Work")], layout("work"));
        assert_eq!(names(&layouts), vec!["work"]);
    }

    #[test]
    fn stays_sorted_by_name() {
        let layouts = merge(vec![layout("zeta"), layout("alpha")], layout("mid"));
        assert_eq!(names(&layouts), vec!["alpha", "mid", "zeta"]);
    }

    #[test]
    fn keeps_the_tree_untouched() {
        let tree = serde_json::json!({ "kind": "split", "ratio": 0.42 });
        let layouts = merge(
            Vec::new(),
            SavedLayout {
                name: "x".into(),
                saved_at: 0,
                root: tree.clone(),
            },
        );
        assert_eq!(layouts[0].root, tree);
    }
}
