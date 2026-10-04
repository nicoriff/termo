//! Projects: agents grouped under a name. At its simplest a project is a
//! folder in the agents panel; a project can also be a team — its agents
//! then only talk among themselves and share a board of their own — with an
//! entry agent that starts its work, a daily limit on runs, and a history of
//! versions to go back to.
//!
//! On disk: `app_data/agents/projects/<id>/project.json`, its board and its
//! `versions/`. Which project an agent is in lives on the agent
//! (`AgentSpec::project_id`), so moving one is saving it.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};

use crate::i18n::{tr, Msg};

use super::spec::{is_valid_id, AgentSpec, Autonomy};
use super::store::{self, now_millis, now_seconds, RunRecord, RunTrigger};

/// Versions kept per project; older ones are dropped.
const KEEP_VERSIONS: usize = 30;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectLimits {
    /// Runs a day for all its agents together (tests don't count); 0: no limit.
    pub max_runs_per_day: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// What the project is for, in the user's words.
    pub goal: String,
    /// New agents in the project start with this folder.
    pub work_dir: String,
    /// A team: its agents only talk among themselves and share its own board.
    pub isolated: bool,
    /// The agent "Run project" starts.
    pub entry_agent_id: String,
    pub limits: ProjectLimits,
    /// Set on the team an orchestration runs (see orchestration/): it's
    /// shown with its orchestration, not among the user's own projects.
    pub orchestration_id: String,
    /// Unix seconds.
    pub created_at: i64,
    pub updated_at: i64,
}

fn projects_dir(root: &Path) -> PathBuf {
    root.join("projects")
}

fn project_dir(root: &Path, id: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(is_valid_id(id), "invalid project id");
    Ok(projects_dir(root).join(id))
}

fn write_atomic(path: &Path, text: &str) -> anyhow::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Every project, by name.
pub fn list_in(root: &Path) -> Vec<Project> {
    let mut projects: Vec<Project> = fs::read_dir(projects_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.path().join("project.json")).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    projects.sort_by_key(|project| project.name.to_lowercase());
    projects
}

pub fn get_in(root: &Path, id: &str) -> Option<Project> {
    let dir = project_dir(root, id).ok()?;
    serde_json::from_str(&fs::read_to_string(dir.join("project.json")).ok()?).ok()
}

pub fn save_in(root: &Path, mut project: Project) -> anyhow::Result<Vec<Project>> {
    let dir = project_dir(root, &project.id)?;
    fs::create_dir_all(&dir)?;
    let now = now_seconds();
    if project.created_at == 0 {
        project.created_at = now;
    }
    project.updated_at = now;
    write_atomic(&dir.join("project.json"), &serde_json::to_string_pretty(&project)?)?;
    Ok(list_in(root))
}

/// The project's folder, with its board and versions. Its agents are the
/// caller's to delete or move out first.
pub fn delete_in(root: &Path, id: &str) -> anyhow::Result<Vec<Project>> {
    let dir = project_dir(root, id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(list_in(root))
}

pub fn agents_of(root: &Path, project_id: &str) -> Vec<AgentSpec> {
    store::list_in(root).into_iter().filter(|spec| spec.project_id == project_id).collect()
}

/// The board an agent reads and writes: its team's own, or the one shared by
/// every agent outside a team.
pub fn board_for(root: &Path, spec: &AgentSpec) -> PathBuf {
    match get_in(root, &spec.project_id).filter(|project| project.isolated) {
        Some(project) => projects_dir(root).join(project.id).join("board.json"),
        None => root.join("board.json"),
    }
}

/// The board of a project, or the common one for `None`.
pub fn board_path(root: &Path, project_id: Option<&str>) -> PathBuf {
    match project_id.filter(|id| is_valid_id(id)) {
        Some(id) => projects_dir(root).join(id).join("board.json"),
        None => root.join("board.json"),
    }
}

/// The ids of the projects that are teams.
pub fn isolated_ids(root: &Path) -> HashSet<String> {
    list_in(root).into_iter().filter(|project| project.isolated).map(|project| project.id).collect()
}

/// Whether two agents may work together: always, unless one of them is in a
/// team the other isn't part of.
pub fn can_talk(a: &AgentSpec, b: &AgentSpec, isolated: &HashSet<String>) -> bool {
    let in_team = |spec: &AgentSpec| !spec.project_id.is_empty() && isolated.contains(&spec.project_id);
    if in_team(a) || in_team(b) {
        a.project_id == b.project_id
    } else {
        true
    }
}

/// Local midnight today, in Unix milliseconds.
fn today_start_ms(now_ms: i64) -> i64 {
    Local
        .timestamp_millis_opt(now_ms)
        .single()
        .and_then(|now| now.date_naive().and_hms_opt(0, 0, 0))
        .and_then(|midnight| Local.from_local_datetime(&midnight).earliest())
        .map_or(now_ms, |midnight| midnight.timestamp_millis())
}

/// Every run of the project's agents, newest first.
pub fn runs_of(root: &Path, project_id: &str) -> Vec<RunRecord> {
    let mut runs: Vec<RunRecord> =
        agents_of(root, project_id).iter().flat_map(|spec| store::list_runs_in(root, &spec.id)).collect();
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at));
    runs
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub runs_today: u32,
    pub cost_today: f64,
    /// The last week's cost, today included.
    pub cost_week: f64,
    pub max_runs_per_day: u32,
}

