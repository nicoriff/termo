//! Codex's sessions: `$CODEX_HOME/sessions/<year>/<month>/<day>/rollout-<time>-<id>.jsonl`,
//! one event a line, each `{ timestamp, type, payload }`:
//!
//! - `session_meta`: the session's id and folder.
//! - `turn_context`: the model a turn runs on.
//! - `event_msg` / `item_completed` with a `UserMessage`: a prompt. Older
//!   versions wrote `event_msg` / `user_message` instead.
//! - `token_usage_record`: one response's tokens. Older versions only wrote
//!   `event_msg` / `token_count`, whose `last_token_usage` is the same
//!   figure, so that's what a rollout without the former is read from.
//! - `response_item` / `*_call` and `*_call_output`: a tool called, and what
//!   it returned.
//!
//! Codex counts cached tokens inside `input_tokens`; here they're apart.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::{excerpt, jsonl, millis, modified_millis, SessionFile, Size};
use crate::companion::{Compaction, Coverage, Request, SessionUsage, Tokens, Turn};
use crate::agents::models::codex_home;

const TOOL: &str = "codex";
const PROMPT_EXCERPT: usize = 400;

#[derive(Deserialize)]
struct Line {
    timestamp: Option<String>,
    #[serde(rename = "type", default)]
    kind: String,
    payload: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(rename = "type", default)]
    kind: String,
    // session_meta
    id: Option<String>,
    cwd: Option<String>,
    // turn_context
    model: Option<String>,
    effort: Option<Value>,
    // token_usage_record
    usage: Option<Usage>,
    // event_msg / token_count
    info: Option<TokenInfo>,
    // event_msg / item_completed
    item: Option<Item>,
    // event_msg / user_message
    message: Option<Value>,
    // response_item / *_call_output
    output: Option<Size>,
}

#[derive(Deserialize)]
struct TokenInfo {
    last_token_usage: Option<Usage>,
}

#[derive(Deserialize, Clone, Copy)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    cache_write_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

impl Usage {
    fn tokens(&self) -> Tokens {
        Tokens {
            input: self.input_tokens.saturating_sub(self.cached_input_tokens + self.cache_write_input_tokens),
            cache_write: self.cache_write_input_tokens,
            cache_read: self.cached_input_tokens,
            output: self.output_tokens,
        }
    }
}

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    content: Vec<Text>,
}

#[derive(Deserialize)]
struct Text {
    text: Option<String>,
}

fn sessions_dir() -> Option<PathBuf> {
    codex_home().map(|home| home.join("sessions"))
}

/// Every rollout under `dir`, however deep its date folders go.
fn rollouts(dir: &Path, depth: u8, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                rollouts(&path, depth - 1, found);
            }
        } else if path.extension().is_some_and(|ext| ext == "jsonl") {
            found.push(path);
        }
    }
}

/// The id a rollout's name ends in: `rollout-2026-09-20T20-48-59-<id>.jsonl`.
fn id_in(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let after_time = stem.strip_prefix("rollout-")?.get(20..)?;
    Some(after_time.to_string())
}

/// The sessions changed since `since` (Unix milliseconds).
pub fn list(since: i64) -> Vec<SessionFile> {
    sessions_dir().map(|dir| list_in(&dir, since)).unwrap_or_default()
}

fn list_in(dir: &Path, since: i64) -> Vec<SessionFile> {
    let mut paths = Vec::new();
    rollouts(dir, 3, &mut paths);
    paths
        .into_iter()
        .filter_map(|path| {
            let meta = path.metadata().ok()?;
            let modified_at = modified_millis(&meta);
            (modified_at >= since).then(|| SessionFile { tool: TOOL.into(), session_id: id_in(&path).unwrap_or_default(), modified_at, bytes: meta.len(), path })
        })
        .filter(|file| !file.session_id.is_empty())
        .collect()
}

pub fn find(session_id: &str) -> Option<PathBuf> {
    if session_id.is_empty() {
        return None;
    }
    let mut paths = Vec::new();
    rollouts(&sessions_dir()?, 3, &mut paths);
    paths.into_iter().find(|path| id_in(path).as_deref() == Some(session_id))
}

