//! A session in brief, for looking at many at once: what it cost, on which
//! models, and the habits found in it.
//!
//! Reading a session means going through its whole file, and a look at a
//! month is hundreds of them. So each summary is kept, in
//! `<app data>/companion/summaries.json`, beside the size and date of the
//! file it was made from: only the sessions that changed since are read
//! again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::rules::{self, FindingKind, Severity};
use super::sources::{self, SessionFile};
use super::{SessionUsage, Tokens};

/// Bumped when what a summary holds, or how it's worked out, changes: the
/// ones kept under another number are made again.
const VERSION: u32 = 1;

const FILE: &str = "summaries.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelShare {
    pub model: String,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FindingBrief {
    pub kind: FindingKind,
    pub severity: Severity,
    pub wasted_usd: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub tool: String,
    pub session_id: String,
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    /// The prompts the user typed.
    pub turns: u32,
    pub tokens: Tokens,
    /// The agent's own figure when it wrote one down, else the estimate.
    pub cost_usd: Option<f64>,
    pub cost_reported: bool,
    /// Billed by token at all (see `Coverage::priced`).
    pub priced: bool,
    pub models: Vec<ModelShare>,
    pub findings: Vec<FindingBrief>,
}

pub fn summarize(session: &SessionUsage) -> SessionSummary {
    let mut models: Vec<ModelShare> = Vec::new();
    for request in session.turns.iter().flat_map(|turn| &turn.requests) {
        let priced = if session.coverage.priced { super::pricing::cost(request) } else { None };
        let cost = request.reported_cost_usd.or(priced);
        match models.iter_mut().find(|share| share.model == request.model) {
            Some(share) => {
                share.tokens += request.tokens.total();
                share.cost_usd = share.cost_usd.zip(cost).map(|(sum, cost)| sum + cost);
            }
            None => models.push(ModelShare { model: request.model.clone(), tokens: request.tokens.total(), cost_usd: cost }),
        }
    }
    models.sort_by_key(|share| std::cmp::Reverse(share.tokens));
    SessionSummary {
        tool: session.tool.clone(),
        session_id: session.session_id.clone(),
        cwd: session.cwd.clone(),
        title: session.title.clone(),
        started_at: session.started_at,
        ended_at: session.ended_at,
        turns: session.turns.iter().filter(|turn| !turn.automatic).count() as u32,
        tokens: session.tokens,
        cost_usd: session.reported_cost_usd.or(session.cost_usd),
        cost_reported: session.reported_cost_usd.is_some(),
        priced: session.coverage.priced,
        models,
        findings: rules::findings(session)
            .into_iter()
            .map(|finding| FindingBrief { kind: finding.kind, severity: finding.severity, wasted_usd: finding.wasted_usd })
            .collect(),
    }
}

/// A summary and the state of the file it was made from.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Kept {
    modified_at: i64,
    bytes: u64,
    summary: SessionSummary,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    version: u32,
    #[serde(default)]
    sessions: HashMap<String, Kept>,
}

fn load(file: &Path) -> Store {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<Store>(&text).ok())
        .filter(|store| store.version == VERSION)
        .unwrap_or_default()
}

fn save(file: &Path, store: &Store) {
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Whole or not at all: written beside it, then moved into place.
    let partial = file.with_extension("json.tmp");
    if let Ok(text) = serde_json::to_string(store) {
        if std::fs::write(&partial, text).is_ok() {
            let _ = std::fs::rename(&partial, file);
        }
    }
}

/// Where the summaries are kept, under the app's data folder.
pub fn store_file(app_data: &Path) -> PathBuf {
    app_data.join("companion").join(FILE)
}

/// Every session changed since `since` (Unix milliseconds), in brief, newest
/// first.
pub fn overview(store_file: &Path, since: i64) -> Vec<SessionSummary> {
    overview_of(store_file, sources::list(since), |file| sources::read(&file.tool, &file.session_id))
}

