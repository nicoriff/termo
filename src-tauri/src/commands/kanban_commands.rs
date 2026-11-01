//! The Kanban panel and tab: reading boards is reading their files; every
//! change goes where the agents run (agents/ops.rs), since the driver there
//! writes the same files.

use serde_json::{json, Value};

use crate::agents::host::client::dispatch;
use crate::agents::kanban::model::{Column, KanbanBoard};
use crate::agents::kanban::store;
use crate::agents::kanban::worktree;
use crate::commands::agent_commands::root;

/// Off the main thread: a window reads it as it starts.
#[tauri::command]
pub async fn kanban_list(app: tauri::AppHandle) -> Result<Vec<KanbanBoard>, String> {
    Ok(store::list_in(&root(&app)?))
}

/// The branches a board's worktrees could start from: the folder's current
/// one first. Empty when it isn't a repository.
#[tauri::command]
pub async fn kanban_branches(folder: String) -> Result<Vec<String>, String> {
    let dir = std::path::PathBuf::from(folder.trim());
    if !worktree::is_repo(&dir) {
        return Ok(Vec::new());
    }
    let current = worktree::current_branch(&dir);
    let listed = crate::git::read(&dir, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]).unwrap_or_default();
    let mut branches: Vec<String> = current.into_iter().collect();
    for branch in listed.lines().map(str::trim).filter(|branch| !branch.is_empty() && !branch.starts_with("termo/kanban/")) {
        if !branches.iter().any(|known| known == branch) {
            branches.push(branch.to_string());
        }
    }
    Ok(branches)
}

/// Creates a board or changes its settings (`board` as kanban::ops::Settings).
#[tauri::command]
pub async fn kanban_save(app: tauri::AppHandle, board: Value) -> Result<KanbanBoard, String> {
    dispatch(app, "kanban_save", board).await
}

#[tauri::command]
pub async fn kanban_set_enabled(app: tauri::AppHandle, id: String, enabled: bool) -> Result<KanbanBoard, String> {
    dispatch(app, "kanban_set_enabled", json!({ "id": id, "enabled": enabled })).await
}

#[tauri::command]
pub async fn kanban_delete(app: tauri::AppHandle, id: String) -> Result<Vec<KanbanBoard>, String> {
    dispatch(app, "kanban_delete", json!({ "id": id })).await
}

/// Creates a task (no id) or edits one (`task` as kanban::ops::TaskInput).
#[tauri::command]
pub async fn kanban_task_upsert(app: tauri::AppHandle, board_id: String, task: Value) -> Result<KanbanBoard, String> {
    dispatch(app, "kanban_task_upsert", json!({ "boardId": board_id, "task": task })).await
}

#[tauri::command]
pub async fn kanban_task_move(
    app: tauri::AppHandle,
    board_id: String,
    task_id: String,
    column: Column,
    before: Option<String>,
    cancel: bool,
) -> Result<KanbanBoard, String> {
    let args = json!({ "boardId": board_id, "taskId": task_id, "column": column, "before": before, "cancel": cancel });
    dispatch(app, "kanban_task_move", args).await
}

/// The calls on one task that take nothing else: cancel, delete, resolve, merge.
async fn on_task(app: tauri::AppHandle, call: &'static str, board_id: String, task_id: String) -> Result<KanbanBoard, String> {
    dispatch(app, call, json!({ "boardId": board_id, "taskId": task_id })).await
}

#[tauri::command]
pub async fn kanban_task_cancel(app: tauri::AppHandle, board_id: String, task_id: String) -> Result<KanbanBoard, String> {
    on_task(app, "kanban_task_cancel", board_id, task_id).await
}

#[tauri::command]
pub async fn kanban_task_delete(app: tauri::AppHandle, board_id: String, task_id: String) -> Result<KanbanBoard, String> {
    on_task(app, "kanban_task_delete", board_id, task_id).await
}

#[tauri::command]
pub async fn kanban_task_resolve(app: tauri::AppHandle, board_id: String, task_id: String) -> Result<KanbanBoard, String> {
    on_task(app, "kanban_task_resolve", board_id, task_id).await
}

#[tauri::command]
pub async fn kanban_task_merge(app: tauri::AppHandle, board_id: String, task_id: String) -> Result<KanbanBoard, String> {
    on_task(app, "kanban_task_merge", board_id, task_id).await
}

#[tauri::command]
pub async fn kanban_task_note(app: tauri::AppHandle, board_id: String, task_id: String, text: String) -> Result<KanbanBoard, String> {
    dispatch(app, "kanban_task_note", json!({ "boardId": board_id, "taskId": task_id, "text": text })).await
}
