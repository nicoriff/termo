//! What can only be done where the agents run: starting and stopping runs,
//! answering what they ask, driving orchestrations. Everything else about
//! the agents (their definitions, their history) is files that any process
//! of Termo reads and writes.
//!
//! The agents run in the app, or — with "Keep agents running when Termo
//! closes" on — in a process of their own (see host/). Either way these go
//! through `call`, by name with their arguments as JSON: the app runs it on
//! its own agents, or hands it to that process, which runs the same `call`.

use std::path::Path;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::commands::agent_commands::{delete_agent, save_agent};
use crate::i18n::{tr, Msg};

use super::hub::Hub;
use super::orchestration::executor::{ensure_team, Orchestrator};
use super::orchestration::plan::{self, apply, edit_plan, max_autonomy, sanitize, Orchestration, Plan, RevisionState, Status, TaskStatus};
use super::orchestration::{planner, questions, report, store as orchestrations};
use super::projects;
use super::run_questions;
use super::runner::Runner;
use super::spec::AgentSpec;
use super::store::{self, now_millis, RunTrigger};
use super::webhook;

/// The agents where they run: who hears about them, and what runs them.
#[derive(Clone)]
pub struct Agents {
    pub hub: Hub,
    pub runner: Arc<Runner>,
    pub orchestrator: Arc<Orchestrator>,
}

fn args<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|err| format!("bad arguments: {err}"))
}

fn reply<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|err| err.to_string())
}

#[derive(Deserialize)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
struct Start {
    spec: AgentSpec,
    trigger: RunTrigger,
    #[serde(default)]
    context: Option<String>,
}

#[derive(Deserialize)]
struct Answer {
    question_id: String,
    answer: String,
}

#[derive(Deserialize)]
struct AgentId {
    agent_id: String,
}

#[derive(Deserialize)]
struct ProjectDelete {
    id: String,
    delete_agents: bool,
}

#[derive(Deserialize)]
struct ProjectRun {
    id: String,
    rehearsal: bool,
}

#[derive(Deserialize)]
struct Approve {
    id: String,
    chosen: String,
    plan: Plan,
    budget_usd: f64,
}

#[derive(Deserialize)]
struct EditPlan {
    id: String,
    plan: Plan,
}

#[derive(Deserialize)]
struct OrchestrationAnswer {
    id: String,
    question_id: String,
    answer: String,
}

#[derive(Deserialize)]
struct Resume {
    id: String,
    budget_usd: Option<f64>,
}

#[derive(Deserialize)]
struct ResolveRevision {
    id: String,
    revision_id: String,
    approve: bool,
}

/// Runs `name` with `args` on these agents.
pub fn call(agents: &Agents, name: &str, value: Value) -> Result<Value, String> {
    let root = agents.hub.root();
    match name {
        "agent_start" => {
            let Start { spec, trigger, context } = args(value)?;
            reply(agents.runner.start(&agents.hub, &spec, trigger, context.as_deref())?)
        }
        "agent_questions" => reply(run_questions::pending()),
        "agent_answer_question" => {
            let Answer { question_id, answer } = args(value)?;
            reply(run_questions::answer(&agents.hub, &question_id, &answer))
        }
        "agent_stop" => {
            let AgentId { agent_id } = args(value)?;
            agents.runner.stop(&agent_id);
            Ok(Value::Null)
        }
        "agents_running" => reply(agents.runner.running()),
        "agents_webhook" => Ok(json!({ "port": webhook::PORT, "listening": webhook::listening() })),
        "agents_delete" => {
            let Id { id } = args(value)?;
            reply(delete_agent(root, &agents.runner, &id)?)
        }
        "project_delete" => {
            let ProjectDelete { id, delete_agents } = args(value)?;
            reply(project_delete(agents, &id, delete_agents)?)
        }
        "project_run" => {
            let ProjectRun { id, rehearsal } = args(value)?;
            let project = projects::get_in(root, &id).ok_or_else(|| tr(Msg::ProjectNotFound))?;
            let entry = store::get_in(root, &project.entry_agent_id)
                .filter(|spec| spec.project_id == id)
                .ok_or_else(|| tr(Msg::ProjectNoEntry))?;
            reply(agents.runner.start_linked(&agents.hub, &entry, RunTrigger::Manual, None, None, rehearsal)?)
        }
        "project_stop" => {
            let Id { id } = args(value)?;
            for spec in projects::agents_of(root, &id) {
                agents.runner.stop(&spec.id);
            }
            Ok(Value::Null)
        }
        "orchestration_approve" => reply(approve(agents, args(value)?)?),
        "orchestration_edit_plan" => reply(edit(agents, args(value)?)?),
        "orchestration_answer" => reply(answer_orchestration(agents, args(value)?)?),
        "orchestration_stop" => {
            let Id { id } = args(value)?;
            agents.orchestrator.stop(&id);
            Ok(Value::Null)
        }
        "orchestration_resume" => reply(resume(agents, args(value)?)?),
        "orchestration_report" => {
            let Id { id } = args(value)?;
            reply(write_report(agents, &id)?)
        }
        "orchestration_resolve_revision" => reply(resolve_revision(agents, args(value)?)?),
        "orchestration_delete" => {
            let Id { id } = args(value)?;
            reply(delete_orchestration(agents, &id)?)
        }
        _ => Err(format!("unknown call: {name}")),
    }
}

