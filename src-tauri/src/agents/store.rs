//! Agents on disk: `app_data/agents/<id>/agent.json` for each definition and
//! `app_data/agents/<id>/runs/` for its history — one `<runId>.json` record
//! per run and a `<runId>.log.jsonl` with its events.
//!
//! A folder per agent rather than one list file (unlike layouts.json): runs
//! are written while other agents are being edited, and deleting an agent
//! takes its whole history with it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;

use super::events::RunEvent;
use super::spec::{is_valid_id, AgentSpec};

/// Older runs than this many are pruned when a new one is recorded.
const KEEP_RUNS: usize = 50;

pub fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Running,
    Success,
    Error,
    Stopped,
    TimedOut,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunTrigger {
    Manual,
    Test,
    Schedule,
    Watch,
    Git,
    Webhook,
    /// Another agent it follows finished (`triggers.after`).
    Chain,
    /// Another agent asked it something and waits for the answer.
    Agent,
    /// Another agent left it a message.
    Message,
}

/// Where a run sits in a conversation between agents: the run that set it
/// off and the agents above it. A run with no link starts its own
/// conversation, named after its own id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RunLink {
    pub conversation_id: String,
    pub parent_run_id: String,
    pub parent_agent_id: String,
    /// The agents from the conversation's first down to the parent.
    pub path: Vec<String>,
    /// Part of a rehearsal (see projects::rehearsal_spec).
    pub rehearsal: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub id: String,
    pub agent_id: String,
    pub trigger: RunTrigger,
    pub status: RunStatus,
    /// Unix milliseconds.
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// The final answer, or why it failed.
    #[serde(default)]
    pub summary: String,
    /// What it's doing right now, while running.
    #[serde(default)]
    pub activity: String,
    pub cost_usd: Option<f64>,
    /// Set when another agent's run started this one.
    #[serde(default)]
    pub link: Option<RunLink>,
    /// A rehearsal: it could only look.
    #[serde(default)]
    pub rehearsal: bool,
}

impl RunRecord {
    /// The conversation it belongs to: its parent's, or its own.
    pub fn conversation_id(&self) -> &str {
        self.link.as_ref().map_or(&self.id, |link| &link.conversation_id)
    }
}

pub fn root(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let dir = app.path().app_data_dir()?.join("agents");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn agent_dir(root: &Path, id: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(is_valid_id(id), "invalid agent id");
    Ok(root.join(id))
}

fn write_atomic(path: &Path, text: &str) -> anyhow::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Every agent, by name. A folder without an agent.json (a wizard that ran a
/// test and was then cancelled) isn't an agent.
pub fn list_in(root: &Path) -> Vec<AgentSpec> {
    let mut agents: Vec<AgentSpec> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.path().join("agent.json")).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    agents.sort_by_key(|agent| agent.name.to_lowercase());
    agents
}

pub fn save_in(root: &Path, mut spec: AgentSpec) -> anyhow::Result<Vec<AgentSpec>> {
    let dir = agent_dir(root, &spec.id)?;
    fs::create_dir_all(&dir)?;
    let now = now_seconds();
    if spec.created_at == 0 {
        spec.created_at = now;
    }
    spec.updated_at = now;
    write_atomic(&dir.join("agent.json"), &serde_json::to_string_pretty(&spec)?)?;
    Ok(list_in(root))
}

pub fn get_in(root: &Path, id: &str) -> Option<AgentSpec> {
    let dir = agent_dir(root, id).ok()?;
    serde_json::from_str(&fs::read_to_string(dir.join("agent.json")).ok()?).ok()
}

/// Takes the agent's history with it.
pub fn delete_in(root: &Path, id: &str) -> anyhow::Result<Vec<AgentSpec>> {
    let dir = agent_dir(root, id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    Ok(list_in(root))
}

fn runs_dir(root: &Path, agent_id: &str) -> anyhow::Result<PathBuf> {
    let dir = agent_dir(root, agent_id)?.join("runs");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn save_run_in(root: &Path, run: &RunRecord) -> anyhow::Result<()> {
    let dir = runs_dir(root, &run.agent_id)?;
    write_atomic(&dir.join(format!("{}.json", run.id)), &serde_json::to_string(run)?)
}

/// Newest first.
pub fn list_runs_in(root: &Path, agent_id: &str) -> Vec<RunRecord> {
    let Ok(dir) = runs_dir(root, agent_id) else {
        return Vec::new();
    };
    let mut runs: Vec<RunRecord> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".json"))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at));
    runs
}

pub fn get_run_in(root: &Path, agent_id: &str, run_id: &str) -> Option<RunRecord> {
    if !is_valid_id(run_id) {
        return None;
    }
    let dir = runs_dir(root, agent_id).ok()?;
    serde_json::from_str(&fs::read_to_string(dir.join(format!("{run_id}.json"))).ok()?).ok()
}

/// Every run of a conversation between agents, across all of them, oldest
/// first.
pub fn conversation_in(root: &Path, conversation_id: &str) -> Vec<RunRecord> {
    let mut runs: Vec<RunRecord> = list_in(root)
        .iter()
        .flat_map(|spec| list_runs_in(root, &spec.id))
        .filter(|run| run.conversation_id() == conversation_id)
        .collect();
    runs.sort_by_key(|run| run.started_at);
    runs
}

