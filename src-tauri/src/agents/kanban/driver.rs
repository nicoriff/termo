//! Running a board: one thread per board that's on, which hands each task
//! to its column's agent when there's room (model::wanted), follows the run
//! through the same runner every agent uses, and moves the card when the
//! run ends — work done goes to review, an approval to done, changes back
//! to work. The saved file is the state: the thread reads it, changes it
//! and writes it back, so the user's moves and the consoles' new tasks are
//! edits to that same file.
//!
//! Each change reaches the frontend through the `kanban` event, with the
//! board as saved.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::commands::agent_commands::{delete_agent, save_agent};
use crate::i18n::{tr, Msg};

use super::super::hub::Hub;
use super::super::project_ai::clip;
use super::super::projects::{self, Project};
use super::super::runner::Runner;
use super::super::spec::{AgentSpec, AskLevel};
use super::super::store::{self as agent_store, now_millis, RunStatus, RunTrigger};
use super::model::{self, Actor, Column, KanbanBoard, KanbanTask, LogKind, Mode, Outcome, Role, Stuck, Success, Verdict};
use super::store;
use super::worktree;

const TICK: Duration = Duration::from_millis(900);
/// Work on a codebase takes longer than an agent's usual errand.
const TASK_TIMEOUT_MIN: u32 = 45;
/// How much of an earlier answer the next agent gets.
const HANDOVER: usize = 6000;
/// The latest log entries an agent gets as context.
const LOG_CONTEXT: usize = 15;

pub fn emit(hub: &Hub, board: &KanbanBoard) {
    hub.emit("kanban", board);
}

/// The boards being driven now, each with the flag that stops its thread.
#[derive(Default)]
pub struct Kanban {
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

fn agent_name(board: &str, role: Role) -> String {
    match role {
        Role::Work => tr(Msg::KanbanWorkAgent(board)),
        Role::Review => tr(Msg::KanbanReviewAgent(board)),
        Role::Summary => tr(Msg::KanbanSummaryAgent(board)),
    }
}

/// Saves the board's column agents as a project of agents (see projects.rs),
/// one saved agent per task each can work on at once — new ones created,
/// the ones there brought up to date, the ones left over removed — so their
/// runs, history and costs live where every agent's do.
pub fn ensure_team(root: &Path, runner: &Runner, board: &mut KanbanBoard) -> Result<(), String> {
    let save_failed = |err: String| tr(Msg::KanbanSaveFailed(&err));
    if board.project_id.is_empty() {
        board.project_id = uuid::Uuid::new_v4().to_string();
    }
    if projects::get_in(root, &board.project_id).is_none() {
        let project = Project {
            id: board.project_id.clone(),
            name: board.name.clone(),
            icon: board.icon.clone(),
            work_dir: board.work_dir.clone(),
            // Its agents keep to themselves, with a board of notes of their own.
            isolated: true,
            kanban_id: board.id.clone(),
            ..Project::default()
        };
        projects::save_in(root, project).map_err(|err| save_failed(err.to_string()))?;
    }
    let slots = board.capacity();
    let name = board.name.clone();
    let work_dir = board.work_dir.clone();
    let project_id = board.project_id.clone();
    let ask_level = board.ask_level;
    let mut unused: Vec<String> = Vec::new();
    for role in Role::ALL {
        let Some(column) = board.agents.get_mut(role) else { continue };
        while column.agent_ids.len() > slots {
            unused.extend(column.agent_ids.pop());
        }
        while column.agent_ids.len() < slots {
            column.agent_ids.push(uuid::Uuid::new_v4().to_string());
        }
        for (slot, id) in column.agent_ids.iter().enumerate() {
            let mut spec = agent_store::get_in(root, id).unwrap_or_else(|| AgentSpec {
                id: id.clone(),
                // Nothing starts it but its board.
                enabled: false,
                ..AgentSpec::default()
            });
            let base = agent_name(&name, role);
            spec.name = if slots > 1 { format!("{base} {}", slot + 1) } else { base };
            spec.icon = match role {
                Role::Work => "🛠️",
                Role::Review => "🔍",
                Role::Summary => "📝",
            }
            .into();
            spec.engine = column.engine;
            spec.model = column.model.clone();
            spec.work_dir = work_dir.clone();
            spec.task = tr(Msg::KanbanAgentTask);
            spec.instructions = column.context.clone();
            spec.autonomy = column.autonomy;
            spec.tools.web = column.web;
            // Notes shared with the other columns' agents — and, being on,
            // Termo's server with the kanban tools (see tools.rs).
            spec.collaboration.board = true;
            spec.collaboration.ask_user = ask_level != AskLevel::Never;
            spec.collaboration.ask_level = ask_level;
            spec.limits.timeout_min = TASK_TIMEOUT_MIN;
            spec.project_id = project_id.clone();
            save_agent(root, spec)?;
        }
    }
    // The summary agent taken away: its saved agents go too.
    if board.agents.summary.is_none() {
        for spec in projects::agents_of(root, &board.project_id) {
            if board.agents.role_of(&spec.id).is_none() {
                unused.push(spec.id);
            }
        }
    }
    for id in unused {
        if !runner.is_running(&id) {
            delete_agent(root, runner, &id)?;
        }
    }
    Ok(())
}

/// What a column's agent is told about asking the user.
fn asking_msg(level: AskLevel) -> Msg<'static> {
    match level {
        AskLevel::Never => Msg::OrchestrationNoAsking,
        AskLevel::Essential => Msg::OrchestrationCanAsk,
        AskLevel::Unsure => Msg::OrchestrationAskWhenUnsure,
    }
}