/// Deletes a project, and its agents too when `delete_agents`; otherwise
/// they're left outside any project. Returns both lists.
fn project_delete(agents: &Agents, id: &str, delete_agents: bool) -> Result<(Vec<projects::Project>, Vec<AgentSpec>), String> {
    let root = agents.hub.root();
    for mut spec in projects::agents_of(root, id) {
        if delete_agents {
            delete_agent(root, &agents.runner, &spec.id)?;
        } else {
            spec.project_id = String::new();
            save_agent(root, spec)?;
        }
    }
    let list = projects::delete_in(root, id).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))?;
    Ok((list, store::list_in(root)))
}

fn orchestration(root: &Path, id: &str) -> Result<Orchestration, String> {
    orchestrations::get_in(root, id).ok_or_else(|| tr(Msg::OrchestrationNotFound))
}

fn saved(result: anyhow::Result<Orchestration>) -> Result<Orchestration, String> {
    result.map_err(|err| tr(Msg::OrchestrationSaveFailed(&err.to_string())))
}

/// Saves it, tells everyone and — when it's left running — starts it.
fn commit(agents: &Agents, orchestration: Orchestration) -> Result<Orchestration, String> {
    let orchestration = saved(orchestrations::save_in(agents.hub.root(), orchestration))?;
    agents.hub.emit("orchestration", &orchestration);
    if orchestration.status == Status::Running {
        agents.orchestrator.start(&agents.hub, &agents.runner, &orchestration.id);
    }
    Ok(orchestration)
}