pub fn read(path: &Path) -> Option<SessionUsage> {
    let mut session = SessionUsage {
        tool: TOOL.into(),
        session_id: id_in(path).unwrap_or_default(),
        coverage: Coverage { tokens: true, cache: true, prompts: true, tools: true, subagents: false, priced: true },
        ..Default::default()
    };
    // Which of the two records of a response's tokens this rollout has: read
    // once through for that, since counting both would count twice.
    let has_records = jsonl::<Line>(path)?.any(|line| line.kind == "token_usage_record");
    let mut model = String::new();
    let mut effort = None;
    for line in jsonl::<Line>(path)? {
        let at = line.timestamp.as_deref().map(millis).unwrap_or(0);
        if at > 0 {
            if session.started_at == 0 {
                session.started_at = at;
            }
            session.ended_at = session.ended_at.max(at);
        }
        let Some(payload) = line.payload else { continue };
        match (line.kind.as_str(), payload.kind.as_str()) {
            ("session_meta", _) => {
                session.cwd = payload.cwd.or(session.cwd.take());
                if let Some(id) = payload.id {
                    session.session_id = id;
                }
            }
            ("turn_context", _) => {
                model = payload.model.unwrap_or(model);
                effort = payload.effort.as_ref().and_then(Value::as_str).map(str::to_string).or(effort);
            }
            ("event_msg", "item_completed") => {
                let Some(item) = payload.item.filter(|item| item.kind == "UserMessage") else { continue };
                let texts: Vec<&str> = item.content.iter().filter_map(|text| text.text.as_deref()).collect();
                session.turns.push(prompt(at, &texts.join("\n")));
            }
            ("event_msg", "user_message") => {
                if let Some(text) = payload.message.as_ref().and_then(Value::as_str) {
                    session.turns.push(prompt(at, text));
                }
            }
            ("event_msg", "context_compacted") => {
                session.compactions.push(Compaction { at, automatic: true, tokens_before: 0, tokens_after: 0 });
            }
            ("token_usage_record", _) => add(&mut session, at, &model, &effort, payload.usage),
            ("event_msg", "token_count") if !has_records => {
                add(&mut session, at, &model, &effort, payload.info.and_then(|info| info.last_token_usage));
            }
            ("response_item", kind) if kind.ends_with("_call") => {
                if let Some(turn) = session.turns.last_mut() {
                    turn.tool_calls += 1;
                }
            }
            ("response_item", kind) if kind.ends_with("_call_output") => {
                if let (Some(turn), Some(Size(chars))) = (session.turns.last_mut(), payload.output) {
                    turn.largest_result_chars = turn.largest_result_chars.max(chars);
                }
            }
            _ => {}
        }
    }
    session.finish();
    Some(session)
}

fn prompt(at: i64, text: &str) -> Turn {
    Turn { at, prompt_chars: text.trim().chars().count() as u32, prompt: excerpt(text, PROMPT_EXCERPT), ..Default::default() }
}

