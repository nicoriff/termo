//! What the user (and the consoles) do to a board: create and set it up,
//! switch it on, write tasks, move them, merge what's done. These run where
//! the agents run (see agents/ops.rs): the driver writes the same file, and
//! the store's lock only holds within one process.

use std::path::Path;

use serde::Deserialize;

use crate::commands::agent_commands::delete_agent;
use crate::i18n::{tr, Msg};

use super::super::ops::Agents;
use super::super::projects;
use super::super::store::now_millis;
use super::driver::{emit, ensure_team};
use super::model::{self, clean_dependencies, Actor, BoardAgents, Column, KanbanBoard, KanbanTask, LogKind, Mode};
use super::store;
use super::worktree;

const MAX_TITLE: usize = 200;
const MAX_TEXT: usize = 20_000;

fn not_found() -> String {
    tr(Msg::KanbanNotFound)
}

fn task_not_found() -> String {
    tr(Msg::KanbanTaskNotFound)
}

fn save_failed(error: impl std::fmt::Display) -> String {
    tr(Msg::KanbanSaveFailed(&error.to_string()))
}

fn clip(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

/// The board's settings as the user set them; its tasks are left alone.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub work_dir: String,
    pub mode: Mode,
    pub max_parallel: u32,
    pub max_review_rounds: u32,
    pub base_branch: String,
    pub ask_level: super::super::spec::AskLevel,
    pub agents: BoardAgents,
}

impl Default for Settings {
    fn default() -> Self {
        let board = KanbanBoard::default();
        Self {
            id: String::new(),
            name: String::new(),
            icon: String::new(),
            work_dir: String::new(),
            mode: board.mode,
            max_parallel: board.max_parallel,
            max_review_rounds: board.max_review_rounds,
            base_branch: String::new(),
            ask_level: board.ask_level,
            agents: BoardAgents::default(),
        }
    }
}

fn check_folder(work_dir: &str, mode: Mode) -> Result<(), String> {
    if work_dir.trim().is_empty() {
        return Err(tr(Msg::OrchestrationFolderRequired));
    }
    let dir = Path::new(work_dir.trim());
    if !dir.is_dir() {
        return Err(tr(Msg::OrchestrationNoFolder(work_dir.trim())));
    }
    if mode == Mode::Worktrees && !worktree::is_repo(dir) {
        return Err(tr(Msg::KanbanNotARepo));
    }
    Ok(())
}

/// Creates a board, or changes one's settings. Its column agents are saved
/// to match, and a board that's on is driven.
pub fn save(agents: &Agents, settings: Settings) -> Result<KanbanBoard, String> {
    let root = agents.hub.root();
    if settings.name.trim().is_empty() {
        return Err(tr(Msg::KanbanNameRequired));
    }
    check_folder(&settings.work_dir, settings.mode)?;
    let existing = (!settings.id.is_empty()).then(|| store::get_in(root, &settings.id)).flatten();
    let mut board = existing.unwrap_or_else(|| KanbanBoard { id: uuid::Uuid::new_v4().to_string(), ..KanbanBoard::default() });
    // The saved agents behind each column stay the board's own.
    let mut wanted = settings.agents;
    for role in model::Role::ALL {
        if let (Some(new), Some(old)) = (wanted.get_mut(role), board.agents.get(role)) {
            new.agent_ids = old.agent_ids.clone();
        }
    }
    let changed_folder = !board.work_dir.is_empty() && board.work_dir != settings.work_dir.trim();
    let running = board.running() > 0;
    if running && (changed_folder || board.mode != settings.mode) {
        return Err(tr(Msg::KanbanBusy));
    }
    board.name = clip(&settings.name, MAX_TITLE);
    board.icon = settings.icon;
    board.work_dir = settings.work_dir.trim().to_string();
    board.mode = settings.mode;
    board.max_parallel = settings.max_parallel;
    board.max_review_rounds = settings.max_review_rounds;
    board.base_branch = settings.base_branch.trim().to_string();
    board.ask_level = settings.ask_level;
    board.agents = wanted;
    let mut board = model::sanitize(board);
    ensure_team(root, &agents.runner, &mut board)?;
    commit(agents, board)
}

/// Saves it, tells everyone and drives it when it's on.
fn commit(agents: &Agents, board: KanbanBoard) -> Result<KanbanBoard, String> {
    let board = store::save_in(agents.hub.root(), board).map_err(save_failed)?;
    emit(&agents.hub, &board);
    wake(agents, &board);
    Ok(board)
}

fn wake(agents: &Agents, board: &KanbanBoard) {
    if board.enabled {
        agents.kanban.start(&agents.hub, &agents.runner, &board.id);
    }
}

