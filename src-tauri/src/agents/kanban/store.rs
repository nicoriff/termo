//! Kanban boards on disk: `app_data/agents/kanban/<id>/kanban.json`, one
//! file each, with its tasks, their logs and its column agents.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::super::spec::is_valid_id;
use super::super::store::now_seconds;
use super::model::KanbanBoard;

/// Under the agents' own folder, where no agent's folder can be: an agent's
/// has an agent.json, which this one never does.
fn all_dir(root: &Path) -> PathBuf {
    root.join("kanban")
}

pub fn dir_of(root: &Path, id: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(is_valid_id(id), "invalid board id");
    Ok(all_dir(root).join(id))
}

/// The driver's thread, the commands and the MCP tools all change a board:
/// each change reads it, edits it and writes it back under this lock.
fn lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    &LOCK
}

fn read(root: &Path, id: &str) -> Option<KanbanBoard> {
    let dir = dir_of(root, id).ok()?;
    serde_json::from_str(&fs::read_to_string(dir.join("kanban.json")).ok()?).ok()
}

fn write(root: &Path, board: &mut KanbanBoard) -> anyhow::Result<()> {
    let dir = dir_of(root, &board.id)?;
    fs::create_dir_all(&dir)?;
    let now = now_seconds();
    if board.created_at == 0 {
        board.created_at = now;
    }
    board.updated_at = now;
    let path = dir.join("kanban.json");
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string_pretty(board)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Every board, by name.
pub fn list_in(root: &Path) -> Vec<KanbanBoard> {
    let mut all: Vec<KanbanBoard> = fs::read_dir(all_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.path().join("kanban.json")).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    all.sort_by_key(|board: &KanbanBoard| board.name.to_lowercase());
    all
}

pub fn get_in(root: &Path, id: &str) -> Option<KanbanBoard> {
    let _guard = lock().lock().unwrap();
    read(root, id)
}

pub fn save_in(root: &Path, mut board: KanbanBoard) -> anyhow::Result<KanbanBoard> {
    let _guard = lock().lock().unwrap();
    write(root, &mut board)?;
    Ok(board)
}

/// Changes the saved board in one step and returns it as saved; `None` when
/// it's gone.
pub fn update_in(root: &Path, id: &str, change: impl FnOnce(&mut KanbanBoard)) -> Option<KanbanBoard> {
    let _guard = lock().lock().unwrap();
    let mut board = read(root, id)?;
    change(&mut board);
    write(root, &mut board).ok()?;
    Some(board)
}

/// The same, for a change that can refuse: nothing is written then.
pub fn try_update_in<T>(
    root: &Path,
    id: &str,
    change: impl FnOnce(&mut KanbanBoard) -> Result<T, String>,
) -> Option<Result<(KanbanBoard, T), String>> {
    let _guard = lock().lock().unwrap();
    let mut board = read(root, id)?;
    let value = match change(&mut board) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    Some(write(root, &mut board).map(|_| (board, value)).map_err(|err| err.to_string()))
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

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-kanban-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saves_updates_and_deletes() {
        let root = temp_root("crud");
        let saved = save_in(&root, KanbanBoard { id: "one".into(), name: "Web".into(), ..KanbanBoard::default() }).unwrap();
        assert!(saved.created_at > 0);
        let updated = update_in(&root, "one", |board| board.enabled = true).unwrap();
        assert!(updated.enabled);
        assert_eq!(list_in(&root).len(), 1);
        let refused = try_update_in(&root, "one", |board| {
            board.name = "changed".into();
            Err::<(), _>("no".to_string())
        });
        assert_eq!(refused, Some(Err("no".into())));
        assert_eq!(get_in(&root, "one").unwrap().name, "Web");
        delete_in(&root, "one").unwrap();
        assert!(get_in(&root, "one").is_none());
        assert!(update_in(&root, "one", |_| {}).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_id_cannot_leave_the_folder() {
        let root = temp_root("ids");
        assert!(save_in(&root, KanbanBoard { id: "../x".into(), ..KanbanBoard::default() }).is_err());
        assert!(get_in(&root, "../x").is_none());
        let _ = fs::remove_dir_all(&root);
    }
}
