//! Tab groups: a name and a color that gather tabs behind a chip in the tab
//! strip, as Chrome's do (see src/lib/tab-groups/tab-groups.ts). Each is
//! kept with a copy of its tabs, so a group whose tabs were all closed — or
//! that outlived Termo — can be opened again.
//!
//! Only the id is looked at here: the rest is stored as opaque JSON, so the
//! frontend can add fields without a matching change. Its own file under app
//! data, like the layouts; saved one group at a time so two windows don't
//! overwrite each other's, and every window hears about changes through
//! `tab-groups`.

use std::fs;
use std::path::PathBuf;

use tauri::{Emitter, Manager};

/// Far more than anyone keeps by hand; it only bounds a corrupted file.
const MAX_GROUPS: usize = 200;

fn file(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_data_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("tab_groups.json"))
}

fn id_of(group: &serde_json::Value) -> Option<&str> {
    group.get("id").and_then(|id| id.as_str()).filter(|id| !id.is_empty())
}

pub fn list(app: &tauri::AppHandle) -> Vec<serde_json::Value> {
    let Ok(path) = file(app) else {
        return Vec::new();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<serde_json::Value>>(&text).ok())
        .map(|groups| groups.into_iter().filter(|group| id_of(group).is_some()).collect())
        .unwrap_or_default()
}

fn store(app: &tauri::AppHandle, groups: &[serde_json::Value]) -> Result<(), String> {
    let write = || -> anyhow::Result<()> {
        let path = file(app)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(groups)?)?;
        fs::rename(&tmp, &path)?;
        Ok(())
    };
    write().map_err(|err| err.to_string())?;
    let _ = app.emit("tab-groups", groups);
    Ok(())
}

/// Replaces the group with the same id where it is, or adds it at the end.
fn upsert(mut groups: Vec<serde_json::Value>, incoming: serde_json::Value) -> Vec<serde_json::Value> {
    let Some(id) = id_of(&incoming).map(str::to_owned) else {
        return groups;
    };
    match groups.iter_mut().find(|group| id_of(group) == Some(id.as_str())) {
        Some(existing) => *existing = incoming,
        None => groups.push(incoming),
    }
    groups.truncate(MAX_GROUPS);
    groups
}

pub fn save(app: &tauri::AppHandle, group: serde_json::Value) -> Result<Vec<serde_json::Value>, String> {
    if id_of(&group).is_none() {
        return Ok(list(app));
    }
    let groups = upsert(list(app), group);
    store(app, &groups)?;
    Ok(groups)
}

pub fn remove(app: &tauri::AppHandle, id: &str) -> Result<Vec<serde_json::Value>, String> {
    let mut groups = list(app);
    groups.retain(|group| id_of(group) != Some(id));
    store(app, &groups)?;
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn saving_an_existing_group_replaces_it_in_place() {
        let groups = upsert(Vec::new(), json!({ "id": "a", "name": "API" }));
        let groups = upsert(groups, json!({ "id": "b", "name": "Web" }));
        let groups = upsert(groups, json!({ "id": "a", "name": "Backend", "tabs": [] }));
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], json!({ "id": "a", "name": "Backend", "tabs": [] }));
    }

    #[test]
    fn a_group_without_an_id_is_ignored() {
        assert!(upsert(Vec::new(), json!({ "name": "x" })).is_empty());
        assert!(upsert(Vec::new(), json!({ "id": "" })).is_empty());
    }
}