/// A change to the saved board, told to everyone.
fn change<T>(agents: &Agents, id: &str, edit: impl FnOnce(&mut KanbanBoard) -> Result<T, String>) -> Result<(KanbanBoard, T), String> {
    let (board, value) = store::try_update_in(agents.hub.root(), id, edit).ok_or_else(not_found)??;
    emit(&agents.hub, &board);
    wake(agents, &board);
    Ok((board, value))
}

pub fn set_enabled(agents: &Agents, id: &str, enabled: bool) -> Result<KanbanBoard, String> {
    let current = store::get_in(agents.hub.root(), id).ok_or_else(not_found)?;
    if enabled {
        check_folder(&current.work_dir, current.mode)?;
    }
    Ok(change(agents, id, |board| {
        board.enabled = enabled;
        Ok(())
    })?
    .0)
}

/// Deletes a board with its team: what runs is stopped, the agents and
/// their history go, and so do its worktrees (their branches stay).
pub fn delete(agents: &Agents, id: &str) -> Result<Vec<KanbanBoard>, String> {
    let root = agents.hub.root();
    agents.kanban.stop(id);
    if let Some(board) = store::get_in(root, id) {
        for task in board.tasks.iter().filter(|task| task.is_running()) {
            agents.runner.stop(&task.agent_id);
        }
        for task in board.tasks.iter().filter(|task| !task.worktree.is_empty()) {
            worktree::remove(Path::new(&board.work_dir), Path::new(&task.worktree), &task.branch, false);
        }
        if !board.project_id.is_empty() {
            for spec in projects::agents_of(root, &board.project_id) {
                delete_agent(root, &agents.runner, &spec.id)?;
            }
            let _ = projects::delete_in(root, &board.project_id);
        }
    }
    store::delete_in(root, id).map_err(save_failed)?;
    Ok(store::list_in(root))
}

/// A task as the user or a console writes it.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskInput {
    /// Empty: a new task.
    pub id: String,
    pub title: String,
    pub spec: String,
    pub success_criteria: String,
    pub depends_on: Vec<String>,
    /// Where a new one goes: not ready (the default) or to do.
    pub column: Option<Column>,
}

/// Creates a task or edits one. A task under way can be edited: its next
/// run reads it as it is then.
pub fn upsert_task(agents: &Agents, board_id: &str, input: TaskInput, actor: Actor) -> Result<(KanbanBoard, KanbanTask), String> {
    let title = clip(&input.title, MAX_TITLE);
    if title.is_empty() {
        return Err(tr(Msg::KanbanTitleRequired));
    }
    let (board, task_id) = change(agents, board_id, |board| {
        let now = now_millis();
        if input.id.is_empty() {
            if board.tasks.len() >= model::MAX_TASKS {
                return Err(tr(Msg::KanbanTooManyTasks));
            }
            let id = uuid::Uuid::new_v4().to_string();
            let column = match input.column {
                Some(Column::Todo) => Column::Todo,
                _ => Column::NotReady,
            };
            let mut task = KanbanTask {
                id: id.clone(),
                number: board.next_number,
                title,
                spec: clip(&input.spec, MAX_TEXT),
                success_criteria: clip(&input.success_criteria, MAX_TEXT),
                column,
                created_at: now,
                ..KanbanTask::default()
            };
            board.next_number += 1;
            task.note(now, actor, LogKind::Created, "");
            board.tasks.push(task);
            let deps = clean_dependencies(board, &id, &input.depends_on);
            if let Some(task) = board.task_mut(&id) {
                task.depends_on = deps;
            }
            // At the end of its column.
            model::move_task(board, &id, column, None, now, Actor::Termo);
            Ok(id)
        } else {
            let deps = clean_dependencies(board, &input.id, &input.depends_on);
            let task = board.task_mut(&input.id).ok_or_else(task_not_found)?;
            task.title = title;
            task.spec = clip(&input.spec, MAX_TEXT);
            task.success_criteria = clip(&input.success_criteria, MAX_TEXT);
            task.depends_on = deps;
            task.note(now, actor, LogKind::Edited, "");
            Ok(input.id.clone())
        }
    })?;
    let task = board.task(&task_id).cloned().ok_or_else(task_not_found)?;
    Ok((board, task))
}

/// Stops what runs a task, if anything: the run's id goes first, so the
/// driver doesn't take its end for a result.
fn cancel_run(board: &mut KanbanBoard, task_id: &str, actor: &Actor) -> Option<String> {
    let task = board.task_mut(task_id)?;
    if !task.is_running() {
        return None;
    }
    let agent = task.agent_id.clone();
    task.release();
    task.note(now_millis(), actor.clone(), LogKind::Cancelled, "");
    Some(agent)
}