fn role_how_to(role: Role) -> Msg<'static> {
    match role {
        Role::Work => Msg::KanbanWorkHowTo,
        Role::Review => Msg::KanbanReviewHowTo,
        Role::Summary => Msg::KanbanSummaryHowTo,
    }
}

fn actor_label(actor: &Actor) -> String {
    match actor {
        Actor::User => "user".into(),
        Actor::Console => "console".into(),
        Actor::Termo => "Termo".into(),
        Actor::Agent { role, engine, model } => {
            let role = match role {
                Role::Work => "work",
                Role::Review => "review",
                Role::Summary => "summary",
            };
            let model = if model.is_empty() { String::new() } else { format!(" {model}") };
            format!("{role} agent ({engine:?}{model})")
        }
    }
}

/// What a column's agent is asked for this task, and what it gets as
/// context: what the work agent answered, what review sent back, the tasks
/// it builds on and the latest of its log. The context is the user's and
/// other agents' text: the runner fences it as data (see spec::with_context).
pub fn request_for(board: &KanbanBoard, task: &KanbanTask, role: Role) -> (String, Option<String>) {
    let mut request = String::new();
    if let Some(harness) = board.agents.get(role).map(|agent| agent.harness.trim()).filter(|harness| !harness.is_empty()) {
        request.push_str(harness);
        request.push_str("\n\n");
    }
    request.push_str(&format!("# #{} {}", task.number, task.title.trim()));
    if !task.tags.is_empty() {
        request.push_str(&format!("\n{} {}", tr(Msg::KanbanTags), task.tags.join(", ")));
    }
    request.push_str(&format!("\n\n{}", task.spec.trim()));
    if !task.success_criteria.trim().is_empty() {
        request.push_str(&format!("\n\n{}\n{}", tr(Msg::KanbanSuccessCriteria), task.success_criteria.trim()));
    }
    if !task.branch.is_empty() && role != Role::Summary {
        request.push_str(&format!("\n\n{}", tr(Msg::KanbanOnBranch(&task.branch))));
    }
    if !task.instructions.trim().is_empty() && role != Role::Summary {
        request.push_str(&format!("\n\n{}\n{}", tr(Msg::KanbanUserInstructions), task.instructions.trim()));
    }
    request.push_str(&format!("\n\n{}\n\n{}", tr(role_how_to(role)), tr(asking_msg(board.ask_level))));

    let mut context = String::new();
    let mut section = |heading: Msg, body: &str| {
        if !context.is_empty() {
            context.push_str("\n\n");
        }
        context.push_str(&format!("{}\n{}", tr(heading), clip(body.trim(), HANDOVER)));
    };
    if role != Role::Work && !task.work_summary.trim().is_empty() {
        section(Msg::KanbanWorkSummary, &task.work_summary);
    }
    if role != Role::Review && task.review_rounds > 0 && !task.review_notes.trim().is_empty() {
        section(Msg::KanbanReviewNotes, &task.review_notes);
    }
    let earlier: Vec<String> = task
        .depends_on
        .iter()
        .filter_map(|dep| board.task(dep))
        .filter_map(|other| {
            let text = other.outcome.as_ref().map(|outcome| outcome.summary.as_str()).unwrap_or(other.work_summary.as_str());
            (!text.trim().is_empty()).then(|| format!("## #{} {}\n{}", other.number, other.title, clip(text.trim(), HANDOVER / 2)))
        })
        .collect();
    if !earlier.is_empty() {
        section(Msg::KanbanDependsOn, &earlier.join("\n\n"));
    }
    let log: Vec<String> = task
        .log
        .iter()
        .filter(|entry| !entry.text.trim().is_empty())
        .rev()
        .take(LOG_CONTEXT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|entry| format!("- [{}] {}", actor_label(&entry.actor), clip(entry.text.trim(), 600)))
        .collect();
    if !log.is_empty() {
        section(Msg::KanbanPreviousLog, &log.join("\n"));
    }
    (request, (!context.is_empty()).then_some(context))
}