fn add(session: &mut SessionUsage, at: i64, model: &str, effort: &Option<String>, usage: Option<Usage>) {
    let Some(usage) = usage else { return };
    if session.turns.is_empty() {
        session.turns.push(Turn { at, automatic: true, ..Default::default() });
    }
    let turn = session.turns.last_mut().expect("just made sure of one");
    turn.effort = effort.clone();
    turn.requests.push(Request { at, model: model.to_string(), tokens: usage.tokens(), ..Default::default() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rollout(lines: &[Value]) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("termo-companion-{}", uuid::Uuid::new_v4()));
        let day = dir.join("2026").join("09").join("20");
        std::fs::create_dir_all(&day).unwrap();
        let path = day.join("rollout-2026-09-20T20-48-59-01a0c139-175f-7fc0-a449-a7ed7c3aaf3b.jsonl");
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(&path, text.join("\n")).unwrap();
        (dir, path)
    }

    fn usage(input: u64, cached: u64, output: u64) -> Value {
        json!({ "input_tokens": input, "cached_input_tokens": cached, "cache_write_input_tokens": 0, "output_tokens": output })
    }

    #[test]
    fn a_rollout_reads_into_its_turns() {
        let (dir, path) = rollout(&[
            json!({ "timestamp": "2026-09-20T23:49:49Z", "type": "session_meta", "payload": { "id": "01a0c139-175f-7fc0-a449-a7ed7c3aaf3b", "cwd": "/work" } }),
            json!({ "timestamp": "2026-09-20T23:49:49Z", "type": "turn_context", "payload": { "model": "gpt-5.6-terra", "effort": "medium" } }),
            // The context Codex sends along isn't something the user typed.
            json!({ "timestamp": "2026-09-20T23:49:49Z", "type": "response_item", "payload": { "type": "message", "role": "user" } }),
            json!({ "timestamp": "2026-09-20T23:49:50Z", "type": "event_msg",
                "payload": { "type": "item_completed", "item": { "type": "UserMessage", "content": [{ "type": "text", "text": "what does this app do" }] } } }),
            json!({ "timestamp": "2026-09-20T23:49:52Z", "type": "event_msg", "payload": { "type": "item_completed", "item": { "type": "Reasoning" } } }),
            json!({ "timestamp": "2026-09-20T23:49:57Z", "type": "response_item", "payload": { "type": "custom_tool_call", "name": "exec" } }),
            json!({ "timestamp": "2026-09-20T23:49:57Z", "type": "token_usage_record", "payload": { "usage": usage(13_852, 9_984, 166) } }),
            json!({ "timestamp": "2026-09-20T23:50:17Z", "type": "response_item",
                "payload": { "type": "custom_tool_call_output", "output": [{ "type": "input_text", "text": "0123456789" }] } }),
            // The same response's tokens, the old way: not counted again.
            json!({ "timestamp": "2026-09-20T23:50:17Z", "type": "event_msg", "payload": { "type": "token_count", "info": { "last_token_usage": usage(13_852, 9_984, 166) } } }),
            json!({ "timestamp": "2026-09-20T23:50:20Z", "type": "token_usage_record", "payload": { "usage": usage(14_500, 13_800, 400) } }),
        ]);
        let found = list_in(&dir, 0);
        let session = read(&path).unwrap();
        let _ = std::fs::remove_dir_all(dir);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].session_id, "01a0c139-175f-7fc0-a449-a7ed7c3aaf3b");
        assert_eq!((session.tool.as_str(), session.cwd.as_deref()), ("codex", Some("/work")));
        assert_eq!(session.turns.len(), 1);
        let turn = &session.turns[0];
        assert_eq!((turn.prompt.as_str(), turn.prompt_chars), ("what does this app do", 21));
        assert_eq!((turn.request_count, turn.tool_calls, turn.largest_result_chars), (2, 1, 10));
        assert_eq!(turn.model.as_deref(), Some("gpt-5.6-terra"));
        assert_eq!(turn.effort.as_deref(), Some("medium"));
        // Cached tokens come out of the input they were counted in.
        assert_eq!(turn.tokens, Tokens { input: 3_868 + 700, cache_write: 0, cache_read: 9_984 + 13_800, output: 566 });
        assert_eq!(turn.peak_context, 14_500);
    }

    #[test]
    fn an_older_rollout_is_read_from_its_token_counts() {
        let (dir, path) = rollout(&[
            json!({ "timestamp": "2026-09-20T23:49:49Z", "type": "turn_context", "payload": { "model": "gpt-5" } }),
            json!({ "timestamp": "2026-09-20T23:49:50Z", "type": "event_msg", "payload": { "type": "user_message", "message": "hello" } }),
            json!({ "timestamp": "2026-09-20T23:50:17Z", "type": "event_msg", "payload": { "type": "token_count", "info": { "last_token_usage": usage(1_000, 400, 50) } } }),
            json!({ "timestamp": "2026-09-20T23:50:18Z", "type": "event_msg", "payload": { "type": "token_count", "info": null } }),
        ]);
        let session = read(&path).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(session.turns.len(), 1);
        assert_eq!(session.turns[0].prompt, "hello");
        assert_eq!(session.tokens, Tokens { input: 600, cache_write: 0, cache_read: 400, output: 50 });
    }
}
