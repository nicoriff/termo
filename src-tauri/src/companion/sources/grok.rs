//! Grok's sessions: `~/.grok/sessions/<folder, URL-encoded>/<id>/`, a folder
//! of files each. Two of them are read:
//!
//! - `summary.json`: where the session ran (`info.cwd`) and its title
//!   (`session_summary`).
//! - `updates.jsonl`: what the session sent its client, one update a line,
//!   each `{ timestamp, params: { update } }` with `timestamp` in seconds.
//!   A `user_message_chunk` is (a piece of) a prompt, a `tool_call` a tool
//!   called and a `tool_call_update` what it returned, and a
//!   `turn_completed` closes the turn with its `usage`: the tokens of every
//!   model call in it added up, how many calls that was, and what Grok says
//!   it cost.
//!
//! So a turn's tokens come as one sum rather than call by call. To keep what
//! is said of a turn's context honest, the sum is spread evenly over its
//! calls. Grok counts cached tokens inside `inputTokens`; here they're apart.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{excerpt, jsonl, modified_millis, SessionFile, Size};
use crate::companion::{Coverage, Request, SessionUsage, Tokens, Turn};

const TOOL: &str = "grok";
const PROMPT_EXCERPT: usize = super::PROMPT_KEPT;
const UPDATES: &str = "updates.jsonl";

/// Grok writes costs in ten-billionths of a dollar.
const TICKS_PER_USD: f64 = 1e10;

#[derive(Deserialize)]
struct Line {
    /// Unix seconds.
    #[serde(default)]
    timestamp: i64,
    params: Option<Params>,
}

#[derive(Deserialize)]
struct Params {
    update: Option<Update>,
}

#[derive(Deserialize)]
struct Update {
    #[serde(rename = "sessionUpdate", default)]
    kind: String,
    /// A prompt's text in a `user_message_chunk`, a tool's result in a
    /// `tool_call_update`.
    content: Option<serde_json::Value>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cached_read_tokens: u64,
    #[serde(default)]
    cache_creation_tokens: u64,
    #[serde(default)]
    model_calls: u64,
    cost_usd_ticks: Option<f64>,
    #[serde(default)]
    model_usage: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct Summary {
    info: Option<Info>,
    session_summary: Option<String>,
}

#[derive(Deserialize)]
struct Info {
    cwd: Option<String>,
}

fn sessions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".grok").join("sessions"))
}

/// Every session's folder: `<sessions>/<folder>/<id>/`.
fn folders(sessions: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for project in std::fs::read_dir(sessions).into_iter().flatten().flatten() {
        for entry in std::fs::read_dir(project.path()).into_iter().flatten().flatten() {
            if entry.path().join(UPDATES).is_file() {
                found.push(entry.path());
            }
        }
    }
    found
}

/// The sessions changed since `since` (Unix milliseconds).
pub fn list(since: i64) -> Vec<SessionFile> {
    sessions_dir().map(|dir| list_in(&dir, since)).unwrap_or_default()
}

fn list_in(sessions: &Path, since: i64) -> Vec<SessionFile> {
    folders(sessions)
        .into_iter()
        .filter_map(|folder| {
            let path = folder.join(UPDATES);
            let meta = path.metadata().ok()?;
            let modified_at = modified_millis(&meta);
            if modified_at < since {
                return None;
            }
            let session_id = folder.file_name()?.to_str()?.to_string();
            Some(SessionFile { tool: TOOL.into(), session_id, modified_at, bytes: meta.len(), path })
        })
        .collect()
}

pub fn find(session_id: &str) -> Option<PathBuf> {
    if session_id.is_empty() {
        return None;
    }
    let folder = folders(&sessions_dir()?).into_iter().find(|folder| folder.file_name().is_some_and(|name| name == session_id))?;
    Some(folder.join(UPDATES))
}

/// The characters of text in a JSON value already read.
fn chars_in(value: &serde_json::Value) -> u64 {
    serde_json::from_value::<Size>(value.clone()).map(|size| size.0).unwrap_or(0)
}

