//! Requests to run an agent that arrive from outside the app — a git hook
//! calling `termo --run-agent` — left as small files for the scheduler to
//! pick up. The hook's call writes one and exits at once, so git never
//! waits on an agent.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::spec::is_valid_id;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub agent_id: String,
    pub context: String,
}

/// The folder, found without the app running: Termo's data folder, the
/// same one Tauri's `app_data_dir` names.
pub fn dir_outside_app() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("com.ln.termo").join("agent-triggers"))
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

/// The waiting requests, merged by agent (several commits during one run
/// make one run afterwards), taken off the queue — except for the agents
/// `keep` says not to start yet, whose requests stay for a later look.
pub fn take(dir: &Path, keep: impl Fn(&str) -> bool) -> HashMap<String, Vec<String>> {
    let mut merged: HashMap<String, Vec<String>> = HashMap::new();
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
        merged.entry(request.agent_id).or_default().push(request.context);
    }
    merged
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
        Request { agent_id: id.into(), context: context.into() }
    }

    #[test]
    fn requests_are_merged_by_agent_and_taken() {
        let dir = temp("merge");
        push(&dir, &request("a", "c1")).unwrap();
        push(&dir, &request("a", "c2")).unwrap();
        push(&dir, &request("b", "x")).unwrap();
        let taken = take(&dir, |_| false);
        assert_eq!(taken["a"].len(), 2);
        assert_eq!(taken["b"], ["x"]);
        assert!(take(&dir, |_| false).is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_busy_agents_requests_wait() {
        let dir = temp("busy");
        push(&dir, &request("a", "c1")).unwrap();
        assert!(take(&dir, |id| id == "a").is_empty());
        assert_eq!(take(&dir, |_| false)["a"], ["c1"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn bad_ids_never_become_files() {
        let dir = temp("bad");
        push(&dir, &request("../x", "c")).unwrap();
        assert!(!dir.exists() || fs::read_dir(&dir).unwrap().next().is_none());
    }
}
