//! Running an approved plan: one thread per orchestration that starts each
//! task when it can (plan::ready), follows its run through the same runner
//! every agent uses, and at every pause — a round of tasks done, one failed —
//! asks the orchestrator what's next (supervisor.rs). The saved file is the
//! state: the thread reads it, changes it and writes it back, so stopping,
//! resuming and approving a change are edits to that file plus a (re)start.
//!
//! Each change reaches the frontend through the `orchestration` event, with
//! the orchestration as saved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::Emitter;

use crate::commands::agent_commands::save_agent;
use crate::i18n::{tr, Msg};

use super::super::project_ai::clip;
use super::super::projects::{self, Project};
use super::super::runner::Runner;
use super::super::spec::AgentSpec;
use super::super::store::{self as agent_store, now_millis, RunStatus, RunTrigger};
use super::plan::{
    self, agent_of, apply, hold_for, Hold, Orchestration, PlannedTask, Revision, RevisionState, Status, TaskStatus,
};
use super::planner::catalog;
use super::store;
use super::supervisor::{self, Verdict};

const TICK: Duration = Duration::from_millis(700);
/// Work on a codebase takes longer than an agent's usual errand.
const TASK_TIMEOUT_MIN: u32 = 45;
/// How much of an earlier task's answer the next one gets.
const HANDOVER: usize = 6000;

/// The orchestrations running now, each with the flag that stops it.
#[derive(Default)]
pub struct Orchestrator {
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

fn emit(app: &tauri::AppHandle, orchestration: &Orchestration) {
    let _ = app.emit("orchestration", orchestration);
}

fn money(usd: f64) -> String {
    format!("${usd:.2}")
}

/// Saves the plan's team as a project of agents (see projects.rs) — new
/// agents created, the ones already there brought up to date with the plan
/// (a changed model, say) — so its runs, history and costs live where every
/// agent's do. Fills in each agent's `agent_id`.
pub fn ensure_team(root: &Path, orchestration: &mut Orchestration) -> Result<(), String> {
    let save_failed = |err: String| tr(Msg::OrchestrationSaveFailed(&err));
    if orchestration.project_id.is_empty() {
        orchestration.project_id = uuid::Uuid::new_v4().to_string();
    }
    if projects::get_in(root, &orchestration.project_id).is_none() {
        let project = Project {
            id: orchestration.project_id.clone(),
            name: orchestration.name.clone(),
            icon: orchestration.icon.clone(),
            goal: orchestration.prompt.clone(),
            work_dir: orchestration.work_dir.clone(),
            // Its agents keep to themselves, with a board of their own.
            isolated: true,
            orchestration_id: orchestration.id.clone(),
            ..Project::default()
        };
        projects::save_in(root, project).map_err(|err| save_failed(err.to_string()))?;
    }
    for agent in &mut orchestration.plan.agents {
        let saved = (!agent.agent_id.is_empty()).then(|| agent_store::get_in(root, &agent.agent_id)).flatten();
        let mut spec = saved.unwrap_or_else(|| AgentSpec {
            id: uuid::Uuid::new_v4().to_string(),
            // Nothing starts it but its orchestration.
            enabled: false,
            ..AgentSpec::default()
        });
        spec.name = agent.name.clone();
        spec.description = agent.specialty.clone();
        spec.icon = agent.icon.clone();
        spec.engine = agent.engine;
        spec.model = agent.model.clone();
        spec.work_dir = orchestration.work_dir.clone();
        spec.task = tr(Msg::OrchestrationAgentTask);
        spec.instructions = agent.instructions.clone();
        spec.autonomy = agent.autonomy;
        spec.tools.web = agent.web;
        spec.limits.timeout_min = TASK_TIMEOUT_MIN;
        spec.project_id = orchestration.project_id.clone();
        agent.agent_id = spec.id.clone();
        save_agent(root, spec)?;
    }
    Ok(())
}

/// What the agent is asked for this task, and what it gets as context: the
/// answers of the tasks it builds on, and its own last attempt when this is
/// a retry. The context is other agents' text: the runner fences it as data
/// (see spec::with_context).
fn request_for(orchestration: &Orchestration, task: &PlannedTask) -> (String, Option<String>) {
    let mut request = task.description.clone();
    if !task.done_when.is_empty() {
        request.push_str(&format!("\n\n{} {}", tr(Msg::OrchestrationDoneWhen), task.done_when));
    }
    let plan = &orchestration.plan;
    let mut context = String::new();
    let earlier: Vec<&PlannedTask> = task
        .depends_on
        .iter()
        .filter_map(|dep| plan.tasks.iter().find(|other| other.key == *dep))
        .filter(|other| other.status == TaskStatus::Done && !other.summary.trim().is_empty())
        .collect();
    if !earlier.is_empty() {
        context.push_str(&tr(Msg::OrchestrationBuildsOn));
        for other in earlier {
            let by = agent_of(plan, other).map_or("", |agent| agent.name.as_str());
            context.push_str(&format!("\n\n## {} ({by})\n{}", other.title, clip(other.summary.trim(), HANDOVER)));
        }
    }
    if task.attempts > 0 && !task.summary.trim().is_empty() {
        if !context.is_empty() {
            context.push_str("\n\n");
        }
        context.push_str(&format!("{}\n{}", tr(Msg::OrchestrationLastAttempt), clip(task.summary.trim(), HANDOVER)));
    }
    (request, (!context.is_empty()).then_some(context))
}

/// Left as it is when Termo closed mid-run: nothing is running it any more.
pub fn recover(app: &tauri::AppHandle) {
    let Ok(root) = agent_store::root(app) else { return };
    for orchestration in store::list_in(&root).into_iter().filter(|orchestration| orchestration.status == Status::Running) {
        store::update_in(&root, &orchestration.id, |orchestration| {
            release_running(orchestration);
            orchestration.status = Status::Stopped;
            orchestration.note = tr(Msg::OrchestrationInterrupted);
        });
    }
}

/// Tasks that were running go back to waiting: their runs are gone.
fn release_running(orchestration: &mut Orchestration) {
    for task in &mut orchestration.plan.tasks {
        if task.status == TaskStatus::Running {
            task.status = TaskStatus::Pending;
            task.attempts = task.attempts.saturating_sub(1);
        }
    }
}

impl Orchestrator {
    pub fn is_active(&self, id: &str) -> bool {
        self.active.lock().unwrap().contains_key(id)
    }