pub fn read(path: &Path) -> Option<SessionUsage> {
    let folder = path.parent()?;
    let summary: Option<Summary> = std::fs::read_to_string(folder.join("summary.json")).ok().and_then(|text| serde_json::from_str(&text).ok());
    let mut session = SessionUsage {
        tool: TOOL.into(),
        session_id: folder.file_name()?.to_str()?.to_string(),
        cwd: summary.as_ref().and_then(|summary| summary.info.as_ref()).and_then(|info| info.cwd.clone()),
        title: summary.and_then(|summary| summary.session_summary).filter(|title| !title.trim().is_empty()),
        coverage: Coverage { tokens: true, cache: true, prompts: true, tools: true, subagents: false, priced: true },
        ..Default::default()
    };
    let mut reported = None;
    // A prompt arrives in pieces; the turn it opens stays open, taking the
    // rest of them, until something else is said.
    let mut typing = false;
    let mut prompt = String::new();
    for line in jsonl::<Line>(path)? {
        let at = line.timestamp * 1000;
        if at > 0 {
            if session.started_at == 0 {
                session.started_at = at;
            }
            session.ended_at = session.ended_at.max(at);
        }
        let Some(update) = line.params.and_then(|params| params.update) else { continue };
        if update.kind == "user_message_chunk" {
            let piece = update.content.as_ref().and_then(|content| content.get("text")).and_then(|text| text.as_str()).unwrap_or("");
            if !typing {
                typing = true;
                prompt.clear();
                session.turns.push(Turn { at, ..Default::default() });
            }
            prompt.push_str(piece);
            let turn = session.turns.last_mut().expect("just pushed, or still typing");
            turn.prompt_chars = prompt.trim().chars().count() as u32;
            turn.prompt = excerpt(&prompt, PROMPT_EXCERPT);
            continue;
        }
        // A hook running between two pieces of a prompt doesn't end it.
        if update.kind != "hook_execution" {
            typing = false;
        }
        match update.kind.as_str() {
            "tool_call" => {
                if let Some(turn) = session.turns.last_mut() {
                    turn.tool_calls += 1;
                }
            }
            "tool_call_update" => {
                if let (Some(turn), Some(content)) = (session.turns.last_mut(), &update.content) {
                    turn.largest_result_chars = turn.largest_result_chars.max(chars_in(content));
                }
            }
            "turn_completed" => {
                let Some(usage) = update.usage else { continue };
                if session.turns.is_empty() {
                    session.turns.push(Turn { at, automatic: true, ..Default::default() });
                }
                let turn = session.turns.last_mut().expect("just made sure of one");
                let cost = usage.cost_usd_ticks.map(|ticks| ticks / TICKS_PER_USD);
                reported = match (reported, cost) {
                    (sum, Some(cost)) => Some(sum.unwrap_or(0.0) + cost),
                    (sum, None) => sum,
                };
                turn.requests.extend(spread(at, &usage, cost));
            }
            _ => {}
        }
    }
    session.reported_cost_usd = reported;
    session.finish();
    Some(session)
}

