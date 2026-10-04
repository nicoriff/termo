//! An orchestration's plan: the team designed for one piece of work and the
//! tasks each agent takes, with what depends on what. Everything here is
//! plain data and pure rules — what a plan from the model is allowed to
//! contain, which tasks can start, what the plan is expected to cost and
//! whether a change to it needs the user — so it can be tested without
//! running anything.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::super::project_ai::MAX_AGENTS;
use super::super::spec::{is_valid_model, Autonomy, Engine};
use super::pricing::{self, Price};

pub const MAX_TASKS: usize = 24;
/// Runs of one task, the first included.
pub const MAX_ATTEMPTS: u32 = 3;
/// Changes to a plan while it runs; past it the orchestrator can only go on
/// with the plan it has.
pub const MAX_REVISIONS: usize = 10;
/// More than any real task: a number the model made up can't blow the
/// estimate.
const MAX_TASK_TOKENS: u64 = 50_000_000;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    #[default]
    Pending,
    Running,
    Done,
    Failed,
    /// Taken out of the plan; what depended on it no longer waits for it.
    Dropped,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PlannedAgent {
    /// How tasks refer to it.
    pub key: String,
    pub name: String,
    pub icon: String,
    /// What it's good at, in a line.
    pub specialty: String,
    pub engine: Engine,
    pub model: String,
    /// Why this model for this role.
    pub why_model: String,
    pub autonomy: Autonomy,
    pub web: bool,
    pub instructions: String,
    /// The saved agent that runs its tasks, once the plan is approved.
    pub agent_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PlannedTask {
    pub key: String,
    pub title: String,
    /// The request the agent gets.
    pub description: String,
    /// The key of the agent that does it.
    pub agent: String,
    pub depends_on: Vec<String>,
    pub done_when: String,
    /// The planner's guess, all of the task's turns together.
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub status: TaskStatus,
    /// Its current run, or its last one.
    pub run_id: String,
    /// What the agent answered, or why it failed.
    pub summary: String,
    /// What its runs cost, when the CLI says (only Claude Code does).
    pub cost_usd: Option<f64>,
    pub attempts: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Plan {
    pub agents: Vec<PlannedAgent>,
    pub tasks: Vec<PlannedTask>,
}

/// One way of doing the work, for the user to compare with the others.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Strategy {
    /// "economy" | "balanced" | "quality"
    pub key: String,
    pub name: String,
    /// What it trades for what.
    pub summary: String,
    pub plan: Plan,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// Planned, waiting for the user to approve it.
    #[default]
    Draft,
    Running,
    /// The orchestrator wants a change the user has to approve.
    AwaitingApproval,
    /// What it spent went over the budget.
    Paused,
    Done,
    Failed,
    Stopped,
}

/// One change the orchestrator makes to a plan while it runs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Change {
    AddAgent { agent: PlannedAgent },
    AddTask { task: PlannedTask },
    Reassign { task: String, agent: String },
    #[serde(rename_all = "camelCase")]
    ChangeModel { agent: String, engine: Engine, model: String },
    DropTask { task: String },
    RetryTask { task: String },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum RevisionState {
    #[default]
    Applied,
    Pending,
    Rejected,
}

/// Why a revision waits for the user instead of being applied.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Hold {
    #[default]
    None,
    /// The plan would be expected to cost more than the budget.
    Budget,
    /// An agent would be allowed more than any agent the user approved.
    Autonomy,
    /// Too many changes already.
    Limit,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Revision {
    pub id: String,
    /// Unix milliseconds.
    pub at: i64,
    /// The orchestrator's reason, for the user.
    pub note: String,
    pub changes: Vec<Change>,
    pub state: RevisionState,
    pub hold: Hold,
    /// What the whole plan is expected to cost with it.
    pub estimate_usd: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Orchestration {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// What the user asked for.
    pub prompt: String,
    pub work_dir: String,
    pub status: Status,
    /// The Claude model that plans and supervises; empty: the CLI's default.
    pub orchestrator_model: String,
    pub strategies: Vec<Strategy>,
    /// The key of the strategy the planner would pick, and why.
    pub recommended: String,
    pub why: String,
    /// The key of the strategy the user approved.
    pub chosen: String,
    /// The most the user agreed to spend; 0: no limit.
    pub budget_usd: f64,
    /// The most any agent of the approved plan may do.
    pub max_autonomy: Autonomy,
    /// The plan as it stands: the approved one, with every change since.
    pub plan: Plan,
    pub revisions: Vec<Revision>,
    /// The team its agents are saved as (see projects.rs).
    pub project_id: String,
    /// What planning and supervising cost.
    pub overhead_usd: f64,
    /// The orchestrator's last word: what it's waiting for, why it stopped,
    /// or how the work went.
    pub note: String,
    /// Unix seconds.
    pub created_at: i64,
    pub updated_at: i64,
}

/// A model an engine offers on this computer.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price: Option<Price>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEngine {
    pub engine: Engine,
    /// Its CLI is on this computer.
    pub installed: bool,
    pub models: Vec<CatalogModel>,
}

/// What a team can be made of here: each engine's CLI and its models.
pub type Catalog = Vec<CatalogEngine>;

/// A key as tasks and agents refer to each other: lowercase, no spaces.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(40).collect()
}