/// Moves a task to a column, before another task or at its end. One under
/// way only with `cancel`: its run is stopped.
pub fn move_task(
    agents: &Agents,
    board_id: &str,
    task_id: &str,
    column: Column,
    before: Option<&str>,
    cancel: bool,
    actor: Actor,
) -> Result<KanbanBoard, String> {
    let (board, stopped) = change(agents, board_id, |board| {
        let task = board.task(task_id).ok_or_else(task_not_found)?;
        let mut stopped = None;
        if task.is_running() && task.column != column {
            if !cancel {
                return Err(tr(Msg::KanbanTaskRunning));
            }
            stopped = cancel_run(board, task_id, &actor);
        }
        model::move_task(board, task_id, column, before, now_millis(), actor);
        Ok(stopped)
    })?;
    if let Some(agent) = stopped {
        agents.runner.stop(&agent);
    }
    Ok(board)
}

/// Stops the run a task is under, leaving it where it is and waiting for
/// the user.
pub fn cancel_task(agents: &Agents, board_id: &str, task_id: &str) -> Result<KanbanBoard, String> {
    let (board, stopped) = change(agents, board_id, |board| {
        let stopped = cancel_run(board, task_id, &Actor::User);
        if let Some(task) = board.task_mut(task_id).filter(|_| stopped.is_some()) {
            task.needs_you = tr(Msg::KanbanCancelled);
        }
        Ok(stopped)
    })?;
    if let Some(agent) = stopped {
        agents.runner.stop(&agent);
    }
    Ok(board)
}

pub fn delete_task(agents: &Agents, board_id: &str, task_id: &str) -> Result<KanbanBoard, String> {
    let (board, (stopped, worktree)) = change(agents, board_id, |board| {
        let stopped = cancel_run(board, task_id, &Actor::User);
        let index = board.tasks.iter().position(|task| task.id == task_id).ok_or_else(task_not_found)?;
        let task = board.tasks.remove(index);
        for other in &mut board.tasks {
            other.depends_on.retain(|dep| dep != task_id);
        }
        Ok((stopped, (!task.worktree.is_empty()).then(|| (task.worktree, task.branch))))
    })?;
    if let Some(agent) = stopped {
        agents.runner.stop(&agent);
    }
    if let Some((folder, branch)) = worktree {
        worktree::remove(Path::new(&board.work_dir), Path::new(&folder), &branch, true);
    }
    Ok(board)
}

/// The user decided on a task that was stuck: it's taken again where it is,
/// with its rounds of review counted from zero.
pub fn resolve_task(agents: &Agents, board_id: &str, task_id: &str) -> Result<KanbanBoard, String> {
    Ok(change(agents, board_id, |board| {
        let task = board.task_mut(task_id).ok_or_else(task_not_found)?;
        task.needs_you = String::new();
        task.review_rounds = 0;
        task.note(now_millis(), Actor::User, LogKind::Resolved, "");
        Ok(())
    })?
    .0)
}

pub fn note_task(agents: &Agents, board_id: &str, task_id: &str, text: &str, actor: Actor) -> Result<KanbanBoard, String> {
    let text = clip(text, MAX_TEXT);
    if text.is_empty() {
        return Err(tr(Msg::KanbanNoteRequired));
    }
    Ok(change(agents, board_id, |board| {
        let task = board.task_mut(task_id).ok_or_else(task_not_found)?;
        task.note(now_millis(), actor, LogKind::Note, text);
        Ok(())
    })?
    .0)
}

/// Merges a done task's branch into the board's base branch, then takes its
/// worktree and branch away. A conflict leaves everything as it was.
pub fn merge_task(agents: &Agents, board_id: &str, task_id: &str) -> Result<KanbanBoard, String> {
    let root = agents.hub.root();
    let board = store::get_in(root, board_id).ok_or_else(not_found)?;
    let task = board.task(task_id).ok_or_else(task_not_found)?;
    if task.branch.is_empty() || task.merged || task.column != Column::Done || task.is_running() {
        return Err(tr(Msg::KanbanNotMergeable));
    }
    let repo = Path::new(&board.work_dir);
    let message = format!("Merge #{} {}", task.number, task.title);
    let merged = worktree::merge(repo, &task.branch, &board.base_branch, &message);
    let (branch, folder) = (task.branch.clone(), task.worktree.clone());
    let (board, _) = change(agents, board_id, |board| {
        let task = board.task_mut(task_id).ok_or_else(task_not_found)?;
        match &merged {
            Ok(()) => {
                task.merged = true;
                task.worktree = String::new();
                task.note(now_millis(), Actor::User, LogKind::Merged, branch.clone());
            }
            Err(error) => task.note(now_millis(), Actor::User, LogKind::MergeFailed, error.clone()),
        }
        Ok(())
    })?;
    match merged {
        Ok(()) => {
            worktree::remove(repo, Path::new(&folder), &branch, true);
            Ok(board)
        }
        Err(error) => Err(tr(Msg::KanbanMergeFailed(&error))),
    }
}