fn agent_actor(board: &KanbanBoard, role: Role) -> Actor {
    let column = board.agents.get(role).cloned().unwrap_or_default();
    Actor::Agent { role, engine: column.engine, model: column.model }
}

/// Left as it is when Termo closed mid-run: nothing runs those tasks any
/// more. They stay in their column and are taken again.
pub fn recover(root: &Path) {
    for board in store::list_in(root).into_iter().filter(|board| board.tasks.iter().any(KanbanTask::is_running)) {
        store::update_in(root, &board.id, |board| {
            let now = now_millis();
            for task in board.tasks.iter_mut().filter(|task| task.is_running()) {
                task.release();
                task.note(now, Actor::Termo, LogKind::Interrupted, "");
            }
        });
    }
}

impl Kanban {
    /// Asks its thread to stop, without touching what runs (deleting it
    /// stops those first).
    pub fn stop(&self, id: &str) {
        if let Some(flag) = self.active.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Drives every board that's on or still has something running: at
    /// launch, and when Modo IA comes back on.
    pub fn start_all(self: &Arc<Self>, hub: &Hub, runner: &Arc<Runner>) {
        for board in store::list_in(hub.root()) {
            if board.enabled || board.running() > 0 {
                self.start(hub, runner, &board.id);
            }
        }
    }

    /// Drives the board from where its file says it is, until it's off with
    /// nothing running. Nothing happens if it's already driven.
    pub fn start(self: &Arc<Self>, hub: &Hub, runner: &Arc<Runner>, id: &str) {
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active.lock().unwrap();
            if active.contains_key(id) {
                return;
            }
            active.insert(id.to_string(), stop.clone());
        }
        let drive = Drive { hub: hub.clone(), runner: runner.clone(), root: hub.root().to_path_buf(), id: id.to_string(), stop };
        let kanban = self.clone();
        std::thread::spawn(move || {
            drive.run();
            kanban.active.lock().unwrap().remove(&drive.id);
        });
    }
}

struct Drive {
    hub: Hub,
    runner: Arc<Runner>,
    root: PathBuf,
    id: String,
    stop: Arc<AtomicBool>,
}

/// How a run that ended leaves its task.
struct Ended {
    task_id: String,
    role: Role,
    status: RunStatus,
    summary: String,
    cost: Option<f64>,
    failure: String,
}

impl Drive {
    fn update(&self, change: impl FnOnce(&mut KanbanBoard)) -> Option<KanbanBoard> {
        let board = store::update_in(&self.root, &self.id, change)?;
        emit(&self.hub, &board);
        Some(board)
    }