/// Approves a draft as the user left it — the strategy picked, models
/// changed, the budget set — and starts it.
fn approve(agents: &Agents, Approve { id, chosen, plan, budget_usd }: Approve) -> Result<Orchestration, String> {
    let root = agents.hub.root();
    let mut orchestration = orchestration(root, &id)?;
    if orchestration.status != Status::Draft || agents.orchestrator.is_active(&id) {
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
    ensure_team(root, &mut orchestration)?;
    orchestration.status = Status::Running;
    orchestration.started_at = now_millis();
    commit(agents, orchestration)
}

/// Saves changes to the plan of one that isn't running — stopped, paused,
/// failed or done — keeping what has run (see plan::edit_plan), and updates
/// its team's agents to match. Resuming it picks up from there.
fn edit(agents: &Agents, EditPlan { id, plan }: EditPlan) -> Result<Orchestration, String> {
    let root = agents.hub.root();
    let mut orchestration = orchestration(root, &id)?;
    let editable = matches!(orchestration.status, Status::Stopped | Status::Paused | Status::Failed | Status::Done);
    if !editable || agents.orchestrator.is_active(&id) {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    orchestration.plan = edit_plan(&orchestration.plan, &plan, &planner::catalog());
    if orchestration.plan.tasks.is_empty() {
        return Err(tr(Msg::OrchestrationEmptyPlan));
    }
    // The user's own edit: how far its agents may go is what they left.
    orchestration.max_autonomy = max_autonomy(&orchestration.plan);
    ensure_team(root, &mut orchestration)?;
    commit(agents, orchestration)
}

/// The user's answer to what one of its agents asked; the agent, waiting on
/// it, goes on. Here, where the orchestration runs: its file is written by
/// whoever drives it, one change at a time.
fn answer_orchestration(agents: &Agents, OrchestrationAnswer { id, question_id, answer }: OrchestrationAnswer) -> Result<Orchestration, String> {
    if answer.trim().is_empty() {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    let mut answered = false;
    let updated = orchestrations::update_in(agents.hub.root(), &id, |orchestration| {
        answered = questions::answer(orchestration, &question_id, &answer);
    })
    .ok_or_else(|| tr(Msg::OrchestrationNotFound))?;
    if !answered {
        return Err(tr(Msg::OrchestrationQuestionClosed));
    }
    agents.hub.emit("orchestration", &updated);
    Ok(updated)
}

/// Picks up one that stopped, was paused over its budget or failed: what
/// failed gets to run again. `budget_usd` replaces the budget when given.
fn resume(agents: &Agents, Resume { id, budget_usd }: Resume) -> Result<Orchestration, String> {
    let mut orchestration = orchestration(agents.hub.root(), &id)?;
    let resumable = matches!(orchestration.status, Status::Stopped | Status::Paused | Status::Failed);
    if !resumable || agents.orchestrator.is_active(&id) {
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
    commit(agents, orchestration)
}

/// Has the orchestrator write (again) its report on how it went — for one
/// that was stopped or paused, or to replace the one it wrote.
fn write_report(agents: &Agents, id: &str) -> Result<Orchestration, String> {
    let root = agents.hub.root();
    let current = orchestration(root, id)?;
    let ended = matches!(current.status, Status::Done | Status::Failed | Status::Stopped | Status::Paused);
    if !ended || current.reporting || agents.orchestrator.is_active(id) {
        return Err(tr(Msg::OrchestrationWrongState));
    }
    let mark = |reporting: bool| {
        if let Some(orchestration) = orchestrations::update_in(root, id, |orchestration| orchestration.reporting = reporting) {
            agents.hub.emit("orchestration", &orchestration);
        }
    };
    mark(true);
    let report = match report::write(&current) {
        Ok(report) => report,
        Err(error) => {
            mark(false);
            return Err(error);
        }
    };
    let updated = orchestrations::update_in(root, id, |orchestration| {
        orchestration.reporting = false;
        orchestration.overhead_usd += report.cost_usd;
        if orchestration.ended_at == 0 {
            orchestration.ended_at = report.at;
        }
        orchestration.report = Some(report);
    })
    .ok_or_else(|| tr(Msg::OrchestrationNotFound))?;
    agents.hub.emit("orchestration", &updated);
    Ok(updated)
}

/// The user's answer to a change the orchestrator couldn't make on its own.
/// Approving it makes the change and raises what it needed raised — the
/// budget to what the plan is now expected to cost, or how much the agents
/// may do; rejecting it goes on with the plan as it was.
fn resolve_revision(agents: &Agents, ResolveRevision { id, revision_id, approve }: ResolveRevision) -> Result<Orchestration, String> {
    let root = agents.hub.root();
    let mut orchestration = orchestration(root, &id)?;
    let index = orchestration
        .revisions
        .iter()
        .position(|revision| revision.id == revision_id && revision.state == RevisionState::Pending)
        .filter(|_| orchestration.status == Status::AwaitingApproval && !agents.orchestrator.is_active(&id))
        .ok_or_else(|| tr(Msg::OrchestrationWrongState))?;
    if approve {
        let changed = apply(&orchestration.plan, &orchestration.revisions[index].changes, &planner::catalog());
        let expected = plan::projected_usd(&changed) + orchestration.overhead_usd;
        if orchestration.budget_usd > 0.0 && expected > orchestration.budget_usd {
            orchestration.budget_usd = (expected * 100.0).ceil() / 100.0;
        }
        orchestration.max_autonomy = orchestration.max_autonomy.max(max_autonomy(&changed));
        orchestration.plan = changed;
        ensure_team(root, &mut orchestration)?;
        orchestration.revisions[index].state = RevisionState::Applied;
    } else {
        orchestration.revisions[index].state = RevisionState::Rejected;
    }
    orchestration.note = String::new();
    orchestration.status = Status::Running;
    commit(agents, orchestration)
}

/// Deletes an orchestration with its team: the agents, their history and
/// the project that held them.
fn delete_orchestration(agents: &Agents, id: &str) -> Result<Vec<Orchestration>, String> {
    let root = agents.hub.root();
    agents.orchestrator.stop(id);
    if let Some(orchestration) = orchestrations::get_in(root, id) {
        if !orchestration.project_id.is_empty() {
            for spec in projects::agents_of(root, &orchestration.project_id) {
                delete_agent(root, &agents.runner, &spec.id)?;
            }
            let _ = projects::delete_in(root, &orchestration.project_id);
        }
    }
    orchestrations::delete_in(root, id).map_err(|err| tr(Msg::OrchestrationSaveFailed(&err.to_string())))?;
    Ok(orchestrations::list_in(root))
}
