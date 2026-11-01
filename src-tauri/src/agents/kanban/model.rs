//! A kanban board: tasks the user writes and orders, and one agent per
//! column that takes them — one does the work, another reviews it, an
//! optional third writes how it went. Everything here is plain data and
//! pure rules (which task is next, what a move does, what a cycle of
//! dependencies is), so it can be tested without running anything.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::super::spec::{AskLevel, Autonomy, Engine};

/// More than a person keeps on one board; a console can't flood it.
pub const MAX_TASKS: usize = 500;
pub const MAX_PARALLEL: u32 = 6;
pub const MAX_REVIEW_ROUNDS: u32 = 10;
/// Entries a task's log keeps; the oldest go first.
const MAX_LOG: usize = 200;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Column {
    /// Still being written: no agent takes it.
    #[default]
    NotReady,
    /// Ready: the work agent takes it when it can.
    Todo,
    InProgress,
    Review,
    Done,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// One task at a time in the board's folder.
    #[default]
    Sequential,
    /// Each task on its own branch, in its own git worktree: several at once.
    Worktrees,
}

/// What a column's agent does with a task.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// In progress: does what the task asks.
    #[default]
    Work,
    /// In review: checks the work against the task, approves it or sends it back.
    Review,
    /// Done: writes what was done and how well.
    Summary,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Work, Role::Review, Role::Summary];
}

/// The agent the user set for a column.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ColumnAgent {
    pub engine: Engine,
    /// Empty: the CLI's default.
    pub model: String,
    pub autonomy: Autonomy,
    pub web: bool,
    /// What the agent should know: the project, its conventions.
    pub context: String,
    /// What it does with each task in this column, step by step.
    pub harness: String,
    /// The saved agents that run it, one per task it can work on at once
    /// (the runner runs one task per agent). Filled in by the driver.
    pub agent_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BoardAgents {
    pub work: ColumnAgent,
    pub review: ColumnAgent,
    /// Without it, the reviewer's verdict is the task's summary.
    pub summary: Option<ColumnAgent>,
}

impl BoardAgents {
    pub fn get(&self, role: Role) -> Option<&ColumnAgent> {
        match role {
            Role::Work => Some(&self.work),
            Role::Review => Some(&self.review),
            Role::Summary => self.summary.as_ref(),
        }
    }

    pub fn get_mut(&mut self, role: Role) -> Option<&mut ColumnAgent> {
        match role {
            Role::Work => Some(&mut self.work),
            Role::Review => Some(&mut self.review),
            Role::Summary => self.summary.as_mut(),
        }
    }

    /// Which column agent a saved agent stands for.
    pub fn role_of(&self, agent_id: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|role| self.get(*role).is_some_and(|agent| agent.agent_ids.iter().any(|id| id == agent_id)))
    }
}

/// Who did something to a task.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Actor {
    User,
    /// From one of the user's consoles, through Termo's MCP server.
    Console,
    #[serde(rename_all = "camelCase")]
    Agent { role: Role, engine: Engine, model: String },
    /// Termo itself: a run that failed, Termo closing mid-run.
    Termo,
}

/// What happened, for the frontend to say in the user's language; `text`
/// carries what an agent or the user wrote.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LogKind {
    #[default]
    Note,
    Created,
    Edited,
    Moved,
    Taken,
    WorkDone,
    WorkFailed,
    Approved,
    ChangesRequested,
    /// Sent back more times than the board allows.
    TooManyRounds,
    /// The reviewer ended without approving or sending it back.
    NoVerdict,
    Summarized,
    Cancelled,
    Interrupted,
    Branch,
    Merged,
    MergeFailed,
    Resolved,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// Unix milliseconds.
    pub at: i64,
    pub actor: Actor,
    #[serde(default)]
    pub kind: LogKind,
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Column>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Column>,
}

/// Why a task waits for the user, which decides what they're offered.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Stuck {
    /// The work agent failed, or said it couldn't do it.
    WorkFailed,
    /// The review agent's run failed.
    ReviewFailed,
    /// Review sent it back more times than the board allows.
    TooManyRounds,
    /// The review ended without approving or sending it back.
    NoVerdict,
    /// The user stopped its agent.
    Cancelled,
    /// Its agent couldn't even start: no worktree, no agent.
    NotStarted,
}