    fn run(&self) {
        loop {
            if self.stop.load(Ordering::Relaxed) || !super::super::host::runs_here() {
                return;
            }
            let Some(board) = store::get_in(&self.root, &self.id) else { return };
            if self.collect(&board) {
                continue;
            }
            if !board.enabled && board.running() == 0 {
                return;
            }
            if board.enabled && crate::ai_mode::enabled() && self.launch(&board) {
                continue;
            }
            std::thread::sleep(TICK);
        }
    }

    /// Takes in the runs that ended since the last look. Returns whether any did.
    fn collect(&self, board: &KanbanBoard) -> bool {
        let mut ended: Vec<Ended> = Vec::new();
        for task in board.tasks.iter().filter(|task| task.is_running()) {
            if self.runner.is_running_run(&task.agent_id, &task.run_id) {
                continue;
            }
            let run = agent_store::get_run_in(&self.root, &task.agent_id, &task.run_id);
            let (status, summary, cost, failure) = match run {
                Some(run) => {
                    let failure = run.failure.as_ref().map(|failure| failure.message.clone()).unwrap_or_default();
                    (run.status, run.summary, run.cost_usd, failure)
                }
                None => (RunStatus::Error, String::new(), None, String::new()),
            };
            ended.push(Ended { task_id: task.id.clone(), role: task.role.unwrap_or_default(), status, summary, cost, failure });
        }
        if ended.is_empty() {
            return false;
        }
        self.update(|board| {
            for run in ended {
                settle(board, run, now_millis());
            }
        });
        true
    }

    /// Starts what can start now. Returns whether anything did.
    fn launch(&self, board: &KanbanBoard) -> bool {
        let mut room = board.capacity().saturating_sub(board.running());
        if room == 0 {
            return false;
        }
        let mut taken: HashSet<String> = board.tasks.iter().filter(|task| task.is_running()).map(|task| task.agent_id.clone()).collect();
        let mut started = false;
        for (task_id, role) in model::wanted(board) {
            if room == 0 {
                break;
            }
            let Some(column) = board.agents.get(role) else { continue };
            let Some(agent_id) =
                column.agent_ids.iter().find(|id| !taken.contains(*id) && !self.runner.is_running(id)).cloned()
            else {
                continue;
            };
            let Some(task) = board.task(&task_id) else { continue };
            self.start_task(board, task, role, &agent_id);
            taken.insert(agent_id);
            room -= 1;
            started = true;
        }
        started
    }

    fn start_task(&self, board: &KanbanBoard, task: &KanbanTask, role: Role, agent_id: &str) {
        let now = now_millis();
        let actor = agent_actor(board, role);
        // In worktree mode the work starts on its own branch.
        let mut worktree_note = None;
        let mut checkout: Option<(String, String)> = None;
        if board.mode == Mode::Worktrees && role == Role::Work {
            let branch = if task.branch.is_empty() { worktree::branch_for(task.number, &task.id) } else { task.branch.clone() };
            let folder = store::dir_of(&self.root, &board.id).map(|dir| worktree::folder_for(&dir, &task.id));
            let made = folder
                .map_err(|err| err.to_string())
                .and_then(|folder| worktree::create(Path::new(&board.work_dir), &folder, &branch, &board.base_branch).map(|_| folder));
            match made {
                Ok(folder) => {
                    if task.worktree.is_empty() {
                        worktree_note = Some(branch.clone());
                    }
                    checkout = Some((branch, folder.to_string_lossy().into_owned()));
                }
                Err(error) => {
                    let reason = tr(Msg::KanbanWorktreeFailed(&error));
                    self.update(|board| {
                        if let Some(task) = board.task_mut(&task.id) {
                            task.stick(Stuck::NotStarted, reason.clone());
                            task.note(now, Actor::Termo, LogKind::WorkFailed, reason);
                        }
                    });
                    return;
                }
            }
        }
        let folder = checkout.as_ref().map(|(_, folder)| folder.clone()).unwrap_or_else(|| task.worktree.clone());
        let started = match agent_store::get_in(&self.root, agent_id) {
            Some(mut spec) => {
                let (request, context) = request_for(board, task, role);
                spec.task = request;
                if !folder.is_empty() {
                    spec.work_dir = folder;
                }
                self.runner.start_linked(&self.hub, &spec, RunTrigger::Kanban, context.as_deref(), None, false)
            }
            None => Err(tr(Msg::AgentRunFailed("agent missing"))),
        };
        self.update(|board| {
            let Some(task) = board.task_mut(&task.id) else { return };
            if let Some((branch, folder)) = checkout {
                task.branch = branch;
                task.worktree = folder;
            }
            if let Some(branch) = worktree_note {
                task.note(now, Actor::Termo, LogKind::Branch, branch);
            }
            match started {
                Ok(run) => {
                    if task.column == Column::Todo {
                        let from = task.column;
                        task.column = Column::InProgress;
                        task.log.push(model::LogEntry {
                            at: now,
                            actor: actor.clone(),
                            kind: LogKind::Moved,
                            text: String::new(),
                            from: Some(from),
                            to: Some(Column::InProgress),
                        });
                    }
                    task.role = Some(role);
                    task.run_id = run.id;
                    task.agent_id = agent_id.to_string();
                    task.verdict = None;
                    task.report = None;
                    task.note(now, actor, LogKind::Taken, "");
                }
                Err(error) => {
                    task.stick(Stuck::NotStarted, tr(Msg::KanbanRunFailed(&error)));
                    task.note(now, Actor::Termo, LogKind::WorkFailed, error);
                }
            }
        });
    }
}

