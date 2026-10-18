//! What the Git panel, the status bar's item and the diff and graph tabs ask
//! of a repository (see src-tauri/src/git). Every one of them takes the
//! repository's root, checked to be one before git is run there.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::agents::draft::Step;
use crate::agents::spec::{AgentSpec, Autonomy, GitRole};
use crate::agents::store::{self, RunRecord, RunTrigger};
use crate::git::forge::ForgeKind;
use crate::git::graph::{self, Branch, CommitDetail, GraphCommit, GraphScope};
use crate::git::ops::{Context, GitOp};
use crate::git::pr::{self, NewPr, PrPatch, PrState, Tools};
use crate::git::review::{self, Review, ReviewRequest, Reviewer, Stored};
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

/// Where the reviews of every repository are kept.
fn reviews_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|err| err.to_string())?.join("git-reviews"))
}

/// A step of a review being worked out, for the panel that asked for it.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ReviewStep {
    root: String,
    step: Step,
}

/// Who reviews: the agent of the user's that was named, as far as its
/// engine, model and instructions go, or the engine given. An agent that's
/// gone since it was picked, or isn't one made to review any more, leaves
/// the built-in reviewer.
fn reviewer(app: &tauri::AppHandle, request: &ReviewRequest) -> Reviewer {
    let named = request.agent_id.as_deref().map(str::trim).filter(|id| !id.is_empty());
    let agent = named.and_then(|id| store::get_in(&store::root(app).ok()?, id)).filter(|agent| agent.git_role == GitRole::Reviewer);
    match agent {
        Some(agent) => Reviewer { engine: agent.engine, model: agent.model, instructions: agent.instructions },
        None => Reviewer { engine: request.engine, ..Reviewer::default() },
    }
}

/// Reviews what `request` names, to the end: what the agent does on the way
/// comes as `git-review-step` events, and what it found is kept (see
/// git/review.rs). One at a time per repository.
#[tauri::command]
pub async fn git_review_start(app: tauri::AppHandle, root: String, request: ReviewRequest) -> Result<Review, String> {
    checked(&root).ok_or_else(|| tr(Msg::GitNotARepo))?;
    let dir = reviews_dir(&app)?;
    let reviews = app.state::<AppState>().git_reviews.clone();
    let reviewer = reviewer(&app, &request);
    blocking(move || {
        let on_step = |step: Step| {
            let _ = app.emit("git-review-step", ReviewStep { root: root.clone(), step });
        };
        reviews.run(&dir, &root, &request, &reviewer, &on_step)
    })
    .await
    .unwrap_or_else(|| Err(tr(Msg::GitReviewFailed("?"))))
}

#[tauri::command]
pub fn git_review_cancel(state: tauri::State<'_, AppState>, root: String) {
    state.git_reviews.cancel(&root);
}

/// The repositories with a review under way.
#[tauri::command]
pub fn git_review_busy(state: tauri::State<'_, AppState>) -> Vec<String> {
    state.git_reviews.busy()
}

/// The repository's reviews, newest first, and its note.
#[tauri::command]
pub async fn git_review_load(app: tauri::AppHandle, root: String) -> Result<Stored, String> {
    let dir = reviews_dir(&app)?;
    Ok(blocking(move || review::load(&dir, &root)).await.unwrap_or_default())
}

#[tauri::command]
pub async fn git_review_dismiss(app: tauri::AppHandle, root: String, review_id: String, finding_id: String, dismissed: bool) -> Result<Stored, String> {
    let dir = reviews_dir(&app)?;
    blocking(move || review::dismiss(&dir, &root, &review_id, &finding_id, dismissed)).await.unwrap_or_else(|| Err(tr(Msg::GitReviewGone)))
}

#[tauri::command]
pub async fn git_review_clear(app: tauri::AppHandle, root: String, review_id: String) -> Result<Stored, String> {
    let dir = reviews_dir(&app)?;
    blocking(move || review::clear(&dir, &root, &review_id)).await.unwrap_or_else(|| Err(tr(Msg::GitReviewGone)))
}

#[tauri::command]
pub async fn git_review_set_note(app: tauri::AppHandle, root: String, note: String) -> Result<Stored, String> {
    let dir = reviews_dir(&app)?;
    blocking(move || review::set_note(&dir, &root, &note)).await.unwrap_or_else(|| Err(tr(Msg::GitReviewGone)))
}

/// The comment `git_review_publish` would leave, to show before it does.
#[tauri::command]
pub async fn git_review_comment(app: tauri::AppHandle, root: String, review_id: String, finding_ids: Vec<String>) -> Result<String, String> {
    let dir = reviews_dir(&app)?;
    let review = blocking(move || review::review(&dir, &root, &review_id)).await.flatten().ok_or_else(|| tr(Msg::GitReviewGone))?;
    Ok(review::comment_markdown(&review, &finding_ids))
}