/// Drops the history past the newest `KEEP_RUNS`.
pub fn prune_runs_in(root: &Path, agent_id: &str) {
    let Ok(dir) = runs_dir(root, agent_id) else {
        return;
    };
    for run in list_runs_in(root, agent_id).into_iter().skip(KEEP_RUNS) {
        let _ = fs::remove_file(dir.join(format!("{}.json", run.id)));
        let _ = fs::remove_file(dir.join(format!("{}.log.jsonl", run.id)));
    }
}

pub fn append_log_in(root: &Path, agent_id: &str, run_id: &str, event: &RunEvent) -> anyhow::Result<()> {
    let dir = runs_dir(root, agent_id)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{run_id}.log.jsonl")))?;
    writeln!(file, "{}", serde_json::to_string(event)?)?;
    Ok(())
}

pub fn read_log_in(root: &Path, agent_id: &str, run_id: &str) -> Vec<RunEvent> {
    if !is_valid_id(run_id) {
        return Vec::new();
    }
    let Ok(dir) = runs_dir(root, agent_id) else {
        return Vec::new();
    };
    fs::read_to_string(dir.join(format!("{run_id}.log.jsonl")))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::EventKind;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-agents-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn agent(id: &str, name: &str) -> AgentSpec {
        AgentSpec {
            id: id.into(),
            name: name.into(),
            ..AgentSpec::default()
        }
    }

    fn run(agent_id: &str, id: &str, started_at: i64) -> RunRecord {
        RunRecord {
            id: id.into(),
            agent_id: agent_id.into(),
            trigger: RunTrigger::Manual,
            status: RunStatus::Success,
            started_at,
            ended_at: None,
            summary: String::new(),
            activity: String::new(),
            cost_usd: None,
            link: None,
            rehearsal: false,
        }
    }

    #[test]
    fn saving_lists_by_name_and_stamps_times() {
        let root = temp_root("save");
        save_in(&root, agent("b", "zeta")).unwrap();
        let list = save_in(&root, agent("a", "Alfa")).unwrap();
        assert_eq!(list.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["Alfa", "zeta"]);
        assert!(list[0].created_at > 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resaving_keeps_the_creation_time() {
        let root = temp_root("resave");
        let mut first = save_in(&root, agent("a", "A")).unwrap().remove(0);
        first.created_at = 42;
        let again = save_in(&root, first).unwrap().remove(0);
        assert_eq!(again.created_at, 42);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deleting_takes_the_history_and_a_bad_id_is_refused() {
        let root = temp_root("delete");
        save_in(&root, agent("a", "A")).unwrap();
        save_run_in(&root, &run("a", "r1", 1)).unwrap();
        assert!(delete_in(&root, "a").unwrap().is_empty());
        assert!(!root.join("a").exists());
        assert!(save_in(&root, agent("../evil", "x")).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_folder_with_only_runs_is_not_an_agent() {
        let root = temp_root("orphan");
        save_run_in(&root, &run("tmp", "r1", 1)).unwrap();
        assert!(list_in(&root).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runs_come_newest_first_and_old_ones_are_pruned() {
        let root = temp_root("runs");
        for i in 0..(KEEP_RUNS as i64 + 3) {
            save_run_in(&root, &run("a", &format!("r{i}"), i)).unwrap();
        }
        prune_runs_in(&root, "a");
        let runs = list_runs_in(&root, "a");
        assert_eq!(runs.len(), KEEP_RUNS);
        assert_eq!(runs[0].started_at, KEEP_RUNS as i64 + 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_conversation_gathers_runs_across_agents() {
        let root = temp_root("conversation");
        save_in(&root, agent("a", "A")).unwrap();
        save_in(&root, agent("b", "B")).unwrap();
        save_run_in(&root, &run("a", "r1", 1)).unwrap();
        let mut child = run("b", "r2", 2);
        child.link = Some(RunLink {
            conversation_id: "r1".into(),
            parent_run_id: "r1".into(),
            parent_agent_id: "a".into(),
            path: vec!["a".into()],
            rehearsal: false,
        });
        save_run_in(&root, &child).unwrap();
        save_run_in(&root, &run("b", "other", 3)).unwrap();
        let ids: Vec<String> = conversation_in(&root, "r1").into_iter().map(|run| run.id).collect();
        assert_eq!(ids, ["r1", "r2"]);
        assert_eq!(get_run_in(&root, "b", "r2").unwrap().link.unwrap().parent_agent_id, "a");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn the_log_reads_back_in_order() {
        let root = temp_root("log");
        for text in ["uno", "dos"] {
            let event = RunEvent {
                kind: EventKind::Text,
                tool: String::new(),
                text: text.into(),
                at: 0,
            };
            append_log_in(&root, "a", "r1", &event).unwrap();
        }
        let log = read_log_in(&root, "a", "r1");
        assert_eq!(log.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(), ["uno", "dos"]);
        let _ = fs::remove_dir_all(root);
    }
}
