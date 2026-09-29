//! Requests to run an agent, left as small files for the scheduler to pick
//! up: from outside the app — a git hook calling `termo --run-agent`, which
//! writes one and exits at once, so git never waits on an agent — and from
//! other agents (a finished run its followers come after, a message), which
//! wait here while the agent they're for is busy.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::spec::is_valid_id;
use super::store::{RunLink, RunTrigger};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub agent_id: String,
    pub context: String,
    /// None: a git hook's (the only kind there was before).
    #[serde(default)]
    pub trigger: Option<RunTrigger>,
    #[serde(default)]
    pub link: Option<RunLink>,
}

impl Request {
    pub fn trigger(&self) -> RunTrigger {
        self.trigger.unwrap_or(RunTrigger::Git)
    }
}

/// The folder, found without the app running: Termo's data folder, the
/// same one Tauri's `app_data_dir` names.
pub fn dir_outside_app() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("com.ln.termo").join("agent-triggers"))
}

/// The same folder, from the agents' folder the app knows (`store::root`).
pub fn dir_for(agents_root: &Path) -> PathBuf {
    agents_root.parent().map(|data| data.join("agent-triggers")).unwrap_or_default()
}

pub fn push(dir: &Path, request: &Request) -> std::io::Result<()> {
    if !is_valid_id(&request.agent_id) {
        return Ok(());
    }
    fs::create_dir_all(dir)?;
    let name = format!("{}-{}.json", request.agent_id, uuid::Uuid::new_v4());
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, serde_json::to_string(request).unwrap_or_default())?;
    fs::rename(tmp, dir.join(name))
}

/// The waiting requests, gathered by agent (several commits during one run
/// make one run afterwards), taken off the queue — except for the agents
/// `keep` says not to start yet, whose requests stay for a later look.
pub fn take(dir: &Path, keep: impl Fn(&str) -> bool) -> HashMap<String, Vec<Request>> {
    let mut merged: HashMap<String, Vec<Request>> = HashMap::new();
    let Ok(entries) = fs::read_dir(dir) else { return merged };
    let mut files: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    for path in files {
        let Some(request) = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Request>(&text).ok())
        else {
            let _ = fs::remove_file(&path);
            continue;
        };
        if keep(&request.agent_id) {
            continue;
        }
        let _ = fs::remove_file(&path);
        merged.entry(request.agent_id.clone()).or_default().push(request);
    }
    merged
}

/// What one run makes of an agent's gathered requests: the first one's
/// trigger, every context, and the deepest link — so a run standing for
/// several conversations counts against the strictest of their limits.
pub fn merge(requests: &[Request]) -> (RunTrigger, String, Option<RunLink>) {
    let trigger = requests.first().map_or(RunTrigger::Git, Request::trigger);
    let context = requests
        .iter()
        .map(|request| request.context.trim())
        .filter(|context| !context.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let link = requests
        .iter()
        .filter_map(|request| request.link.clone())
        .max_by_key(|link| link.path.len());
    (trigger, context, link)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-queue-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn request(id: &str, context: &str) -> Request {
        Request { agent_id: id.into(), context: context.into(), trigger: None, link: None }
    }

    #[test]
    fn requests_are_merged_by_agent_and_taken() {
        let dir = temp("merge");
        push(&dir, &request("a", "c1")).unwrap();
        push(&dir, &request("a", "c2")).unwrap();
        push(&dir, &request("b", "x")).unwrap();
        let taken = take(&dir, |_| false);
        assert_eq!(taken["a"].len(), 2);
        assert_eq!(taken["b"], [request("b", "x")]);
        assert!(take(&dir, |_| false).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_hooks_old_request_still_reads_as_git() {
        let old: Request = serde_json::from_str(r#"{"agentId":"a","context":"c"}"#).unwrap();
        assert_eq!(old.trigger(), RunTrigger::Git);
        assert!(old.link.is_none());
    }

    #[test]
    fn merging_keeps_the_first_trigger_and_the_deepest_link() {
        let link = |depth: usize| RunLink { path: vec!["x".into(); depth], ..RunLink::default() };
        let requests = [
            Request { trigger: Some(RunTrigger::Message), link: Some(link(1)), ..request("a", "uno") },
            Request { trigger: Some(RunTrigger::Chain), link: Some(link(3)), ..request("a", "  ") },
            Request { trigger: Some(RunTrigger::Message), link: None, ..request("a", "dos") },
        ];
        let (trigger, context, deepest) = merge(&requests);
        assert_eq!(trigger, RunTrigger::Message);
        assert_eq!(context, "uno\n\ndos");
        assert_eq!(deepest.unwrap().path.len(), 3);
    }

    #[test]
    fn a_busy_agents_requests_wait() {
        let dir = temp("busy");
        push(&dir, &request("a", "c1")).unwrap();
        assert!(take(&dir, |id| id == "a").is_empty());
        assert_eq!(take(&dir, |_| false)["a"], [request("a", "c1")]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn bad_ids_never_become_files() {
        let dir = temp("bad");
        push(&dir, &request("../x", "c")).unwrap();
        assert!(!dir.exists() || fs::read_dir(&dir).unwrap().next().is_none());
    }
}
