use tauri::{Emitter, State};

use crate::agents::orchestration::executor::ensure_team;
use crate::agents::orchestration::plan::{
    self, apply, edit_plan, max_autonomy, sanitize, Catalog, Orchestration, Plan, RevisionState, Status, TaskStatus,
};
use crate::agents::orchestration::{planner, questions, report, store};
use crate::agents::draft::Step;
use crate::agents::spec::Engine;
use crate::agents::store::now_millis;
use crate::agents::projects;
use crate::commands::agent_commands::{delete_agent, root};
use crate::i18n::{tr, Msg};
use crate::state::AppState;

/// A step of the orchestrator thinking — planning, asking, revising — for
/// the window waiting on it.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    /// "clarify", "plan" or "revise".
    phase: &'static str,
    /// The orchestration being revised; none while planning a new one.
    id: Option<String>,
    step: Step,
    at: i64,
}

/// Tells every window each step the orchestrator takes in `phase`.
fn progress(app: &tauri::AppHandle, phase: &'static str, id: Option<String>) -> impl Fn(Step) {
    let app = app.clone();
    move |step| {
        let _ = app.emit("orchestration-progress", Progress { phase, id: id.clone(), step, at: now_millis() });
    }
}

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

/// What the planner would ask about the request before planning; nothing
/// is saved.
#[tauri::command]
pub async fn orchestration_clarify(
    app: tauri::AppHandle,
    prompt: String,
    work_dir: String,
    engine: Option<Engine>,
    model: String,
) -> Result<planner::Clarifying, String> {
    let on_step = progress(&app, "clarify", None);
    let engine = engine.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || planner::clarify(&prompt, &work_dir, engine, &model, &on_step))
        .await
        .map_err(|err| tr(Msg::OrchestrationPlanFailed(&err.to_string())))?
}

/// Plans the work with AI — with what the user answered before, if asked —
/// and saves the plan as a draft to review: nothing runs until it's
/// approved. `overhead_usd`: what asking those questions cost.
#[tauri::command]
pub async fn orchestration_plan(
    app: tauri::AppHandle,
    prompt: String,
    work_dir: String,
    engine: Option<Engine>,
    model: String,
    answers: Option<Vec<planner::Answered>>,
    overhead_usd: Option<f64>,
) -> Result<Orchestration, String> {
    let root = root(&app)?;
    let request = planner::with_answers(&prompt, &answers.unwrap_or_default());
    let on_step = progress(&app, "plan", None);
    let engine = engine.unwrap_or_default();
    let mut planned =
        tauri::async_runtime::spawn_blocking(move || planner::generate(&request, &work_dir, engine, &model, &on_step))
        .await
        .map_err(|err| tr(Msg::OrchestrationPlanFailed(&err.to_string())))??;
    planned.overhead_usd += overhead_usd.filter(|usd| usd.is_finite() && *usd > 0.0).unwrap_or(0.0);
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
        task.failure = None;
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
    orchestration.started_at = now_millis();
    commit(&app, &state, &root, orchestration)
}