fn short(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

/// The engine an agent runs on here: the one asked for when its CLI is
/// installed, otherwise the first that is.
fn usable_engine(engine: Engine, catalog: &Catalog) -> Engine {
    let installed = |wanted: Engine| catalog.iter().any(|entry| entry.engine == wanted && entry.installed);
    if installed(engine) {
        return engine;
    }
    catalog.iter().find(|entry| entry.installed).map_or(Engine::Claude, |entry| entry.engine)
}

/// The model as asked for when the engine has it (or lists none to check
/// against); otherwise the CLI's own default.
fn usable_model(engine: Engine, model: &str, catalog: &Catalog) -> String {
    let model = model.trim();
    if !is_valid_model(model) {
        return String::new();
    }
    let known = catalog.iter().find(|entry| entry.engine == engine).map(|entry| &entry.models);
    match known {
        Some(models) if !models.is_empty() && !models.iter().any(|option| option.id == model) => String::new(),
        _ => model.to_string(),
    }
}

/// Whether `from` reaches `to` following what each task depends on.
fn reaches(deps: &HashMap<String, Vec<String>>, from: &str, to: &str) -> bool {
    let mut seen = HashSet::new();
    let mut stack = vec![from.to_string()];
    while let Some(key) = stack.pop() {
        if key == to {
            return true;
        }
        if seen.insert(key.clone()) {
            stack.extend(deps.get(&key).into_iter().flatten().cloned());
        }
    }
    false
}

/// A plan as the model (or a change to it) wrote it, made safe to show and
/// run: keys cleaned up and unique, engines and models this computer has,
/// tasks only for agents that exist, no dependency on something that isn't
/// there or that closes a loop, and nothing past the limits.
pub fn sanitize(plan: Plan, catalog: &Catalog) -> Plan {
    let mut agents: Vec<PlannedAgent> = Vec::new();
    for mut agent in plan.agents {
        agent.key = slug(&agent.key);
        if agent.key.is_empty() || agents.iter().any(|other| other.key == agent.key) || agents.len() >= MAX_AGENTS {
            continue;
        }
        agent.engine = usable_engine(agent.engine, catalog);
        agent.model = usable_model(agent.engine, &agent.model, catalog);
        agent.name = short(&agent.name, 60);
        if agent.name.is_empty() {
            agent.name = agent.key.clone();
        }
        agent.icon = agent.icon.trim().chars().take(2).collect();
        agent.specialty = short(&agent.specialty, 200);
        agent.why_model = short(&agent.why_model, 400);
        agent.instructions = agent.instructions.trim().to_string();
        agents.push(agent);
    }

    let mut tasks: Vec<PlannedTask> = Vec::new();
    for mut task in plan.tasks {
        task.key = slug(&task.key);
        task.agent = slug(&task.agent);
        if task.key.is_empty()
            || tasks.iter().any(|other| other.key == task.key)
            || tasks.len() >= MAX_TASKS
            || !agents.iter().any(|agent| agent.key == task.agent)
            || task.description.trim().is_empty()
        {
            continue;
        }
        task.title = short(&task.title, 120);
        if task.title.is_empty() {
            task.title = task.key.clone();
        }
        task.description = task.description.trim().to_string();
        task.done_when = short(&task.done_when, 400);
        task.tokens_in = task.tokens_in.min(MAX_TASK_TOKENS);
        task.tokens_out = task.tokens_out.min(MAX_TASK_TOKENS);
        tasks.push(task);
    }

    // Dependencies, one at a time: kept only when the task it names exists
    // and adding it doesn't close a loop.
    let keys: HashSet<String> = tasks.iter().map(|task| task.key.clone()).collect();
    let mut kept: HashMap<String, Vec<String>> = HashMap::new();
    for task in &mut tasks {
        let mut deps: Vec<String> = Vec::new();
        for dep in task.depends_on.iter().map(|dep| slug(dep)) {
            if dep == task.key || !keys.contains(&dep) || deps.contains(&dep) || reaches(&kept, &dep, &task.key) {
                continue;
            }
            deps.push(dep.clone());
            kept.entry(task.key.clone()).or_default().push(dep);
        }
        task.depends_on = deps;
    }

    // An agent with nothing to do isn't part of the team — unless it already
    // ran, and its history is part of what happened.
    agents.retain(|agent| !agent.agent_id.is_empty() || tasks.iter().any(|task| task.agent == agent.key));
    Plan { agents, tasks }
}

pub fn agent_of<'a>(plan: &'a Plan, task: &PlannedTask) -> Option<&'a PlannedAgent> {
    plan.agents.iter().find(|agent| agent.key == task.agent)
}