    /// Asks its thread to stop: it stops the running agents and leaves the
    /// orchestration as stopped.
    pub fn stop(&self, id: &str) {
        if let Some(flag) = self.active.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Runs the orchestration from where its file says it is, until it ends,
    /// stops or needs the user. Nothing happens if it's already running.
    pub fn start(self: &Arc<Self>, app: &tauri::AppHandle, runner: &Arc<Runner>, id: &str) {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active.lock().unwrap();
            if active.contains_key(id) {
                return;
            }
            active.insert(id.to_string(), stop.clone());
        }
        let Ok(root) = agent_store::root(app) else {
            self.active.lock().unwrap().remove(id);
            return;
        };
        let run = Run { app: app.clone(), runner: runner.clone(), root, id: id.to_string(), stop };
        let orchestrator = self.clone();
        std::thread::spawn(move || {
            run.drive();
            orchestrator.active.lock().unwrap().remove(&run.id);
            // A stop that arrived as the thread was ending still shows.
            if let Some(orchestration) = store::get_in(&run.root, &run.id) {
                emit(&run.app, &orchestration);
            }
        });
    }
}

struct Run {
    app: tauri::AppHandle,
    runner: Arc<Runner>,
    root: PathBuf,
    id: String,
    stop: Arc<AtomicBool>,
}

impl Run {
    fn update(&self, change: impl FnOnce(&mut Orchestration)) -> Option<Orchestration> {
        let orchestration = store::update_in(&self.root, &self.id, change)?;
        emit(&self.app, &orchestration);
        Some(orchestration)
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// Stops the agents that are running and leaves it as stopped.
    fn halt(&self, orchestration: &Orchestration) {
        for task in orchestration.plan.tasks.iter().filter(|task| task.status == TaskStatus::Running) {
            if let Some(agent) = agent_of(&orchestration.plan, task) {
                self.runner.stop(&agent.agent_id);
            }
        }
        self.update(|orchestration| {
            release_running(orchestration);
            orchestration.status = Status::Stopped;
            orchestration.note = tr(Msg::OrchestrationStopped);
        });
    }

    /// Takes in the runs that ended since the last look. Returns whether
    /// any did.
    fn collect(&self, orchestration: &Orchestration) -> bool {
        let mut ended: Vec<(String, TaskStatus, String, Option<f64>)> = Vec::new();
        for task in orchestration.plan.tasks.iter().filter(|task| task.status == TaskStatus::Running) {
            let Some(agent) = agent_of(&orchestration.plan, task) else { continue };
            if self.runner.is_running_run(&agent.agent_id, &task.run_id) {
                continue;
            }
            let run = agent_store::get_run_in(&self.root, &agent.agent_id, &task.run_id);
            let (status, summary, cost) = match run {
                Some(run) if run.status == RunStatus::Success => (TaskStatus::Done, run.summary, run.cost_usd),
                Some(run) => (TaskStatus::Failed, run.summary, run.cost_usd),
                None => (TaskStatus::Failed, String::new(), None),
            };
            ended.push((task.key.clone(), status, summary, cost));
        }
        if ended.is_empty() {
            return false;
        }
        self.update(|orchestration| {
            for (key, status, summary, cost) in ended {
                let Some(task) = orchestration.plan.tasks.iter_mut().find(|task| task.key == key) else { continue };
                task.status = status;
                task.summary = summary;
                if let Some(cost) = cost {
                    task.cost_usd = Some(task.cost_usd.unwrap_or(0.0) + cost);
                }
            }
        });
        true
    }

    /// Starts every task that can start now.
    fn launch(&self, orchestration: &Orchestration) {
        for key in plan::ready(&orchestration.plan) {
            let Some(task) = orchestration.plan.tasks.iter().find(|task| task.key == key) else { continue };
            let spec = agent_of(&orchestration.plan, task).and_then(|agent| agent_store::get_in(&self.root, &agent.agent_id));
            let started = match spec {
                Some(mut spec) => {
                    let (request, context) = request_for(orchestration, task);
                    spec.task = request;
                    self.runner.start_linked(&self.app, &spec, RunTrigger::Orchestrator, context.as_deref(), None, false)
                }
                None => Err(tr(Msg::AgentRunFailed("agent missing"))),
            };
            self.update(|orchestration| {
                let Some(task) = orchestration.plan.tasks.iter_mut().find(|task| task.key == key) else { return };
                task.attempts += 1;
                match started {
                    Ok(run) => {
                        task.status = TaskStatus::Running;
                        task.run_id = run.id;
                    }
                    Err(error) => {
                        task.status = TaskStatus::Failed;
                        task.summary = error;
                    }
                }
            });
        }
    }

    /// Asks the orchestrator what's next and does it. Returns whether the
    /// run goes on.
    fn review(&self, orchestration: &Orchestration) -> bool {
        let scratch = self.root.join(".draft");
        let catalog = catalog();
        let review = supervisor::review(&scratch, orchestration, &catalog);
        if self.stopped() {
            return true;
        }
        let failed = orchestration.plan.tasks.iter().any(|task| task.status == TaskStatus::Failed);
        let done = plan::finished(&orchestration.plan);
        let review = match review {
            Ok(review) => review,
            // Without its orchestrator the plan goes on as it is, as far as
            // it can.
            Err(error) => {
                let note = tr(Msg::OrchestrationNoReview(&error));
                self.update(|orchestration| {
                    orchestration.note = note;
                    if done {
                        orchestration.status = Status::Done;
                    }
                });
                return !done;
            }
        };
        let cost = review.cost_usd.unwrap_or(0.0);
        match review.verdict {
            Verdict::Abort => {
                self.update(|orchestration| {
                    orchestration.overhead_usd += cost;
                    orchestration.status = Status::Failed;
                    orchestration.note = review.note;
                });
                false
            }
            Verdict::Continue => {
                self.update(|orchestration| {
                    orchestration.overhead_usd += cost;
                    if done {
                        orchestration.status = Status::Done;
                        orchestration.note = review.note;
                    } else if !review.note.is_empty() && !failed {
                        orchestration.note = review.note;
                    }
                });
                !done
            }
            Verdict::Revise => {
                let changed = apply(&orchestration.plan, &review.changes, &catalog);
                let hold = hold_for(orchestration, &changed);
                let mut held = false;
                let root = self.root.clone();
                self.update(|orchestration| {
                    orchestration.overhead_usd += cost;
                    let mut revision = Revision {
                        id: uuid::Uuid::new_v4().to_string(),
                        at: now_millis(),
                        note: review.note.clone(),
                        changes: review.changes,
                        state: RevisionState::Applied,
                        hold,
                        estimate_usd: plan::projected_usd(&changed) + orchestration.overhead_usd,
                    };
                    if hold == Hold::None {
                        let before = orchestration.plan.clone();
                        orchestration.plan = changed;
                        // A team that can't be saved isn't one to run.
                        if ensure_team(&root, orchestration).is_err() {
                            orchestration.plan = before;
                            revision.state = RevisionState::Rejected;
                        }
                    } else {
                        revision.state = RevisionState::Pending;
                        orchestration.status = Status::AwaitingApproval;
                        held = true;
                    }
                    orchestration.note = review.note;
                    orchestration.revisions.push(revision);
                });
                !held
            }
        }
    }

    fn drive(&self) {
        // Picked up with everything done: only the final review is left.
        let mut unreviewed = store::get_in(&self.root, &self.id).is_some_and(|orchestration| plan::finished(&orchestration.plan));
        loop {
            // Deleted, or no longer running (it waits for the user).
            let Some(orchestration) = store::get_in(&self.root, &self.id) else { return };
            if self.stopped() {
                self.halt(&orchestration);
                return;
            }
            if orchestration.status != Status::Running {
                return;
            }
            if self.collect(&orchestration) {
                unreviewed = true;
                continue;
            }
            let running = orchestration.plan.tasks.iter().any(|task| task.status == TaskStatus::Running);
            if plan::over_budget(&orchestration) {
                // What's running finishes; nothing new starts.
                if !running {
                    let spent = money(plan::spent_usd(&orchestration.plan) + orchestration.overhead_usd);
                    let budget = money(orchestration.budget_usd);
                    self.update(|orchestration| {
                        orchestration.status = Status::Paused;
                        orchestration.note = tr(Msg::OrchestrationOverBudget(&spent, &budget));
                    });
                    return;
                }
            } else if !running && unreviewed {
                unreviewed = false;
                if !self.review(&orchestration) {
                    return;
                }
                continue;
            } else {
                let ready = plan::ready(&orchestration.plan);
                if !ready.is_empty() {
                    self.launch(&orchestration);
                    continue;
                }
                if !running {
                    // Nothing runs, nothing can start and it isn't done:
                    // what's left waits on tasks that failed.
                    self.update(|orchestration| {
                        orchestration.status = Status::Failed;
                        orchestration.note = tr(Msg::OrchestrationBlocked);
                    });
                    return;
                }
            }
            std::thread::sleep(TICK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::spec::{Autonomy, Engine};
    use super::super::plan::tests::{agent, task};
    use super::super::plan::Plan;
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-orchestrator-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn orchestration() -> Orchestration {
        let mut done = task("map", "scout", &[]);
        done.status = TaskStatus::Done;
        done.summary = "The API lives in src/api.".into();
        let mut retry = task("impl", "dev", &["map"]);
        retry.done_when = "tests pass".into();
        Orchestration {
            id: "o".into(),
            name: "Login".into(),
            prompt: "agregá login".into(),
            work_dir: "/repo".into(),
            plan: Plan { agents: vec![agent("scout", Autonomy::Read), agent("dev", Autonomy::Edit)], tasks: vec![done, retry] },
            ..Orchestration::default()
        }
    }

    #[test]
    fn a_task_gets_its_request_and_what_it_builds_on() {
        let mut orchestration = orchestration();
        let (request, context) = request_for(&orchestration, &orchestration.plan.tasks[1]);
        assert!(request.starts_with("do impl"));
        assert!(request.contains("tests pass"));
        let context = context.unwrap();
        assert!(context.contains("## map (scout)"));
        assert!(context.contains("The API lives in src/api."));
        // The first task builds on nothing.
        assert_eq!(request_for(&orchestration, &orchestration.plan.tasks[0]).1, None);
        // A retry also gets how its last attempt went.
        orchestration.plan.tasks[1].attempts = 1;
        orchestration.plan.tasks[1].summary = "2 tests failed".into();
        let (_, context) = request_for(&orchestration, &orchestration.plan.tasks[1]);
        assert!(context.unwrap().contains("2 tests failed"));
    }

    #[test]
    fn the_team_is_saved_as_a_project_of_its_own_and_kept_up_to_date() {
        let root = temp_root("team");
        let mut orchestration = orchestration();
        ensure_team(&root, &mut orchestration).unwrap();
        let project = projects::get_in(&root, &orchestration.project_id).unwrap();
        assert!(project.isolated);
        assert_eq!(project.orchestration_id, "o");
        let agents = projects::agents_of(&root, &project.id);
        assert_eq!(agents.len(), 2);
        assert!(agents.iter().all(|spec| !spec.enabled && spec.work_dir == "/repo" && spec.limits.timeout_min == TASK_TIMEOUT_MIN));
        // A change of model reaches the same saved agent.
        let dev = orchestration.plan.agents[1].agent_id.clone();
        orchestration.plan.agents[1].engine = Engine::Codex;
        orchestration.plan.agents[1].model = "gpt-5.5".into();
        ensure_team(&root, &mut orchestration).unwrap();
        assert_eq!(orchestration.plan.agents[1].agent_id, dev);
        let saved = agent_store::get_in(&root, &dev).unwrap();
        assert_eq!((saved.engine, saved.model.as_str()), (Engine::Codex, "gpt-5.5"));
        assert_eq!(projects::agents_of(&root, &project.id).len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tasks_left_running_go_back_to_waiting() {
        let mut orchestration = orchestration();
        orchestration.plan.tasks[1].status = TaskStatus::Running;
        orchestration.plan.tasks[1].attempts = 1;
        release_running(&mut orchestration);
        assert_eq!(orchestration.plan.tasks[1].status, TaskStatus::Pending);
        // The run that was cut short doesn't count as an attempt.
        assert_eq!(orchestration.plan.tasks[1].attempts, 0);
        assert_eq!(orchestration.plan.tasks[0].status, TaskStatus::Done);
    }
}
