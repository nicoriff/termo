//! What the Harness panel asks of a repository (see src-tauri/src/harness).
//! Every command takes the repository's root, checked to be one first.

use std::path::PathBuf;

use serde::Serialize;

use crate::harness::model::{self, Harness};
use crate::harness::sync::{self, FileChange};
use crate::i18n::{tr, Msg};

fn checked(root: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(root);
    (path.is_absolute() && path.join(".git").exists()).then_some(path).ok_or_else(|| tr(Msg::GitNotARepo))
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|err| err.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessState {
    /// The repository has a `.termo/harness.json`.
    exists: bool,
    spec: Harness,
    /// What the files lack to match the harness (edited by hand, or brought
    /// in by a pull): empty when they're in sync.
    pending: Vec<FileChange>,
}

fn state(root: &std::path::Path) -> Result<HarnessState, String> {
    let saved = model::load(root).map_err(|err| err.to_string())?;
    let exists = saved.is_some();
    let spec = saved.unwrap_or_default();
    let pending = if exists { sync::plan(root, &spec).map_err(|err| err.to_string())? } else { Vec::new() };
    Ok(HarnessState { exists, spec, pending })
}

#[tauri::command]
pub async fn harness_load(root: String) -> Result<HarnessState, String> {
    let root = checked(&root)?;
    blocking(move || state(&root)).await
}

/// What applying `spec` would change, file by file.
#[tauri::command]
pub async fn harness_preview(root: String, spec: Harness) -> Result<Vec<FileChange>, String> {
    let root = checked(&root)?;
    blocking(move || sync::plan(&root, &spec).map_err(|err| err.to_string())).await
}

/// Writes `spec` and every file it makes, and answers with the state after.
#[tauri::command]
pub async fn harness_apply(root: String, spec: Harness) -> Result<HarnessState, String> {
    let root = checked(&root)?;
    blocking(move || {
        let changes = sync::plan(&root, &spec).map_err(|err| err.to_string())?;
        sync::apply(&root, &changes).map_err(|err| tr(Msg::HarnessWriteFailed(&err.to_string())))?;
        state(&root)
    })
    .await
}