fn overview_of(store_file: &Path, files: Vec<SessionFile>, read: impl Fn(&SessionFile) -> Option<SessionUsage>) -> Vec<SessionSummary> {
    let mut store = load(store_file);
    store.version = VERSION;
    let mut changed = false;
    let mut found = Vec::new();
    for file in &files {
        let key = format!("{}:{}", file.tool, file.session_id);
        let kept = store.sessions.get(&key).filter(|kept| kept.modified_at == file.modified_at && kept.bytes == file.bytes);
        let summary = match kept {
            Some(kept) => kept.summary.clone(),
            None => {
                let Some(session) = read(file) else { continue };
                let summary = summarize(&session);
                store.sessions.insert(key, Kept { modified_at: file.modified_at, bytes: file.bytes, summary: summary.clone() });
                changed = true;
                summary
            }
        };
        // A session opened and never used isn't worth a row.
        if summary.tokens.total() > 0 {
            found.push(summary);
        }
    }
    if changed {
        save(store_file, &store);
    }
    found.sort_by_key(|summary| std::cmp::Reverse(summary.ended_at));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::companion::{Coverage, Request, Turn};
    use std::cell::Cell;

    fn session(id: &str, input: u64) -> SessionUsage {
        let request = |model: &str, input| Request { model: model.into(), tokens: Tokens { input, ..Default::default() }, ..Default::default() };
        let mut session = SessionUsage {
            tool: "claude".into(),
            session_id: id.into(),
            ended_at: input as i64,
            coverage: Coverage { tokens: true, cache: true, prompts: true, tools: true, subagents: true, priced: true },
            turns: vec![
                Turn { prompt_chars: 600, requests: vec![request("claude-opus-5-5", input), request("claude-haiku-5-5", 10)], ..Default::default() },
                Turn { automatic: true, ..Default::default() },
            ],
            ..Default::default()
        };
        session.finish();
        session
    }

    fn file(id: &str, modified_at: i64) -> SessionFile {
        SessionFile { tool: "claude".into(), session_id: id.into(), path: PathBuf::new(), modified_at, bytes: 100 }
    }

    #[test]
    fn a_summary_says_what_a_session_cost_and_on_which_models() {
        let summary = summarize(&session("a", 1_000_000));
        assert_eq!((summary.turns, summary.cost_reported), (1, false));
        assert_eq!(summary.models.len(), 2);
        assert_eq!((summary.models[0].model.as_str(), summary.models[0].tokens, summary.models[0].cost_usd), ("claude-opus-5-5", 1_000_000, Some(4.0)));
        assert!((summary.cost_usd.unwrap() - 4.000001).abs() < 1e-9);
        // 65,000 tokens and more in a first request is a heavy start.
        assert_eq!(summary.findings.len(), 1);
        assert_eq!(summary.findings[0].kind, FindingKind::HeavyBaseline);
    }

    #[test]
    fn only_sessions_that_changed_are_read_again() {
        let dir = std::env::temp_dir().join(format!("termo-companion-{}", uuid::Uuid::new_v4()));
        let store = store_file(&dir);
        let reads = Cell::new(0);
        let read = |file: &SessionFile| {
            reads.set(reads.get() + 1);
            // One that can't be read is left out, and tried again next time.
            match file.session_id.as_str() {
                "broken" => None,
                "empty" => Some(SessionUsage { tool: "claude".into(), session_id: "empty".into(), ..Default::default() }),
                id => Some(session(id, 5_000 + file.modified_at as u64)),
            }
        };

        let first = overview_of(&store, vec![file("a", 1), file("b", 2), file("broken", 3), file("empty", 4)], read);
        // Newest first, and the one nothing was asked of left out.
        assert_eq!(first.iter().map(|summary| summary.session_id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(reads.get(), 4);

        // `b` was worked in again; `a` and `empty` are as they were.
        let second = overview_of(&store, vec![file("a", 1), file("b", 9), file("broken", 3), file("empty", 4)], read);
        assert_eq!(reads.get(), 6);
        assert_eq!(second[0].tokens.input, 5_019);

        // Summaries kept by another version of this are made again.
        std::fs::write(&store, r#"{"version":0,"sessions":{}}"#).unwrap();
        overview_of(&store, vec![file("a", 1)], read);
        assert_eq!(reads.get(), 7);
        let _ = std::fs::remove_dir_all(dir);
    }
}