/// Leaves the findings picked as a comment on the branch's pull request.
#[tauri::command]
pub async fn git_review_publish(app: tauri::AppHandle, root: String, review_id: String, finding_ids: Vec<String>) -> Result<(), String> {
    let path = checked(&root).ok_or_else(|| tr(Msg::GitNotARepo))?;
    let dir = reviews_dir(&app)?;
    blocking(move || {
        let review = review::review(&dir, &root, &review_id).ok_or_else(|| tr(Msg::GitReviewGone))?;
        let (branch, kind) = pr_target(&path);
        let branch = branch.ok_or_else(|| tr(Msg::GitDetached))?;
        if !matches!(pr::view(&path, &branch, kind, false), PrState::Ok { .. }) {
            return Err(tr(Msg::GitReviewNoPr));
        }
        pr::comment(&path, kind, &review::comment_markdown(&review, &finding_ids))?;
        // Its count of comments is one more now.
        pr::forget(&path, &branch);
        Ok(())
    })
    .await
    .unwrap_or_else(|| Err(tr(Msg::PrFailed("?"))))
}

/// The agent a finding is handed to, as it runs for that: in the
/// repository, asked to fix (see `review::FIX_TASK`) in place of its own
/// task. Everything else of it is its own. Only one made to fix is.
pub fn fixing_spec(agent: AgentSpec, root: &str) -> Result<AgentSpec, String> {
    if agent.git_role != GitRole::Fixer {
        return Err(tr(Msg::GitReviewAgentGone));
    }
    if agent.autonomy == Autonomy::Read {
        return Err(tr(Msg::GitReviewFixNeedsEdit));
    }
    Ok(AgentSpec { work_dir: root.to_string(), task: review::FIX_TASK.to_string(), ..agent })
}

/// Hands a finding to one of the user's agents to fix: a run of that agent
/// like any other (see agents/runner.rs), in the repository.
#[tauri::command]
pub async fn git_review_fix(
    app: tauri::AppHandle,
    root: String,
    review_id: String,
    finding_id: String,
    agent_id: String,
) -> Result<RunRecord, String> {
    checked(&root).ok_or_else(|| tr(Msg::GitNotARepo))?;
    let (_, finding) = review::finding(&reviews_dir(&app)?, &root, &review_id, &finding_id).ok_or_else(|| tr(Msg::GitReviewGone))?;
    let agents = store::root(&app).map_err(|err| tr(Msg::GitReviewFailed(&err.to_string())))?;
    let agent = store::get_in(&agents, &agent_id).ok_or_else(|| tr(Msg::GitReviewAgentGone))?;
    let spec = fixing_spec(agent, &root)?;
    let args = serde_json::json!({ "spec": spec, "trigger": RunTrigger::Manual, "context": review::fix_context(&finding) });
    crate::agents::host::client::dispatch(app, "agent_start", args).await
}

/// git, gh and glab: which are installed and signed in to (Preferences).
#[tauri::command]
pub async fn git_tools() -> Tools {
    blocking(pr::tools).await.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_fixes_in_the_repository_with_the_fix_as_its_task() {
        let agent = AgentSpec {
            id: "a1".into(),
            task: "Summarize the week".into(),
            instructions: "Be careful.".into(),
            work_dir: "/elsewhere".into(),
            autonomy: Autonomy::Edit,
            git_role: GitRole::Fixer,
            ..AgentSpec::default()
        };
        let spec = fixing_spec(agent.clone(), "/repo").unwrap();
        assert_eq!((spec.work_dir.as_str(), spec.task.as_str()), ("/repo", review::FIX_TASK));
        // Who it is and how it works stay its own.
        assert_eq!((spec.id.as_str(), spec.instructions.as_str(), spec.autonomy), ("a1", "Be careful.", Autonomy::Edit));
        // One that can only look can't fix.
        let reader = AgentSpec { autonomy: Autonomy::Read, ..agent.clone() };
        assert_eq!(fixing_spec(reader, "/repo").unwrap_err(), tr(Msg::GitReviewFixNeedsEdit));
        // Nor one that wasn't made to fix, whatever it may do.
        for git_role in [GitRole::None, GitRole::Reviewer] {
            let other = AgentSpec { git_role, ..agent.clone() };
            assert_eq!(fixing_spec(other, "/repo").unwrap_err(), tr(Msg::GitReviewAgentGone));
        }
        // An agent saved before there were roles has none.
        let old: AgentSpec = serde_json::from_str(r#"{"id":"a2","name":"Old"}"#).unwrap();
        assert_eq!(old.git_role, GitRole::None);
        let kept: AgentSpec = serde_json::from_str(&serde_json::to_string(&agent).unwrap()).unwrap();
        assert_eq!(kept.git_role, GitRole::Fixer);
    }
}