/// What one run of the task is expected to cost; `None` when its model has
/// no known price.
pub fn task_estimate(plan: &Plan, task: &PlannedTask) -> Option<f64> {
    let agent = agent_of(plan, task)?;
    let price = pricing::price_of(agent.engine, &agent.model)?;
    Some(pricing::cost(price, task.tokens_in, task.tokens_out))
}

/// What the whole plan is expected to cost, start to end: what its tasks
/// already cost where that's known, and the estimate for whatever is still
/// to run or didn't say. Tasks without a price count as nothing.
pub fn projected_usd(plan: &Plan) -> f64 {
    plan.tasks
        .iter()
        .map(|task| {
            let estimate = task_estimate(plan, task).unwrap_or(0.0);
            match (task.status, task.cost_usd) {
                (TaskStatus::Dropped, cost) => cost.unwrap_or(0.0),
                (TaskStatus::Done, Some(cost)) => cost,
                // Still to run (again): what it cost so far, plus one more run.
                (_, Some(cost)) => cost + estimate,
                (_, None) => estimate,
            }
        })
        .sum()
}

/// What its runs are known to have cost so far.
pub fn spent_usd(plan: &Plan) -> f64 {
    plan.tasks.iter().filter_map(|task| task.cost_usd).sum()
}

pub fn max_autonomy(plan: &Plan) -> Autonomy {
    plan.agents.iter().map(|agent| agent.autonomy).max().unwrap_or_default()
}

fn settled(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Done | TaskStatus::Dropped)
}

/// Every task is done (or dropped).
pub fn finished(plan: &Plan) -> bool {
    plan.tasks.iter().all(|task| settled(task.status))
}

/// The tasks that can start now: waiting, with everything they depend on
/// done, their agent free, and — for the ones that change files — nobody
/// else changing files at the same time, since the whole team works in one
/// folder.
pub fn ready(plan: &Plan) -> Vec<String> {
    let status: HashMap<&str, TaskStatus> = plan.tasks.iter().map(|task| (task.key.as_str(), task.status)).collect();
    let writes = |task: &PlannedTask| agent_of(plan, task).is_some_and(|agent| agent.autonomy != Autonomy::Read);
    let mut busy: HashSet<&str> = HashSet::new();
    let mut writing = false;
    for task in plan.tasks.iter().filter(|task| task.status == TaskStatus::Running) {
        busy.insert(task.agent.as_str());
        writing |= writes(task);
    }
    let mut ready = Vec::new();
    for task in plan.tasks.iter().filter(|task| task.status == TaskStatus::Pending) {
        let unblocked = task.depends_on.iter().all(|dep| status.get(dep.as_str()).is_none_or(|status| settled(*status)));
        if !unblocked || busy.contains(task.agent.as_str()) || (writes(task) && writing) {
            continue;
        }
        busy.insert(task.agent.as_str());
        writing |= writes(task);
        ready.push(task.key.clone());
    }
    ready
}

