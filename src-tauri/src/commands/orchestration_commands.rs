use serde_json::json;
use tauri::Emitter;

use crate::agents::draft::Step;
use crate::agents::host::client::dispatch;
use crate::agents::orchestration::plan::{Catalog, Orchestration, Plan, Status};
use crate::agents::orchestration::{planner, store};
use crate::agents::spec::Engine;
use crate::agents::store::now_millis;
use crate::commands::agent_commands::root;
use crate::i18n::{tr, Msg};

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

/// The engines and models a team can be made of on this computer, with
/// what each costs.
#[tauri::command]
pub async fn orchestration_catalog() -> Result<Catalog, String> {
    tauri::async_runtime::spawn_blocking(planner::catalog).await.map_err(|err| err.to_string())
}

/// Off the main thread: a window reads it as it starts, and there it would
/// hold up every other call.
#[tauri::command]
pub async fn orchestration_list(app: tauri::AppHandle) -> Result<Vec<Orchestration>, String> {
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
/// changed, the budget set — and starts it, where the agents run (see
/// agents/ops.rs, as for every change to one that may be running).
#[tauri::command]
pub async fn orchestration_approve(
    app: tauri::AppHandle,
    id: String,
    chosen: String,
    plan: Plan,
    budget_usd: f64,
) -> Result<Orchestration, String> {
    dispatch(app, "orchestration_approve", json!({ "id": id, "chosen": chosen, "plan": plan, "budget_usd": budget_usd })).await
}

/// Saves changes to the plan of one that isn't running — stopped, paused,
/// failed or done — keeping what has run (see plan::edit_plan), and updates
/// its team's agents to match. Resuming it picks up from there.
#[tauri::command]
pub async fn orchestration_edit_plan(app: tauri::AppHandle, id: String, plan: Plan) -> Result<Orchestration, String> {
    dispatch(app, "orchestration_edit_plan", json!({ "id": id, "plan": plan })).await
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
pub async fn orchestration_answer(app: tauri::AppHandle, id: String, question_id: String, answer: String) -> Result<Orchestration, String> {
    dispatch(app, "orchestration_answer", json!({ "id": id, "question_id": question_id, "answer": answer })).await
}

#[tauri::command]
pub async fn orchestration_stop(app: tauri::AppHandle, id: String) -> Result<(), String> {
    dispatch(app, "orchestration_stop", json!({ "id": id })).await
}

/// Picks up one that stopped, was paused over its budget or failed: what
/// failed gets to run again. `budget_usd` replaces the budget when given.
#[tauri::command]
pub async fn orchestration_resume(app: tauri::AppHandle, id: String, budget_usd: Option<f64>) -> Result<Orchestration, String> {
    dispatch(app, "orchestration_resume", json!({ "id": id, "budget_usd": budget_usd })).await
}

/// Has the orchestrator write (again) its report on how it went — for one
/// that was stopped or paused, or to replace the one it wrote.
#[tauri::command]
pub async fn orchestration_report(app: tauri::AppHandle, id: String) -> Result<Orchestration, String> {
    dispatch(app, "orchestration_report", json!({ "id": id })).await
}

/// The user's answer to a change the orchestrator couldn't make on its own.
#[tauri::command]
pub async fn orchestration_resolve_revision(
    app: tauri::AppHandle,
    id: String,
    revision_id: String,
    approve: bool,
) -> Result<Orchestration, String> {
    dispatch(app, "orchestration_resolve_revision", json!({ "id": id, "revision_id": revision_id, "approve": approve })).await
}

/// Deletes an orchestration with its team: the agents, their history and
/// the project that held them.
#[tauri::command]
pub async fn orchestration_delete(app: tauri::AppHandle, id: String) -> Result<Vec<Orchestration>, String> {
    dispatch(app, "orchestration_delete", json!({ "id": id })).await
}