/// A turn's summed usage as one request per model call, each an equal share.
fn spread(at: i64, usage: &Usage, cost: Option<f64>) -> Vec<Request> {
    let calls = usage.model_calls.max(1);
    // The model that did the turn: the one it used most, when it says.
    let model = usage
        .model_usage
        .iter()
        .max_by_key(|(_, of_model)| of_model.get("totalTokens").and_then(|total| total.as_u64()).unwrap_or(0))
        .map(|(model, _)| model.clone())
        .unwrap_or_default();
    let cached = usage.cached_read_tokens;
    let written = usage.cache_creation_tokens;
    let whole = Tokens {
        input: usage.input_tokens.saturating_sub(cached + written),
        cache_write: written,
        cache_read: cached,
        output: usage.output_tokens,
    };
    (0..calls)
        .map(|call| {
            // What doesn't divide evenly goes to the first call.
            let share = |total: u64| total / calls + if call == 0 { total % calls } else { 0 };
            Request {
                at,
                model: model.clone(),
                tokens: Tokens {
                    input: share(whole.input),
                    cache_write: share(whole.cache_write),
                    cache_read: share(whole.cache_read),
                    output: share(whole.output),
                },
                cache_write_1h: 0,
                reported_cost_usd: cost.map(|cost| cost / calls as f64),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn update(timestamp: i64, update: Value) -> Value {
        json!({ "timestamp": timestamp, "method": "session/update", "params": { "sessionId": "s", "update": update } })
    }

    #[test]
    fn a_sessions_updates_read_into_its_turns() {
        let sessions = std::env::temp_dir().join(format!("termo-companion-{}", uuid::Uuid::new_v4()));
        let folder = sessions.join("C%3A%5Cwork").join("01a123b3-b632-7370-96b0-de075dc1f275");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("summary.json"), json!({ "info": { "id": "01a123b3", "cwd": "C:\\work" }, "session_summary": "Add a notes command" }).to_string()).unwrap();
        let lines = [
            update(1_791_600_343, json!({ "sessionUpdate": "hook_execution", "event_name": "session_start" })),
            update(1_791_600_343, json!({ "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "add a command " } })),
            update(1_791_600_343, json!({ "sessionUpdate": "hook_execution", "event_name": "user_prompt_submit" })),
            update(1_791_600_343, json!({ "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "to list notes" } })),
            update(1_791_600_345, json!({ "sessionUpdate": "agent_thought_chunk", "content": { "type": "text", "text": "thinking" } })),
            update(1_791_600_346, json!({ "sessionUpdate": "tool_call", "toolCallId": "c1", "title": "list_dir" })),
            update(1_791_600_347, json!({ "sessionUpdate": "tool_call_update", "toolCallId": "c1", "status": "completed",
                "content": [{ "type": "content", "content": { "type": "text", "text": "0123456789" } }] })),
            update(1_791_600_351, json!({ "sessionUpdate": "turn_completed", "stop_reason": "end_turn", "usage": {
                "inputTokens": 43_036, "outputTokens": 271, "totalTokens": 43_307, "cachedReadTokens": 22_400, "cacheCreationTokens": 0,
                "reasoningTokens": 91, "modelCalls": 2, "costUsdTicks": 540_980_000u64,
                "modelUsage": { "grok-4.7": { "totalTokens": 43_307 } } } })),
            update(1_791_600_400, json!({ "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "thanks" } })),
        ];
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        let path = folder.join(UPDATES);
        std::fs::write(&path, text.join("\n")).unwrap();
        let found = list_in(&sessions, 0);
        let session = read(&path).unwrap();
        let _ = std::fs::remove_dir_all(sessions);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].session_id, "01a123b3-b632-7370-96b0-de075dc1f275");
        assert_eq!((session.cwd.as_deref(), session.title.as_deref()), (Some("C:\\work"), Some("Add a notes command")));
        assert_eq!(session.turns.len(), 2);
        let turn = &session.turns[0];
        // Its two pieces, with a hook between them, are one prompt.
        assert_eq!((turn.prompt.as_str(), turn.prompt_chars), ("add a command to list notes", 27));
        assert_eq!((turn.request_count, turn.tool_calls, turn.largest_result_chars), (2, 1, 10));
        assert_eq!(turn.model.as_deref(), Some("grok-4.7"));
        assert_eq!(turn.tokens, Tokens { input: 20_636, cache_write: 0, cache_read: 22_400, output: 271 });
        // The context of one call, not of the two added up.
        assert_eq!(turn.peak_context, 21_518);
        // What Grok itself says the turn cost.
        assert!((turn.cost_usd.unwrap() - 0.054098).abs() < 1e-9);
        assert!((session.reported_cost_usd.unwrap() - 0.054098).abs() < 1e-9);
        assert_eq!(session.turns[1].prompt, "thanks");
        assert_eq!(session.started_at, 1_791_600_343_000);
    }
}