/// The plan with the orchestrator's changes made. Only what hasn't run can
/// be reassigned or dropped, only a failed task runs again (retried, or
/// handed to another agent), and a new agent or task never replaces one
/// that's there.
pub fn apply(plan: &Plan, changes: &[Change], catalog: &Catalog) -> Plan {
    let mut plan = plan.clone();
    let open = |status: TaskStatus| matches!(status, TaskStatus::Pending | TaskStatus::Failed);
    for change in changes {
        match change {
            Change::AddAgent { agent } => {
                let key = slug(&agent.key);
                if !plan.agents.iter().any(|other| other.key == key) {
                    plan.agents.push(PlannedAgent { key, agent_id: String::new(), ..agent.clone() });
                }
            }
            Change::AddTask { task } => {
                let key = slug(&task.key);
                if !plan.tasks.iter().any(|other| other.key == key) {
                    plan.tasks.push(PlannedTask {
                        key,
                        status: TaskStatus::Pending,
                        run_id: String::new(),
                        summary: String::new(),
                        cost_usd: None,
                        attempts: 0,
                        ..task.clone()
                    });
                }
            }
            Change::Reassign { task, agent } => {
                let (task, agent) = (slug(task), slug(agent));
                if plan.agents.iter().any(|other| other.key == agent) {
                    if let Some(found) = plan.tasks.iter_mut().find(|other| other.key == task && open(other.status)) {
                        found.agent = agent;
                        // Someone else gets to try what failed.
                        if found.status == TaskStatus::Failed && found.attempts < MAX_ATTEMPTS {
                            found.status = TaskStatus::Pending;
                        }
                    }
                }
            }
            Change::ChangeModel { agent, engine, model } => {
                let agent = slug(agent);
                if let Some(found) = plan.agents.iter_mut().find(|other| other.key == agent) {
                    found.engine = *engine;
                    found.model = model.clone();
                }
            }
            Change::DropTask { task } => {
                let task = slug(task);
                if let Some(found) = plan.tasks.iter_mut().find(|other| other.key == task && open(other.status)) {
                    found.status = TaskStatus::Dropped;
                }
            }
            Change::RetryTask { task } => {
                let task = slug(task);
                let failed = |other: &&mut PlannedTask| other.key == task && other.status == TaskStatus::Failed;
                if let Some(found) = plan.tasks.iter_mut().find(failed).filter(|found| found.attempts < MAX_ATTEMPTS) {
                    found.status = TaskStatus::Pending;
                }
            }
        }
    }
    sanitize(plan, catalog)
}

/// Whether a changed plan can go ahead on its own: it has to stay within
/// what the user approved — the budget, and how much the agents may do.
pub fn hold_for(orchestration: &Orchestration, changed: &Plan) -> Hold {
    let applied = orchestration.revisions.iter().filter(|revision| revision.state == RevisionState::Applied).count();
    if applied >= MAX_REVISIONS {
        return Hold::Limit;
    }
    if max_autonomy(changed) > orchestration.max_autonomy {
        return Hold::Autonomy;
    }
    let budget = orchestration.budget_usd;
    if budget > 0.0 && projected_usd(changed) + orchestration.overhead_usd > budget {
        return Hold::Budget;
    }
    Hold::None
}