fn counts(run: &RunRecord) -> bool {
    run.trigger != RunTrigger::Test
}

pub fn stats(root: &Path, project: &Project, now_ms: i64) -> Stats {
    let today = today_start_ms(now_ms);
    let week = today - 6 * 86_400_000;
    let runs = runs_of(root, &project.id);
    let cost_since = |from: i64| runs.iter().filter(|run| run.started_at >= from).filter_map(|run| run.cost_usd).sum();
    Stats {
        runs_today: runs.iter().filter(|run| counts(run) && run.started_at >= today).count() as u32,
        cost_today: cost_since(today),
        cost_week: cost_since(week),
        max_runs_per_day: project.limits.max_runs_per_day,
    }
}

/// Refuses a run of an agent whose project already made its runs for today.
pub fn check_daily_limit(root: &Path, spec: &AgentSpec) -> Result<(), String> {
    if spec.project_id.is_empty() {
        return Ok(());
    }
    let Some(project) = get_in(root, &spec.project_id) else { return Ok(()) };
    let limit = project.limits.max_runs_per_day;
    if limit > 0 && stats(root, &project, now_millis()).runs_today >= limit {
        return Err(tr(Msg::ProjectDailyLimit(limit)));
    }
    Ok(())
}

/// What a run in a rehearsal actually uses: it only looks — no editing, no
/// commands, no integrations that could act elsewhere — and says what it
/// would have done.
pub fn rehearsal_spec(spec: &AgentSpec) -> AgentSpec {
    let mut spec = spec.clone();
    spec.autonomy = Autonomy::Read;
    spec.tools.mcp.clear();
    spec.secrets.clear();
    let note = "THIS IS A REHEARSAL. You can only look: nothing you do may change anything. Where your task \
would have you change, write or send something, describe exactly what you would do instead, and finish \
with that description.";
    spec.instructions = if spec.instructions.trim().is_empty() {
        note.to_string()
    } else {
        format!("{}\n\n{note}", spec.instructions.trim())
    };
    spec
}

/// A project as it was at one moment: itself and its agents.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub project: Project,
    pub agents: Vec<AgentSpec>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    /// Unix milliseconds; also the version's name.
    pub at: i64,
    pub agents: Vec<String>,
}

fn versions_dir(root: &Path, id: &str) -> anyhow::Result<PathBuf> {
    Ok(project_dir(root, id)?.join("versions"))
}

/// A snapshot with what changes on every save taken out, to tell whether
/// anything really changed.
fn comparable(snapshot: &Snapshot) -> String {
    let mut copy = snapshot.clone();
    copy.project.updated_at = 0;
    for agent in &mut copy.agents {
        agent.updated_at = 0;
    }
    copy.agents.sort_by(|a, b| a.id.cmp(&b.id));
    serde_json::to_string(&copy).unwrap_or_default()
}

fn version_files(root: &Path, id: &str) -> Vec<(i64, PathBuf)> {
    let Ok(dir) = versions_dir(root, id) else { return Vec::new() };
    let mut files: Vec<(i64, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let at = path.file_stem()?.to_str()?.parse().ok()?;
            (path.extension()? == "json").then_some((at, path))
        })
        .collect();
    files.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    files
}

/// Records the project as it is now, unless nothing changed since the last
/// version.
pub fn snapshot(root: &Path, id: &str) {
    let Some(project) = get_in(root, id) else { return };
    let snapshot = Snapshot { project, agents: agents_of(root, id) };
    let latest = version_files(root, id)
        .first()
        .and_then(|(_, path)| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Snapshot>(&text).ok());
    if latest.is_some_and(|latest| comparable(&latest) == comparable(&snapshot)) {
        return;
    }
    let Ok(dir) = versions_dir(root, id) else { return };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    // Two saves in the same millisecond keep the later one.
    let _ = write_atomic(&dir.join(format!("{}.json", now_millis())), &serde_json::to_string(&snapshot).unwrap_or_default());
    for (_, path) in version_files(root, id).into_iter().skip(KEEP_VERSIONS) {
        let _ = fs::remove_file(path);
    }
}

/// Newest first.
pub fn versions(root: &Path, id: &str) -> Vec<VersionInfo> {
    version_files(root, id)
        .into_iter()
        .filter_map(|(at, path)| {
            let snapshot: Snapshot = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
            Some(VersionInfo { at, agents: snapshot.agents.into_iter().map(|agent| agent.name).collect() })
        })
        .collect()
}

