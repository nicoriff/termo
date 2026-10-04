use tauri::{Emitter, State};

use crate::agents::orchestration::executor::ensure_team;
use crate::agents::orchestration::plan::{
    self, apply, max_autonomy, sanitize, Catalog, Orchestration, Plan, RevisionState, Status, TaskStatus,
};
use crate::agents::orchestration::{planner, store};
use crate::agents::projects;
use crate::commands::agent_commands::{delete_agent, root};
use crate::i18n::{tr, Msg};
use crate::state::AppState;

fn orchestration(root: &std::path::Path, id: &str) -> Result<Orchestration, String> {
    store::get_in(root, id).ok_or_else(|| tr(Msg::OrchestrationNotFound))
}

fn saved(result: anyhow::Result<Orchestration>) -> Result<Orchestration, String> {
    result.map_err(|err| tr(Msg::OrchestrationSaveFailed(&err.to_string())))
}

/// Saves it, tells every window and — when it's left running — starts it.
fn commit(app: &tauri::AppHandle, state: &AppState, root: &std::path::Path, orchestration: Orchestration) -> Result<Orchestration, String> {
    let orchestration = saved(store::save_in(root, orchestration))?;
    let _ = app.emit("orchestration", &orchestration);
    if orchestration.status == Status::Running {
        state.orchestrator.start(app, &state.agents, &orchestration.id);
    }
    Ok(orchestration)
}

/// The engines and models a team can be made of on this computer, with
/// what each costs.
#[tauri::command]
pub async fn orchestration_catalog() -> Result<Catalog, String> {
    tauri::async_runtime::spawn_blocking(planner::catalog).await.map_err(|err| err.to_string())
}

#[tauri::command]
pub fn orchestration_list(app: tauri::AppHandle) -> Result<Vec<Orchestration>, String> {
    Ok(store::list_in(&root(&app)?))
}

/// Plans the work with AI and saves the plan as a draft to review: nothing
/// runs until it's approved.
#[tauri::command]
pub async fn orchestration_plan(
    app: tauri::AppHandle,
    prompt: String,
    work_dir: String,
    model: String,
) -> Result<Orchestration, String> {
    let root = root(&app)?;
    let scratch = root.join(".draft");
    let planned = tauri::async_runtime::spawn_blocking(move || planner::generate(&scratch, &prompt, &work_dir, &model))
        .await
        .map_err(|err| tr(Msg::OrchestrationPlanFailed(&err.to_string())))??;
    let orchestration = saved(store::save_in(&root, planned))?;
    let _ = app.emit("orchestration", &orchestration);
    Ok(orchestration)
}

/// Approves a draft as the user left it — the strategy picked, models
/// changed, the budget set — and starts it.
#[tauri::command]
pub fn orchestration_approve(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    chosen: String,
    plan: Plan,
    budget_usd: f64,
) -> Result<Orchestration, String> {
    let root = root(&app)?;
    let mut orchestration = orchestration(&root, &id)?;
    if orchestration.status != Status::Draft || state.orchestrator.is_active(&id) {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    // The plan comes back from the review screen: checked again, and as
    // something that hasn't run.
    let mut plan = sanitize(plan, &planner::catalog());
    for agent in &mut plan.agents {
        agent.agent_id = String::new();
    }
    for task in &mut plan.tasks {
        task.status = TaskStatus::Pending;
        task.run_id = String::new();
        task.summary = String::new();
        task.cost_usd = None;
        task.attempts = 0;
    }
    if plan.tasks.is_empty() {
        return Err(tr(Msg::OrchestrationEmptyPlan));
    }
    orchestration.chosen = chosen;
    orchestration.max_autonomy = max_autonomy(&plan);
    orchestration.plan = plan;
    orchestration.budget_usd = if budget_usd.is_finite() { budget_usd.max(0.0) } else { 0.0 };
    orchestration.note = String::new();
    ensure_team(&root, &mut orchestration)?;
    orchestration.status = Status::Running;
    commit(&app, &state, &root, orchestration)
}

#[tauri::command]
pub fn orchestration_stop(state: State<'_, AppState>, id: String) {
    state.orchestrator.stop(&id);
}

/// Picks up one that stopped, was paused over its budget or failed: what
/// failed gets to run again. `budget_usd` replaces the budget when given.
#[tauri::command]
pub fn orchestration_resume(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    budget_usd: Option<f64>,
) -> Result<Orchestration, String> {
    let root = root(&app)?;
    let mut orchestration = orchestration(&root, &id)?;
    let resumable = matches!(orchestration.status, Status::Stopped | Status::Paused | Status::Failed);
    if !resumable || state.orchestrator.is_active(&id) {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    if let Some(budget) = budget_usd.filter(|budget| budget.is_finite()) {
        orchestration.budget_usd = budget.max(0.0);
    }
    for task in &mut orchestration.plan.tasks {
        if matches!(task.status, TaskStatus::Failed | TaskStatus::Running) {
            task.status = TaskStatus::Pending;
            task.attempts = 0;
        }
    }
    orchestration.note = String::new();
    orchestration.status = Status::Running;
    commit(&app, &state, &root, orchestration)
}

/// The user's answer to a change the orchestrator couldn't make on its own.
/// Approving it makes the change and raises what it needed raised — the
/// budget to what the plan is now expected to cost, or how much the agents
/// may do; rejecting it goes on with the plan as it was.
#[tauri::command]
pub fn orchestration_resolve_revision(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    revision_id: String,
    approve: bool,
) -> Result<Orchestration, String> {
    let root = root(&app)?;
    let mut orchestration = orchestration(&root, &id)?;
    let index = orchestration
        .revisions
        .iter()
        .position(|revision| revision.id == revision_id && revision.state == RevisionState::Pending)
        .filter(|_| orchestration.status == Status::AwaitingApproval && !state.orchestrator.is_active(&id))
        .ok_or_else(|| tr(Msg::OrchestrationWrongState))?;
    if approve {
        let changed = apply(&orchestration.plan, &orchestration.revisions[index].changes, &planner::catalog());
        let expected = plan::projected_usd(&changed) + orchestration.overhead_usd;
        if orchestration.budget_usd > 0.0 && expected > orchestration.budget_usd {
            orchestration.budget_usd = (expected * 100.0).ceil() / 100.0;
        }
        orchestration.max_autonomy = orchestration.max_autonomy.max(max_autonomy(&changed));
        orchestration.plan = changed;
        ensure_team(&root, &mut orchestration)?;
        orchestration.revisions[index].state = RevisionState::Applied;
    } else {
        orchestration.revisions[index].state = RevisionState::Rejected;
    }
    orchestration.note = String::new();
    orchestration.status = Status::Running;
    commit(&app, &state, &root, orchestration)
}

/// Deletes an orchestration with its team: the agents, their history and
/// the project that held them.
#[tauri::command]
pub fn orchestration_delete(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<Vec<Orchestration>, String> {
    let root = root(&app)?;
    state.orchestrator.stop(&id);
    if let Some(orchestration) = store::get_in(&root, &id) {
        if !orchestration.project_id.is_empty() {
            for spec in projects::agents_of(&root, &orchestration.project_id) {
                delete_agent(&root, &state.agents, &spec.id)?;
            }
            let _ = projects::delete_in(&root, &orchestration.project_id);
        }
    }
    store::delete_in(&root, &id).map_err(|err| tr(Msg::OrchestrationSaveFailed(&err.to_string())))?;
    Ok(store::list_in(&root))
}