/// Saves changes to the plan of one that isn't running — stopped, paused,
/// failed or done — keeping what has run (see plan::edit_plan), and updates
/// its team's agents to match. Resuming it picks up from there.
#[tauri::command]
pub fn orchestration_edit_plan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    plan: Plan,
) -> Result<Orchestration, String> {
    let root = root(&app)?;
    let mut orchestration = orchestration(&root, &id)?;
    let editable = matches!(orchestration.status, Status::Stopped | Status::Paused | Status::Failed | Status::Done);
    if !editable || state.orchestrator.is_active(&id) {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    orchestration.plan = edit_plan(&orchestration.plan, &plan, &planner::catalog());
    if orchestration.plan.tasks.is_empty() {
        return Err(tr(Msg::OrchestrationEmptyPlan));
    }
    // The user's own edit: how far its agents may go is what they left.
    orchestration.max_autonomy = max_autonomy(&orchestration.plan);
    ensure_team(&root, &mut orchestration)?;
    commit(&app, &state, &root, orchestration)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisedPlan {
    plan: Plan,
    note: String,
    orchestration: Orchestration,
}

/// Asks the orchestrator to work the user's changes into the plan being
/// reviewed — a draft's, or one being edited after it ran. Nothing changes
/// until the user approves or saves what comes back; only what asking cost
/// is kept, with the rest of the orchestration's overhead.
#[tauri::command]
pub async fn orchestration_revise(
    app: tauri::AppHandle,
    id: String,
    plan: Plan,
    instructions: String,
) -> Result<RevisedPlan, String> {
    let root = root(&app)?;
    let current = orchestration(&root, &id)?;
    let reviewable = matches!(
        current.status,
        Status::Draft | Status::Stopped | Status::Paused | Status::Failed | Status::Done
    );
    if !reviewable || instructions.trim().is_empty() {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    let asked = current.clone();
    let on_step = progress(&app, "revise", Some(id.clone()));
    let (revised, note, cost) =
        tauri::async_runtime::spawn_blocking(move || planner::revise(&asked, &plan, instructions.trim(), &on_step))
            .await
            .map_err(|err| tr(Msg::OrchestrationPlanFailed(&err.to_string())))??;
    // Read again: it may have changed while the orchestrator was thinking.
    let mut orchestration = orchestration(&root, &id)?;
    orchestration.overhead_usd += cost;
    let orchestration = saved(store::save_in(&root, orchestration))?;
    let _ = app.emit("orchestration", &orchestration);
    Ok(RevisedPlan { plan: revised, note, orchestration })
}

/// The user's answer to what one of its agents asked; the agent, waiting on
/// it, goes on.
#[tauri::command]
pub fn orchestration_answer(app: tauri::AppHandle, id: String, question_id: String, answer: String) -> Result<Orchestration, String> {
    let root = root(&app)?;
    if answer.trim().is_empty() {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    let mut answered = false;
    let updated = store::update_in(&root, &id, |orchestration| {
        answered = questions::answer(orchestration, &question_id, &answer);
    })
    .ok_or_else(|| tr(Msg::OrchestrationNotFound))?;
    if !answered {
        return Err(tr(Msg::OrchestrationQuestionClosed));
    }
    let _ = app.emit("orchestration", &updated);
    Ok(updated)
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
    // A new end gets a new report.
    orchestration.report = None;
    orchestration.status = Status::Running;
    commit(&app, &state, &root, orchestration)
}

/// Has the orchestrator write (again) its report on how it went — for one
/// that was stopped or paused, or to replace the one it wrote.
#[tauri::command]
pub async fn orchestration_report(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Orchestration, String> {
    let root = root(&app)?;
    let current = orchestration(&root, &id)?;
    let ended = matches!(current.status, Status::Done | Status::Failed | Status::Stopped | Status::Paused);
    if !ended || current.reporting || state.orchestrator.is_active(&id) {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    let mark = |reporting: bool| {
        store::update_in(&root, &id, |orchestration| orchestration.reporting = reporting)
            .inspect(|orchestration| {
                let _ = app.emit("orchestration", orchestration);
            })
    };
    mark(true);
    let written = tauri::async_runtime::spawn_blocking(move || report::write(&current))
        .await
        .map_err(|err| tr(Msg::OrchestrationReportFailed(&err.to_string())))
        .and_then(|written| written);
    let report = match written {
        Ok(report) => report,
        Err(error) => {
            mark(false);
            return Err(error);
        }
    };
    let updated = store::update_in(&root, &id, |orchestration| {
        orchestration.reporting = false;
        orchestration.overhead_usd += report.cost_usd;
        if orchestration.ended_at == 0 {
            orchestration.ended_at = report.at;
        }
        orchestration.report = Some(report);
    })
    .ok_or_else(|| tr(Msg::OrchestrationNotFound))?;
    let _ = app.emit("orchestration", &updated);
    Ok(updated)
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