pub fn load_version(root: &Path, id: &str, at: i64) -> Option<Snapshot> {
    let path = versions_dir(root, id).ok()?.join(format!("{at}.json"));
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::store::RunStatus;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-projects-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn project(id: &str, name: &str) -> Project {
        Project { id: id.into(), name: name.into(), ..Project::default() }
    }

    fn agent(id: &str, project_id: &str) -> AgentSpec {
        AgentSpec { id: id.into(), name: id.to_uppercase(), project_id: project_id.into(), ..AgentSpec::default() }
    }

    fn run(agent_id: &str, id: &str, started_at: i64, trigger: RunTrigger, cost: f64) -> RunRecord {
        RunRecord {
            id: id.into(),
            agent_id: agent_id.into(),
            trigger,
            status: RunStatus::Success,
            started_at,
            ended_at: Some(started_at + 1),
            summary: String::new(),
            activity: String::new(),
            cost_usd: Some(cost),
            link: None,
            rehearsal: false,
        }
    }

    #[test]
    fn projects_are_listed_by_name_and_hold_nothing_but_themselves() {
        let root = temp_root("list");
        save_in(&root, project("b", "zeta")).unwrap();
        let list = save_in(&root, project("a", "Alfa")).unwrap();
        assert_eq!(list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Alfa", "zeta"]);
        // The projects folder isn't mistaken for an agent.
        assert!(store::list_in(&root).is_empty());
        assert!(save_in(&root, project("../x", "bad")).is_err());
        assert!(delete_in(&root, "a").unwrap().len() == 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn only_teams_keep_their_agents_to_themselves() {
        let isolated: HashSet<String> = ["team".to_string()].into();
        let (a, b) = (agent("a", "team"), agent("b", "team"));
        let (loose, folder) = (agent("c", ""), agent("d", "folder"));
        assert!(can_talk(&a, &b, &isolated));
        assert!(!can_talk(&a, &loose, &isolated));
        assert!(!can_talk(&loose, &a, &isolated));
        assert!(can_talk(&loose, &folder, &isolated));
    }

    #[test]
    fn a_team_has_its_own_board() {
        let root = temp_root("board");
        save_in(&root, Project { isolated: true, ..project("team", "Team") }).unwrap();
        save_in(&root, project("folder", "Folder")).unwrap();
        assert!(board_for(&root, &agent("a", "team")).ends_with("projects/team/board.json"));
        assert_eq!(board_for(&root, &agent("b", "folder")), root.join("board.json"));
        assert_eq!(board_for(&root, &agent("c", "")), root.join("board.json"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn todays_runs_count_against_the_limit_and_tests_dont() {
        let root = temp_root("limit");
        save_in(&root, Project { limits: ProjectLimits { max_runs_per_day: 2 }, ..project("p", "P") }).unwrap();
        store::save_in(&root, agent("a", "p")).unwrap();
        let now = now_millis();
        store::save_run_in(&root, &run("a", "r1", now - 1000, RunTrigger::Manual, 0.5)).unwrap();
        store::save_run_in(&root, &run("a", "r2", now - 900, RunTrigger::Test, 0.25)).unwrap();
        store::save_run_in(&root, &run("a", "old", now - 10 * 86_400_000, RunTrigger::Manual, 9.0)).unwrap();
        let stats = stats(&root, &get_in(&root, "p").unwrap(), now);
        assert_eq!(stats.runs_today, 1);
        assert!((stats.cost_today - 0.75).abs() < 1e-9);
        assert!((stats.cost_week - 0.75).abs() < 1e-9);
        assert!(check_daily_limit(&root, &agent("a", "p")).is_ok());
        store::save_run_in(&root, &run("a", "r3", now - 800, RunTrigger::Schedule, 0.0)).unwrap();
        assert!(check_daily_limit(&root, &agent("a", "p")).is_err());
        assert!(check_daily_limit(&root, &agent("x", "")).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_rehearsal_only_looks() {
        let mut spec = agent("a", "");
        spec.autonomy = Autonomy::Run;
        spec.instructions = "Pasos".into();
        spec.secrets = vec!["X".into()];
        spec.tools.mcp.push(crate::agents::spec::McpServer::default());
        let rehearsal = rehearsal_spec(&spec);
        assert_eq!(rehearsal.autonomy, Autonomy::Read);
        assert!(rehearsal.tools.mcp.is_empty() && rehearsal.secrets.is_empty());
        assert!(rehearsal.instructions.starts_with("Pasos") && rehearsal.instructions.contains("REHEARSAL"));
    }

    #[test]
    fn versions_are_kept_only_when_something_changed() {
        let root = temp_root("versions");
        save_in(&root, project("p", "P")).unwrap();
        store::save_in(&root, agent("a", "p")).unwrap();
        snapshot(&root, "p");
        snapshot(&root, "p");
        assert_eq!(versions(&root, "p").len(), 1);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let mut changed = agent("a", "p");
        changed.task = "otra cosa".into();
        store::save_in(&root, changed).unwrap();
        snapshot(&root, "p");
        let list = versions(&root, "p");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].agents, ["A"]);
        let first = load_version(&root, "p", list[1].at).unwrap();
        assert!(first.agents[0].task.is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