/// What it's known to have spent is over the budget.
pub fn over_budget(orchestration: &Orchestration) -> bool {
    let budget = orchestration.budget_usd;
    budget > 0.0 && spent_usd(&orchestration.plan) + orchestration.overhead_usd > budget
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub fn catalog() -> Catalog {
        let model = |id: &str, engine: Engine| CatalogModel {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            price: pricing::price_of(engine, id),
        };
        vec![
            CatalogEngine {
                engine: Engine::Claude,
                installed: true,
                models: vec![model("claude-opus-5-5", Engine::Claude), model("claude-haiku-4-5", Engine::Claude)],
            },
            CatalogEngine { engine: Engine::Codex, installed: true, models: vec![model("gpt-5.5", Engine::Codex)] },
            CatalogEngine { engine: Engine::Gemini, installed: false, models: Vec::new() },
        ]
    }

    pub fn agent(key: &str, autonomy: Autonomy) -> PlannedAgent {
        PlannedAgent {
            key: key.into(),
            name: key.into(),
            engine: Engine::Claude,
            model: "claude-haiku-4-5".into(),
            autonomy,
            ..PlannedAgent::default()
        }
    }

    pub fn task(key: &str, agent: &str, deps: &[&str]) -> PlannedTask {
        PlannedTask {
            key: key.into(),
            title: key.into(),
            description: format!("do {key}"),
            agent: agent.into(),
            depends_on: deps.iter().map(|dep| dep.to_string()).collect(),
            tokens_in: 1_000_000,
            tokens_out: 200_000,
            ..PlannedTask::default()
        }
    }

    fn plan(agents: Vec<PlannedAgent>, tasks: Vec<PlannedTask>) -> Plan {
        sanitize(Plan { agents, tasks }, &catalog())
    }

    #[test]
    fn keeps_only_what_this_computer_can_run() {
        let mut gemini = agent("Researcher One", Autonomy::Read);
        gemini.engine = Engine::Gemini;
        gemini.model = "pro".into();
        let mut unknown = agent("coder", Autonomy::Edit);
        unknown.model = "claude-made-up".into();
        let plan = plan(vec![gemini, unknown], vec![task("a", "researcher-one", &[]), task("b", "coder", &[])]);
        // Gemini isn't installed: the first engine that is, on its default.
        assert_eq!(plan.agents[0].key, "researcher-one");
        assert_eq!((plan.agents[0].engine, plan.agents[0].model.as_str()), (Engine::Claude, ""));
        assert_eq!(plan.agents[1].model, "");
    }

    #[test]
    fn drops_what_points_nowhere_and_what_closes_a_loop() {
        let plan = plan(
            vec![agent("dev", Autonomy::Edit), agent("idle", Autonomy::Read)],
            vec![
                task("a", "dev", &["c", "ghost", "a"]),
                task("b", "dev", &["a"]),
                task("c", "dev", &["b"]),
                task("d", "nobody", &[]),
                task("a", "dev", &[]),
            ],
        );
        let deps: Vec<(&str, Vec<&str>)> =
            plan.tasks.iter().map(|task| (task.key.as_str(), task.depends_on.iter().map(String::as_str).collect())).collect();
        // "a" was read first, so its dependency on "c" stands and it's
        // "c" → "b" that would close the loop.
        assert_eq!(deps, [("a", vec!["c"]), ("b", vec!["a"]), ("c", vec![])]);
        assert_eq!(plan.agents.len(), 1);
    }

    #[test]
    fn one_writer_at_a_time_and_readers_alongside() {
        let mut plan = plan(
            vec![agent("dev", Autonomy::Edit), agent("ui", Autonomy::Edit), agent("docs", Autonomy::Read)],
            vec![task("api", "dev", &[]), task("screen", "ui", &[]), task("notes", "docs", &[]), task("tests", "dev", &["api"])],
        );
        assert_eq!(ready(&plan), ["api", "notes"]);
        plan.tasks[0].status = TaskStatus::Running;
        plan.tasks[2].status = TaskStatus::Running;
        assert!(ready(&plan).is_empty());
        plan.tasks[0].status = TaskStatus::Done;
        // "tests" comes after "screen" in the list, and both write.
        assert_eq!(ready(&plan), ["screen"]);
        plan.tasks[1].status = TaskStatus::Dropped;
        assert_eq!(ready(&plan), ["tests"]);
    }

    #[test]
    fn a_failed_dependency_blocks_and_a_dropped_one_does_not() {
        let mut plan = plan(vec![agent("dev", Autonomy::Edit)], vec![task("a", "dev", &[]), task("b", "dev", &["a"])]);
        plan.tasks[0].status = TaskStatus::Failed;
        assert!(ready(&plan).is_empty());
        assert!(!finished(&plan));
        plan.tasks[0].status = TaskStatus::Dropped;
        assert_eq!(ready(&plan), ["b"]);
    }

    #[test]
    fn the_projection_uses_what_is_known_and_estimates_the_rest() {
        let mut plan = plan(vec![agent("dev", Autonomy::Edit)], vec![task("a", "dev", &[]), task("b", "dev", &["a"])]);
        // Haiku: 1M in at $1 + 200k out at $5 = $2 a task.
        assert!((projected_usd(&plan) - 4.0).abs() < 1e-9);
        plan.tasks[0].status = TaskStatus::Done;
        plan.tasks[0].cost_usd = Some(0.5);
        assert!((projected_usd(&plan) - 2.5).abs() < 1e-9);
        assert!((spent_usd(&plan) - 0.5).abs() < 1e-9);
        // A failed run is paid for, and running it again costs once more.
        plan.tasks[1].status = TaskStatus::Failed;
        plan.tasks[1].cost_usd = Some(1.0);
        assert!((projected_usd(&plan) - 3.5).abs() < 1e-9);
    }

    #[test]
    fn changes_touch_only_what_has_not_run() {
        let mut plan = plan(vec![agent("dev", Autonomy::Edit)], vec![task("a", "dev", &[]), task("b", "dev", &["a"])]);
        plan.tasks[0].status = TaskStatus::Done;
        plan.agents[0].agent_id = "saved".into();
        let changes = vec![
            Change::AddAgent { agent: agent("QA", Autonomy::Read) },
            Change::AddTask { task: PlannedTask { status: TaskStatus::Done, ..task("check", "qa", &["b"]) } },
            Change::Reassign { task: "a".into(), agent: "qa".into() },
            Change::DropTask { task: "a".into() },
            Change::ChangeModel { agent: "dev".into(), engine: Engine::Codex, model: "gpt-5.5".into() },
            Change::AddTask { task: task("b", "qa", &[]) },
        ];
        let changed = apply(&plan, &changes, &catalog());
        assert_eq!(changed.tasks[0].agent, "dev");
        assert_eq!(changed.tasks[0].status, TaskStatus::Done);
        assert_eq!(changed.tasks.len(), 3);
        assert_eq!(changed.tasks[2].status, TaskStatus::Pending);
        assert_eq!((changed.agents[0].engine, changed.agents[0].model.as_str()), (Engine::Codex, "gpt-5.5"));
        assert_eq!(changed.agents[1].key, "qa");
    }

    #[test]
    fn a_task_is_retried_only_so_many_times() {
        let mut plan = plan(vec![agent("dev", Autonomy::Edit)], vec![task("a", "dev", &[])]);
        plan.tasks[0].status = TaskStatus::Failed;
        plan.tasks[0].attempts = 1;
        let retry = [Change::RetryTask { task: "a".into() }];
        assert_eq!(apply(&plan, &retry, &catalog()).tasks[0].status, TaskStatus::Pending);
        plan.tasks[0].attempts = MAX_ATTEMPTS;
        assert_eq!(apply(&plan, &retry, &catalog()).tasks[0].status, TaskStatus::Failed);
    }

    #[test]
    fn a_change_past_the_budget_or_the_permissions_waits_for_the_user() {
        let base = plan(vec![agent("dev", Autonomy::Edit)], vec![task("a", "dev", &[])]);
        let orchestration = Orchestration {
            budget_usd: 5.0,
            max_autonomy: Autonomy::Edit,
            overhead_usd: 0.5,
            plan: base.clone(),
            ..Orchestration::default()
        };
        let one_more = apply(&base, &[Change::AddTask { task: task("b", "dev", &["a"]) }], &catalog());
        assert_eq!(hold_for(&orchestration, &one_more), Hold::None);
        let two_more = apply(&one_more, &[Change::AddTask { task: task("c", "dev", &["b"]) }], &catalog());
        assert_eq!(hold_for(&orchestration, &two_more), Hold::Budget);
        let shell = apply(
            &base,
            &[Change::AddAgent { agent: agent("ops", Autonomy::Run) }, Change::AddTask { task: task("deploy", "ops", &[]) }],
            &catalog(),
        );
        assert_eq!(hold_for(&orchestration, &shell), Hold::Autonomy);
        let unlimited = Orchestration { budget_usd: 0.0, ..orchestration.clone() };
        assert_eq!(hold_for(&unlimited, &two_more), Hold::None);
    }

    #[test]
    fn spending_past_the_budget_is_noticed() {
        let mut base = plan(vec![agent("dev", Autonomy::Edit)], vec![task("a", "dev", &[])]);
        base.tasks[0].cost_usd = Some(4.8);
        let orchestration = Orchestration { budget_usd: 5.0, overhead_usd: 0.3, plan: base, ..Orchestration::default() };
        assert!(over_budget(&orchestration));
        assert!(!over_budget(&Orchestration { budget_usd: 0.0, ..orchestration }));
    }
}