/// Moves a task on from a run that ended: the heart of the board's rules.
fn settle(board: &mut KanbanBoard, run: Ended, now: i64) {
    let max_rounds = board.max_review_rounds;
    let has_summary = board.agents.summary.is_some();
    let actor = agent_actor(board, run.role);
    let Some(task) = board.task_mut(&run.task_id) else { return };
    let verdict = task.verdict;
    let report = task.report.clone();
    task.release();
    task.instructions = String::new();
    if let Some(cost) = run.cost {
        task.cost_usd = Some(task.cost_usd.unwrap_or(0.0) + cost);
    }
    let succeeded = run.status == RunStatus::Success;
    let why = || {
        let text = if run.failure.is_empty() { run.summary.trim() } else { run.failure.trim() };
        clip(text, 600)
    };
    let move_to = |task: &mut KanbanTask, to: Column| {
        let from = task.column;
        if from != to {
            task.column = to;
            task.log.push(model::LogEntry { at: now, actor: actor.clone(), kind: LogKind::Moved, text: String::new(), from: Some(from), to: Some(to) });
        }
    };
    match run.role {
        Role::Work => {
            let failed_by_agent = report.as_ref().is_some_and(|report| report.success == Success::Failed);
            if !succeeded || failed_by_agent {
                let text = report.as_ref().map(|report| report.notes.clone()).filter(|notes| !notes.trim().is_empty()).unwrap_or_else(why);
                task.stick(Stuck::WorkFailed, tr(Msg::KanbanRunFailed(&text)));
                task.note(now, actor.clone(), LogKind::WorkFailed, text);
                return;
            }
            task.work_summary = report.as_ref().map(|report| report.summary.clone()).filter(|text| !text.trim().is_empty()).unwrap_or(run.summary.clone());
            task.note(now, actor.clone(), LogKind::WorkDone, clip(task.work_summary.trim(), 2000));
            move_to(task, Column::Review);
        }
        Role::Review => match verdict {
            Some(Verdict::Approve) => {
                task.note(now, actor.clone(), LogKind::Approved, task.review_notes.clone());
                move_to(task, Column::Done);
                if has_summary {
                    task.summary_pending = true;
                } else {
                    task.outcome = Some(report.unwrap_or_else(|| Outcome {
                        summary: task.work_summary.clone(),
                        success: Success::Full,
                        notes: task.review_notes.clone(),
                        at: now,
                    }));
                    if let Some(outcome) = task.outcome.as_mut() {
                        outcome.at = now;
                    }
                }
            }
            Some(Verdict::Changes) => {
                task.review_rounds += 1;
                task.note(now, actor.clone(), LogKind::ChangesRequested, task.review_notes.clone());
                move_to(task, Column::InProgress);
                if task.review_rounds >= max_rounds {
                    let rounds = task.review_rounds.to_string();
                    task.stick(Stuck::TooManyRounds, tr(Msg::KanbanTooManyRounds(&rounds)));
                    task.note(now, Actor::Termo, LogKind::TooManyRounds, "");
                }
            }
            None if !succeeded => {
                let text = why();
                task.stick(Stuck::ReviewFailed, tr(Msg::KanbanRunFailed(&text)));
                task.note(now, actor.clone(), LogKind::WorkFailed, text);
            }
            None => {
                task.stick(Stuck::NoVerdict, tr(Msg::KanbanNoVerdict));
                task.note(now, actor.clone(), LogKind::NoVerdict, clip(run.summary.trim(), 2000));
            }
        },
        Role::Summary => {
            task.summary_pending = false;
            let outcome = report.unwrap_or_else(|| Outcome {
                summary: run.summary.clone(),
                success: if succeeded { Success::Full } else { Success::Partial },
                notes: String::new(),
                at: now,
            });
            task.outcome = Some(Outcome { at: now, ..outcome });
            task.note(now, actor.clone(), LogKind::Summarized, "");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::tests::{board, task};
    use super::*;

    fn ended(id: &str, role: Role, status: RunStatus, summary: &str) -> Ended {
        Ended { task_id: id.into(), role, status, summary: summary.into(), cost: Some(0.5), failure: String::new() }
    }

    #[test]
    fn work_done_goes_to_review_and_failed_work_waits_for_you() {
        let mut board = board(vec![task("a", Column::InProgress, &[]), task("b", Column::InProgress, &[])]);
        board.tasks[0].run_id = "r".into();
        settle(&mut board, ended("a", Role::Work, RunStatus::Success, "did it"), 1);
        let a = board.task("a").unwrap();
        assert_eq!((a.column, a.work_summary.as_str(), a.is_running()), (Column::Review, "did it", false));
        assert_eq!(a.cost_usd, Some(0.5));

        settle(&mut board, ended("b", Role::Work, RunStatus::Error, "boom"), 2);
        let b = board.task("b").unwrap();
        assert_eq!(b.column, Column::InProgress);
        assert!(b.needs_you.contains("boom"));
    }

    #[test]
    fn the_work_agent_can_say_it_failed() {
        let mut board = board(vec![task("a", Column::InProgress, &[])]);
        board.tasks[0].report = Some(Outcome { success: Success::Failed, notes: "no access to the API".into(), ..Outcome::default() });
        settle(&mut board, ended("a", Role::Work, RunStatus::Success, "x"), 1);
        assert!(board.task("a").unwrap().needs_you.contains("no access"));
    }

    #[test]
    fn review_approves_or_sends_back_up_to_the_limit() {
        let mut board = board(vec![task("a", Column::Review, &[])]);
        board.max_review_rounds = 2;
        for round in 1..=2 {
            board.tasks[0].column = Column::Review;
            board.tasks[0].verdict = Some(Verdict::Changes);
            board.tasks[0].review_notes = "fix the tests".into();
            settle(&mut board, ended("a", Role::Review, RunStatus::Success, ""), round);
            let a = board.task("a").unwrap();
            assert_eq!((a.column, a.review_rounds), (Column::InProgress, round as u32));
            assert_eq!(a.needs_you.is_empty(), round < 2);
        }

        let mut board = super::super::model::tests::board(vec![task("a", Column::Review, &[])]);
        board.tasks[0].work_summary = "built it".into();
        board.tasks[0].verdict = Some(Verdict::Approve);
        settle(&mut board, ended("a", Role::Review, RunStatus::Success, ""), 3);
        let a = board.task("a").unwrap();
        assert_eq!(a.column, Column::Done);
        // No summary agent: the outcome is written from the review.
        assert_eq!(a.outcome.as_ref().unwrap().summary, "built it");
    }

    #[test]
    fn a_review_without_a_verdict_waits_for_you() {
        let mut board = board(vec![task("a", Column::Review, &[])]);
        settle(&mut board, ended("a", Role::Review, RunStatus::Success, "looks fine"), 1);
        let a = board.task("a").unwrap();
        assert_eq!(a.column, Column::Review);
        assert!(!a.needs_you.is_empty());
        assert_eq!(a.log.last().unwrap().kind, LogKind::NoVerdict);
    }

    #[test]
    fn with_a_summary_agent_done_waits_for_its_summary() {
        let mut board = board(vec![task("a", Column::Review, &[])]);
        board.agents.summary = Some(Default::default());
        board.tasks[0].verdict = Some(Verdict::Approve);
        settle(&mut board, ended("a", Role::Review, RunStatus::Success, ""), 1);
        assert!(board.task("a").unwrap().summary_pending);
        assert!(board.task("a").unwrap().outcome.is_none());
        board.tasks[0].report = Some(Outcome { summary: "all good".into(), success: Success::Partial, ..Outcome::default() });
        settle(&mut board, ended("a", Role::Summary, RunStatus::Success, ""), 2);
        let a = board.task("a").unwrap();
        assert!(!a.summary_pending);
        assert_eq!(a.outcome.as_ref().map(|o| (o.summary.as_str(), o.success)), Some(("all good", Success::Partial)));
    }

    #[test]
    fn the_request_has_the_task_and_the_context_what_came_before() {
        let mut board = board(vec![task("dep", Column::Done, &[]), task("a", Column::InProgress, &["dep"])]);
        board.agents.work.harness = "Write tests first.".into();
        board.tasks[0].outcome = Some(Outcome { summary: "The API is in src/api.".into(), ..Outcome::default() });
        board.tasks[1].success_criteria = "tests pass".into();
        board.tasks[1].review_rounds = 1;
        board.tasks[1].review_notes = "missing a test".into();
        board.tasks[1].note(1, Actor::User, LogKind::Note, "use the new client");
        let (request, context) = request_for(&board, &board.tasks[1], Role::Work);
        assert!(request.starts_with("Write tests first."));
        assert!(request.contains("# #2 do a"));
        assert!(request.contains("tests pass"));
        let context = context.unwrap();
        assert!(context.contains("missing a test"));
        assert!(context.contains("The API is in src/api."));
        assert!(context.contains("use the new client"));
        // The reviewer gets what the work agent said, not its own notes back.
        board.tasks[1].work_summary = "added the endpoint".into();
        let (_, context) = request_for(&board, &board.tasks[1], Role::Review);
        let context = context.unwrap();
        assert!(context.contains("added the endpoint"));
        assert!(!context.contains("missing a test"));
    }

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-kanban-driver-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_team_has_one_agent_per_slot_and_follows_the_board() {
        let root = temp_root("team");
        let runner = Runner::default();
        let mut board = board(vec![]);
        ensure_team(&root, &runner, &mut board).unwrap();
        assert_eq!((board.agents.work.agent_ids.len(), board.agents.review.agent_ids.len()), (1, 1));
        let project = projects::get_in(&root, &board.project_id).unwrap();
        assert_eq!(project.kanban_id, "b");
        assert!(project.isolated);

        board.mode = Mode::Worktrees;
        board.max_parallel = 3;
        board.agents.summary = Some(Default::default());
        ensure_team(&root, &runner, &mut board).unwrap();
        assert_eq!(projects::agents_of(&root, &board.project_id).len(), 9);
        let spec = agent_store::get_in(&root, &board.agents.work.agent_ids[0]).unwrap();
        assert!(!spec.enabled && spec.collaboration.board && spec.work_dir == "/repo");

        board.mode = Mode::Sequential;
        board.agents.summary = None;
        ensure_team(&root, &runner, &mut board).unwrap();
        assert_eq!(projects::agents_of(&root, &board.project_id).len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tasks_left_running_are_released_and_logged() {
        let root = temp_root("recover");
        let mut board = board(vec![task("a", Column::Review, &[])]);
        board.tasks[0].run_id = "r".into();
        board.tasks[0].role = Some(Role::Review);
        store::save_in(&root, board).unwrap();
        recover(&root);
        let a = store::get_in(&root, "b").unwrap().tasks.remove(0);
        assert_eq!((a.column, a.is_running()), (Column::Review, false));
        assert_eq!(a.log.last().unwrap().kind, LogKind::Interrupted);
        let _ = std::fs::remove_dir_all(&root);
    }
}
