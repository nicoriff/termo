//! Orchestrations on disk: `app_data/agents/orchestrations/<id>/
//! orchestration.json`, one file each, with the plan as it stands and every
//! change made to it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::super::spec::is_valid_id;
use super::super::store::now_seconds;
use super::plan::Orchestration;

/// Under the agents' own folder, where no agent's folder can be: an agent's
/// has an agent.json, which this one never does.
fn all_dir(root: &Path) -> PathBuf {
    root.join("orchestrations")
}

fn dir_of(root: &Path, id: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(is_valid_id(id), "invalid orchestration id");
    Ok(all_dir(root).join(id))
}

/// The executor's thread and the commands both change an orchestration:
/// each change reads it, edits it and writes it back under this lock.
fn lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    &LOCK
}

fn read(root: &Path, id: &str) -> Option<Orchestration> {
    let dir = dir_of(root, id).ok()?;
    serde_json::from_str(&fs::read_to_string(dir.join("orchestration.json")).ok()?).ok()
}

fn write(root: &Path, orchestration: &mut Orchestration) -> anyhow::Result<()> {
    let dir = dir_of(root, &orchestration.id)?;
    fs::create_dir_all(&dir)?;
    let now = now_seconds();
    if orchestration.created_at == 0 {
        orchestration.created_at = now;
    }
    orchestration.updated_at = now;
    let path = dir.join("orchestration.json");
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string_pretty(orchestration)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Every orchestration, newest first.
pub fn list_in(root: &Path) -> Vec<Orchestration> {
    let mut all: Vec<Orchestration> = fs::read_dir(all_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.path().join("orchestration.json")).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    all.sort_by_key(|orchestration: &Orchestration| std::cmp::Reverse(orchestration.created_at));
    all
}

pub fn get_in(root: &Path, id: &str) -> Option<Orchestration> {
    let _guard = lock().lock().unwrap();
    read(root, id)
}

pub fn save_in(root: &Path, mut orchestration: Orchestration) -> anyhow::Result<Orchestration> {
    let _guard = lock().lock().unwrap();
    write(root, &mut orchestration)?;
    Ok(orchestration)
}

/// Changes the saved orchestration in one step and returns it as saved;
/// `None` when it's gone (deleted while it ran).
pub fn update_in(root: &Path, id: &str, change: impl FnOnce(&mut Orchestration)) -> Option<Orchestration> {
    let _guard = lock().lock().unwrap();
    let mut orchestration = read(root, id)?;
    change(&mut orchestration);
    write(root, &mut orchestration).ok()?;
    Some(orchestration)
}

pub fn delete_in(root: &Path, id: &str) -> anyhow::Result<()> {
    let _guard = lock().lock().unwrap();
    let dir = dir_of(root, id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::orchestration::plan::Status;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-orchestrations-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saves_updates_and_deletes() {
        let root = temp_root("crud");
        let saved = save_in(&root, Orchestration { id: "one".into(), name: "Login".into(), ..Orchestration::default() }).unwrap();
        assert!(saved.created_at > 0);
        let updated = update_in(&root, "one", |orchestration| orchestration.status = Status::Running).unwrap();
        assert_eq!(updated.status, Status::Running);
        assert_eq!(list_in(&root).len(), 1);
        assert_eq!(get_in(&root, "one").unwrap().name, "Login");
        delete_in(&root, "one").unwrap();
        assert!(get_in(&root, "one").is_none());
        assert!(update_in(&root, "one", |_| {}).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_id_cannot_leave_the_folder() {
        let root = temp_root("ids");
        assert!(save_in(&root, Orchestration { id: "../x".into(), ..Orchestration::default() }).is_err());
        assert!(get_in(&root, "../x").is_none());
        let _ = fs::remove_dir_all(&root);
    }
}
