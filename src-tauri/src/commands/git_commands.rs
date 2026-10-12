//! What the Git panel, the status bar's item and the diff and graph tabs ask
//! of a repository (see src-tauri/src/git). Every one of them takes the
//! repository's root, checked to be one before git is run there.

use std::path::{Path, PathBuf};

use tauri::Manager;

use crate::git::forge::ForgeKind;
use crate::git::graph::{self, Branch, CommitDetail, GraphCommit, GraphScope};
use crate::git::ops::{Context, GitOp};
use crate::git::pr::{self, NewPr, PrPatch, PrState, Tools};
use crate::git::snapshot::{self, RepoSnapshot};
use crate::git::{read, read_line, repo_root};
use crate::i18n::{tr, Msg};
use crate::state::AppState;

/// `root` as a path, when it's where a repository starts.
fn checked(root: &str) -> Option<PathBuf> {
    let path = PathBuf::from(root);
    (path.is_absolute() && path.join(".git").exists()).then_some(path)
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tauri::async_runtime::spawn_blocking(work).await.ok()
}

/// The repository `path` is in, as it is right now; `None` outside one.
#[tauri::command]
pub async fn git_snapshot(path: String) -> Option<RepoSnapshot> {
    blocking(move || repo_root(Path::new(&path)).map(|root| snapshot::snapshot(&root))).await.flatten()
}

/// A file as it was at `rev` (a commit, `HEAD`, or the index when empty);
/// `None` when it wasn't there. `path` is relative to the repository.
#[tauri::command]
pub async fn git_blob(root: String, rev: String, path: String) -> Result<Option<String>, String> {
    let Some(root) = checked(&root) else { return Ok(None) };
    blocking(move || graph::blob(&root, &rev, &path)).await.unwrap_or(Ok(None))
}

#[tauri::command]
pub async fn git_branches(root: String) -> Vec<Branch> {
    let Some(root) = checked(&root) else { return Vec::new() };
    blocking(move || graph::branches(&root)).await.unwrap_or_default()
}

/// A page of the history: `extra` are more places to start from than HEAD.
#[tauri::command]
pub async fn git_graph(root: String, scope: GraphScope, extra: Vec<String>, skip: u32, limit: u32) -> Vec<GraphCommit> {
    let Some(root) = checked(&root) else { return Vec::new() };
    blocking(move || graph::graph(&root, scope, &extra, skip, limit)).await.unwrap_or_default()
}

#[tauri::command]
pub async fn git_commit_detail(root: String, sha: String) -> Option<CommitDetail> {
    let root = checked(&root)?;
    blocking(move || graph::commit_detail(&root, &sha)).await.flatten()
}

fn branch_of(root: &Path) -> Option<String> {
    read_line(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
}

fn upstream_of(root: &Path) -> Option<String> {
    read_line(root, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"])
}

/// Runs an action to its end, reporting through `git-op` events as it goes.
/// Resolves with what git printed last, rejects with why it failed.
#[tauri::command]
pub async fn git_run(app: tauri::AppHandle, root: String, op: GitOp) -> Result<String, String> {
    let path = checked(&root).ok_or_else(|| tr(Msg::GitNotARepo))?;
    let ops = app.state::<AppState>().git.clone();
    blocking(move || {
        let ctx = Context::read(&path);
        let result = ops.run(&app, &root, &op, &ctx);
        // What the request was read against may no longer hold (a push, a
        // new branch): the next look asks again.
        if let Some(branch) = ctx.branch {
            pr::forget(&path, &branch);
        }
        result
    })
    .await
    .unwrap_or_else(|| Err(tr(Msg::GitFailed("git"))))
}

#[tauri::command]
pub fn git_cancel(state: tauri::State<'_, AppState>, root: String) {
    state.git.cancel(&root);
}

/// The repositories with an action under way (a window that just opened
/// missed their "started").
#[tauri::command]
pub fn git_busy(state: tauri::State<'_, AppState>) -> Vec<String> {
    state.git.busy()
}

/// The branch a request would be for, and the service its remote is on.
fn pr_target(root: &Path) -> (Option<String>, Option<ForgeKind>) {
    let remotes = read(root, &["remote", "-v"]).map(|out| snapshot::parse_remotes(&out)).unwrap_or_default();
    let upstream = upstream_of(root);
    let kind = snapshot::main_remote(upstream.as_deref(), &remotes).and_then(|remote| remote.forge.as_ref()).map(|forge| forge.kind);
    (branch_of(root), kind)
}

#[tauri::command]
pub async fn git_pr(root: String, refresh: bool) -> PrState {
    let Some(root) = checked(&root) else { return PrState::Unsupported };
    blocking(move || match pr_target(&root) {
        (Some(branch), kind) => pr::view(&root, &branch, kind, refresh),
        // A detached HEAD is no branch to have a request.
        (None, kind) => match kind.and_then(pr::Cli::for_forge) {
            Some(cli) => PrState::None { cli },
            None => PrState::Unsupported,
        },
    })
    .await
    .unwrap_or(PrState::Unsupported)
}

#[tauri::command]
pub async fn git_pr_update(root: String, patch: PrPatch) -> Result<PrState, String> {
    let root = checked(&root).ok_or_else(|| tr(Msg::GitNotARepo))?;
    blocking(move || {
        let (branch, kind) = pr_target(&root);
        let branch = branch.ok_or_else(|| tr(Msg::GitDetached))?;
        pr::update(&root, &branch, kind, &patch)
    })
    .await
    .unwrap_or_else(|| Err(tr(Msg::PrFailed("?"))))
}

#[tauri::command]
pub async fn git_pr_create(root: String, new: NewPr) -> Result<PrState, String> {
    let root = checked(&root).ok_or_else(|| tr(Msg::GitNotARepo))?;
    blocking(move || {
        let (branch, kind) = pr_target(&root);
        let branch = branch.ok_or_else(|| tr(Msg::GitDetached))?;
        pr::create(&root, &branch, kind, &new)
    })
    .await
    .unwrap_or_else(|| Err(tr(Msg::PrFailed("?"))))
}

/// git, gh and glab: which are installed and signed in to (Preferences).
#[tauri::command]
pub async fn git_tools() -> Tools {
    blocking(pr::tools).await.unwrap_or_default()
}