/// What the user decided on a stuck task.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    /// The same column's agent takes it again, with review rounds from zero.
    Retry,
    /// Back to work, with the user's notes as what to change.
    Rework,
    /// Done as it is.
    Approve,
    /// Back to Not ready, to rewrite it.
    Park,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Success {
    #[default]
    Full,
    Partial,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Outcome {
    pub summary: String,
    pub success: Success,
    pub notes: String,
    pub at: i64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    Approve,
    Changes,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct KanbanTask {
    pub id: String,
    /// What the user and the consoles call it: #1, #2…
    pub number: u32,
    pub title: String,
    /// What it's about, in the user's words.
    pub spec: String,
    pub success_criteria: String,
    /// The user's labels for it: "bug", "frontend"… (see clean_tags).
    pub tags: Vec<String>,
    /// Tasks it waits for, by id: it's taken once they're all done.
    pub depends_on: Vec<String>,
    pub column: Column,

    /// What runs it now, when something does.
    pub role: Option<Role>,
    pub run_id: String,
    pub agent_id: String,
    /// Done waits for its summary agent.
    pub summary_pending: bool,
    /// Times review sent it back.
    pub review_rounds: u32,
    /// The reviewer's last notes, for the next round of work.
    pub review_notes: String,
    /// What the reviewer said through `kanban_review` in the run under way.
    pub verdict: Option<Verdict>,
    /// What the agent of the run under way reported through `kanban_finish`.
    pub report: Option<Outcome>,
    /// What the work agent answered last.
    pub work_summary: String,
    /// Why it's stuck until the user decides; empty when it isn't.
    pub needs_you: String,
    /// The kind of thing it's stuck on, set with needs_you.
    pub stuck: Option<Stuck>,
    /// The last run it was under, to see what the agent did once it's over.
    pub last_agent_id: String,
    pub last_run_id: String,
    /// What the user said when they unstuck it, for the next run only.
    pub instructions: String,
    pub outcome: Option<Outcome>,
    pub cost_usd: Option<f64>,
    /// In worktree mode: its branch and the folder it's checked out in.
    pub branch: String,
    pub worktree: String,
    pub merged: bool,

    pub log: Vec<LogEntry>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl KanbanTask {
    pub fn is_running(&self) -> bool {
        !self.run_id.is_empty()
    }

    pub fn note(&mut self, at: i64, actor: Actor, kind: LogKind, text: impl Into<String>) {
        self.log.push(LogEntry { at, actor, kind, text: text.into(), from: None, to: None });
        if self.log.len() > MAX_LOG {
            let extra = self.log.len() - MAX_LOG;
            self.log.drain(..extra);
        }
        self.updated_at = at;
    }

    /// Whatever ran it no longer does.
    pub fn release(&mut self) {
        if self.is_running() {
            self.last_agent_id = std::mem::take(&mut self.agent_id);
            self.last_run_id = std::mem::take(&mut self.run_id);
        }
        self.role = None;
        self.run_id = String::new();
        self.agent_id = String::new();
        self.verdict = None;
        self.report = None;
    }

    /// It waits for the user, for `reason`.
    pub fn stick(&mut self, stuck: Stuck, reason: impl Into<String>) {
        self.stuck = Some(stuck);
        self.needs_you = reason.into();
    }

    pub fn unstick(&mut self) {
        self.stuck = None;
        self.needs_you = String::new();
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct KanbanBoard {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// The folder its agents work in.
    pub work_dir: String,
    /// On: its agents take tasks. Off: nothing new starts.
    pub enabled: bool,
    pub mode: Mode,
    /// Tasks worked on at once, in worktree mode.
    pub max_parallel: u32,
    pub max_review_rounds: u32,
    /// The branch worktrees start from and merge into; empty: the current one.
    pub base_branch: String,
    pub ask_level: AskLevel,
    pub agents: BoardAgents,
    pub tasks: Vec<KanbanTask>,
    /// The project that holds its agents (see projects.rs).
    pub project_id: String,
    pub next_number: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Default for KanbanBoard {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            icon: String::new(),
            work_dir: String::new(),
            enabled: false,
            mode: Mode::Sequential,
            max_parallel: 2,
            max_review_rounds: 3,
            base_branch: String::new(),
            ask_level: AskLevel::Essential,
            agents: BoardAgents::default(),
            tasks: Vec::new(),
            project_id: String::new(),
            next_number: 1,
            created_at: 0,
            updated_at: 0,
        }
    }
}

impl KanbanBoard {
    pub fn task(&self, id: &str) -> Option<&KanbanTask> {
        self.tasks.iter().find(|task| task.id == id)
    }

    pub fn task_mut(&mut self, id: &str) -> Option<&mut KanbanTask> {
        self.tasks.iter_mut().find(|task| task.id == id)
    }

    /// A task by id or by number ("3", "#3").
    pub fn find(&self, reference: &str) -> Option<&KanbanTask> {
        let reference = reference.trim();
        let number = reference.trim_start_matches('#').parse::<u32>().ok();
        self.tasks.iter().find(|task| task.id == reference || Some(task.number) == number)
    }

    /// How many runs it may have at once.
    pub fn capacity(&self) -> usize {
        match self.mode {
            Mode::Sequential => 1,
            Mode::Worktrees => self.max_parallel.clamp(1, MAX_PARALLEL) as usize,
        }
    }

    pub fn running(&self) -> usize {
        self.tasks.iter().filter(|task| task.is_running()).count()
    }
}

pub const MAX_TAGS: usize = 12;
const MAX_TAG_LEN: usize = 32;

/// Tags as they're kept: trimmed, short, without a leading '#', without
/// repeats (whatever their case), at most MAX_TAGS.
pub fn clean_tags(wanted: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for tag in wanted {
        let tag: String = tag.trim().trim_start_matches('#').trim().chars().take(MAX_TAG_LEN).collect();
        if tag.is_empty() || kept.iter().any(|other| other.to_lowercase() == tag.to_lowercase()) {
            continue;
        }
        kept.push(tag);
        if kept.len() == MAX_TAGS {
            break;
        }
    }
    kept
}

/// Whether all of a task's dependencies are done. One that was deleted no
/// longer holds it back.
pub fn unblocked(board: &KanbanBoard, task: &KanbanTask) -> bool {
    task.depends_on.iter().all(|dep| board.task(dep).is_none_or(|other| other.column == Column::Done))
}

/// The dependencies a task still waits for, by number.
pub fn blockers(board: &KanbanBoard, task: &KanbanTask) -> Vec<u32> {
    task.depends_on
        .iter()
        .filter_map(|dep| board.task(dep))
        .filter(|other| other.column != Column::Done)
        .map(|other| other.number)
        .collect()
}

/// The To do tasks the work agent can take, in the order it takes them: the
/// board's order, skipping what waits on others.
pub fn next_up(board: &KanbanBoard) -> Vec<String> {
    board
        .tasks
        .iter()
        .filter(|task| task.column == Column::Todo && task.needs_you.is_empty() && !task.is_running())
        .filter(|task| unblocked(board, task))
        .map(|task| task.id.clone())
        .collect()
}

/// What the driver starts next, in this order: work sent back by review,
/// work waiting for review, summaries, then new tasks. Each is (task, role).
pub fn wanted(board: &KanbanBoard) -> Vec<(String, Role)> {
    let idle = |task: &&KanbanTask| !task.is_running() && task.needs_you.is_empty();
    let mut wanted: Vec<(String, Role)> = Vec::new();
    for task in board.tasks.iter().filter(idle) {
        match task.column {
            Column::InProgress => wanted.push((task.id.clone(), Role::Work)),
            Column::Review => wanted.push((task.id.clone(), Role::Review)),
            _ => {}
        }
    }
    // The reviews before the work that was sent back: a task on its way out
    // frees its slot first.
    wanted.sort_by_key(|(_, role)| *role != Role::Review);
    if board.agents.summary.is_some() {
        for task in board.tasks.iter().filter(idle).filter(|task| task.column == Column::Done && task.summary_pending) {
            wanted.push((task.id.clone(), Role::Summary));
        }
    }
    wanted.extend(next_up(board).into_iter().map(|id| (id, Role::Work)));
    wanted
}

/// Whether `id` depending on `on` would make a cycle.
fn reaches(board: &KanbanBoard, from: &str, target: &str, seen: &mut HashSet<String>) -> bool {
    if from == target {
        return true;
    }
    if !seen.insert(from.to_string()) {
        return false;
    }
    board.task(from).is_some_and(|task| task.depends_on.iter().any(|dep| reaches(board, dep, target, seen)))
}

/// A task's dependencies as they can be kept: tasks of this board, itself
/// and repeats left out, and none that would make it wait on itself.
pub fn clean_dependencies(board: &KanbanBoard, id: &str, wanted: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for dep in wanted {
        if dep == id || kept.contains(dep) || board.task(dep).is_none() {
            continue;
        }
        if reaches(board, dep, id, &mut HashSet::new()) {
            continue;
        }
        kept.push(dep.clone());
    }
    kept
}

/// Moves a task to `to`, before `before` (another task's id) or at the end
/// of that column. The caller has stopped what ran it. What the move means
/// for the task — its counters, being stuck — is set here.
pub fn move_task(board: &mut KanbanBoard, id: &str, to: Column, before: Option<&str>, at: i64, actor: Actor) -> bool {
    let Some(index) = board.tasks.iter().position(|task| task.id == id) else { return false };
    let mut task = board.tasks.remove(index);
    let from = task.column;
    if from != to {
        task.release();
        task.column = to;
        task.unstick();
        match to {
            Column::NotReady | Column::Todo => {
                task.review_rounds = 0;
                task.review_notes = String::new();
                task.summary_pending = false;
            }
            Column::Done => task.summary_pending = false,
            Column::InProgress | Column::Review => {}
        }
        task.log.push(LogEntry { at, actor, kind: LogKind::Moved, text: String::new(), from: Some(from), to: Some(to) });
        task.updated_at = at;
    }
    let position = before
        .and_then(|before| board.tasks.iter().position(|other| other.id == before))
        .or_else(|| board.tasks.iter().rposition(|other| other.column == to).map(|last| last + 1))
        .unwrap_or(board.tasks.len());
    board.tasks.insert(position, task);
    true
}

/// A board as it may be saved: numbers in range, dependencies that hold.
pub fn sanitize(mut board: KanbanBoard) -> KanbanBoard {
    board.max_parallel = board.max_parallel.clamp(1, MAX_PARALLEL);
    board.max_review_rounds = board.max_review_rounds.clamp(1, MAX_REVIEW_ROUNDS);
    board.tasks.truncate(MAX_TASKS);
    let ids: Vec<String> = board.tasks.iter().map(|task| task.id.clone()).collect();
    for id in ids {
        let wanted = board.task(&id).map(|task| task.depends_on.clone()).unwrap_or_default();
        // One at a time against what's already kept: a cycle loses its last link.
        if let Some(task) = board.task_mut(&id) {
            task.depends_on.clear();
        }
        let kept = clean_dependencies(&board, &id, &wanted);
        if let Some(task) = board.task_mut(&id) {
            task.depends_on = kept;
        }
    }
    let highest = board.tasks.iter().map(|task| task.number).max().unwrap_or(0);
    board.next_number = board.next_number.max(highest + 1);
    board
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn task(id: &str, column: Column, deps: &[&str]) -> KanbanTask {
        KanbanTask {
            id: id.into(),
            title: format!("do {id}"),
            spec: format!("spec of {id}"),
            column,
            depends_on: deps.iter().map(|dep| dep.to_string()).collect(),
            ..KanbanTask::default()
        }
    }

    pub fn board(tasks: Vec<KanbanTask>) -> KanbanBoard {
        let mut board = KanbanBoard { id: "b".into(), name: "Board".into(), work_dir: "/repo".into(), tasks, ..KanbanBoard::default() };
        for (index, task) in board.tasks.iter_mut().enumerate() {
            task.number = index as u32 + 1;
        }
        board
    }

    #[test]
    fn next_up_follows_the_order_and_skips_what_waits() {
        let mut done = task("a", Column::Done, &[]);
        done.outcome = Some(Outcome::default());
        let board = board(vec![
            task("x", Column::NotReady, &[]),
            task("b", Column::Todo, &["c"]),
            task("c", Column::Todo, &[]),
            task("d", Column::Todo, &["a"]),
            done,
        ]);
        assert_eq!(next_up(&board), ["c", "d"]);
        assert_eq!(blockers(&board, board.task("b").unwrap()), [3]);
    }

    #[test]
    fn a_deleted_dependency_no_longer_holds_a_task_back() {
        let board = board(vec![task("b", Column::Todo, &["gone"])]);
        assert_eq!(next_up(&board), ["b"]);
    }

    #[test]
    fn stuck_or_running_tasks_are_not_taken() {
        let mut stuck = task("a", Column::Todo, &[]);
        stuck.needs_you = "why".into();
        let mut running = task("b", Column::Todo, &[]);
        running.run_id = "r".into();
        assert!(next_up(&board(vec![stuck, running])).is_empty());
    }

    #[test]
    fn reviews_come_first_then_returned_work_then_new_tasks() {
        let mut board = board(vec![
            task("new", Column::Todo, &[]),
            task("back", Column::InProgress, &[]),
            task("check", Column::Review, &[]),
            task("sum", Column::Done, &[]),
        ]);
        board.tasks[3].summary_pending = true;
        let order: Vec<(String, Role)> = wanted(&board);
        let ids: Vec<&str> = order.iter().map(|(id, _)| id.as_str()).collect();
        // Without a summary agent nothing is summarized.
        assert_eq!(ids, ["check", "back", "new"]);
        board.agents.summary = Some(ColumnAgent::default());
        let order = wanted(&board);
        assert_eq!(order[2], ("sum".to_string(), Role::Summary));
    }

    #[test]
    fn dependencies_cannot_make_a_cycle() {
        let board = board(vec![task("a", Column::Todo, &["b"]), task("b", Column::Todo, &["c"]), task("c", Column::Todo, &[])]);
        // c on a would close a → b → c → a.
        assert!(clean_dependencies(&board, "c", &["a".into()]).is_empty());
        assert!(clean_dependencies(&board, "c", &["c".into(), "zzz".into()]).is_empty());
        assert_eq!(clean_dependencies(&board, "a", &["c".into(), "c".into()]), ["c"]);

        let mut cyclic = board.clone();
        cyclic.tasks[2].depends_on = vec!["a".into()];
        let cleaned = sanitize(cyclic);
        let total: usize = cleaned.tasks.iter().map(|task| task.depends_on.len()).sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn a_move_reorders_and_resets_what_it_should() {
        let mut board = board(vec![task("a", Column::Todo, &[]), task("b", Column::Todo, &[]), task("c", Column::Review, &[])]);
        {
            let c = board.task_mut("c").unwrap();
            c.review_rounds = 2;
            c.needs_you = "too many".into();
            c.run_id = "r".into();
        }
        assert!(move_task(&mut board, "c", Column::Todo, Some("a"), 5, Actor::User));
        let order: Vec<&str> = board.tasks.iter().map(|task| task.id.as_str()).collect();
        assert_eq!(order, ["c", "a", "b"]);
        let c = board.task("c").unwrap();
        assert_eq!((c.column, c.review_rounds, c.needs_you.as_str(), c.is_running()), (Column::Todo, 0, "", false));
        assert_eq!(c.log.last().unwrap().from, Some(Column::Review));

        // No `before`: to the end of its column.
        assert!(move_task(&mut board, "c", Column::Todo, None, 6, Actor::User));
        let order: Vec<&str> = board.tasks.iter().map(|task| task.id.as_str()).collect();
        assert_eq!(order, ["a", "b", "c"]);
        // A reorder within the column isn't logged as a move.
        assert_eq!(board.task("c").unwrap().log.len(), 1);
        assert!(!move_task(&mut board, "nope", Column::Done, None, 7, Actor::User));
    }

    #[test]
    fn tags_are_trimmed_and_kept_once() {
        let wanted: Vec<String> = ["  bug ", "#Frontend", "BUG", "", "frontend", &"x".repeat(50)].iter().map(|s| s.to_string()).collect();
        assert_eq!(clean_tags(&wanted), ["bug", "Frontend", &"x".repeat(32)]);
        let many: Vec<String> = (0..20).map(|n| format!("t{n}")).collect();
        assert_eq!(clean_tags(&many).len(), MAX_TAGS);
    }

    #[test]
    fn capacity_depends_on_the_mode() {
        let mut board = board(vec![]);
        board.max_parallel = 4;
        assert_eq!(board.capacity(), 1);
        board.mode = Mode::Worktrees;
        assert_eq!(board.capacity(), 4);
        board.max_parallel = 99;
        assert_eq!(sanitize(board).max_parallel, MAX_PARALLEL);
    }

    #[test]
    fn tasks_are_found_by_id_or_number() {
        let board = board(vec![task("a", Column::Todo, &[]), task("b", Column::Todo, &[])]);
        assert_eq!(board.find("#2").unwrap().id, "b");
        assert_eq!(board.find("1").unwrap().id, "a");
        assert_eq!(board.find("b").unwrap().number, 2);
        assert!(board.find("#9").is_none());
    }

    #[test]
    fn the_log_keeps_its_latest_entries() {
        let mut task = task("a", Column::Todo, &[]);
        for at in 0..(MAX_LOG as i64 + 5) {
            task.note(at, Actor::User, LogKind::Note, "x");
        }
        assert_eq!(task.log.len(), MAX_LOG);
        assert_eq!(task.log[0].at, 5);
    }

    #[test]
    fn the_role_of_a_saved_agent_is_found() {
        let mut agents = BoardAgents::default();
        agents.work.agent_ids = vec!["w1".into(), "w2".into()];
        agents.review.agent_ids = vec!["r1".into()];
        assert_eq!(agents.role_of("w2"), Some(Role::Work));
        assert_eq!(agents.role_of("r1"), Some(Role::Review));
        assert_eq!(agents.role_of("zz"), None);
    }
}
