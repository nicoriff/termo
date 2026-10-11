//! Copilot CLI's sessions: `~/.copilot/session-state/<id>/events.jsonl`, one
//! event a line, each `{ type, data, timestamp }`, with the session's name in
//! `workspace.yaml` beside it. Of its many events:
//!
//! - `session.start`: the folder it runs in.
//! - `user.message`: a prompt.
//! - `session.usage_record`: one call to a model, with its tokens under
//!   `usage`. Versions before it wrote `model.model_call_success` instead,
//!   with the tokens by kind under
//!   `responseChunk.copilot_usage.token_details`; a session with the former
//!   is read from those alone. Either includes the small calls Copilot makes
//!   on its own, to name the session.
//! - `tool.execution_start` and `tool.execution_complete`: a tool, and what
//!   it returned.
//!
//! Copilot bills by request, not by token, so none of this has a cost.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::{excerpt, jsonl, millis, modified_millis, SessionFile, Size};
use crate::companion::{Coverage, Request, SessionUsage, Tokens, Turn};

const TOOL: &str = "copilot";
const PROMPT_EXCERPT: usize = 400;
const EVENTS: &str = "events.jsonl";

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type", default)]
    kind: String,
    timestamp: Option<String>,
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    // session.start
    context: Option<Context>,
    // user.message
    content: Option<Value>,
    // model.model_call_success
    #[serde(rename = "modelCall")]
    model_call: Option<ModelCall>,
    #[serde(rename = "responseChunk")]
    response: Option<Response>,
    // tool.execution_complete
    result: Option<Size>,
    // session.usage_record
    usage: Option<UsageRecord>,
}

/// Cached tokens are counted inside `inputTokens`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageRecord {
    model: Option<String>,
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_tokens: u64,
    #[serde(default)]
    cache_write_tokens: u64,
}

#[derive(Deserialize)]
struct Context {
    cwd: Option<String>,
}

#[derive(Deserialize)]
struct ModelCall {
    model: Option<String>,
}

#[derive(Deserialize)]
struct Response {
    copilot_usage: Option<CopilotUsage>,
}

#[derive(Deserialize)]
struct CopilotUsage {
    #[serde(default)]
    token_details: Vec<TokenDetail>,
}

#[derive(Deserialize)]
struct TokenDetail {
    #[serde(default)]
    token_type: String,
    #[serde(default)]
    token_count: u64,
}

fn sessions_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(crate::agents::spec::copilot_home(&home).join("session-state"))
}

/// The sessions changed since `since` (Unix milliseconds).
pub fn list(since: i64) -> Vec<SessionFile> {
    sessions_dir().map(|dir| list_in(&dir, since)).unwrap_or_default()
}

fn list_in(dir: &Path, since: i64) -> Vec<SessionFile> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path().join(EVENTS);
            let meta = path.metadata().ok()?;
            let modified_at = modified_millis(&meta);
            if modified_at < since {
                return None;
            }
            let session_id = entry.file_name().to_str()?.to_string();
            Some(SessionFile { tool: TOOL.into(), session_id, modified_at, bytes: meta.len(), path })
        })
        .collect()
}

pub fn find(session_id: &str) -> Option<PathBuf> {
    // The id names a folder: nothing in it may walk out of the one it's in.
    if session_id.is_empty() || !session_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let path = sessions_dir()?.join(session_id).join(EVENTS);
    path.is_file().then_some(path)
}

/// How much of a session's name is kept: an unnamed one is named after its
/// whole first prompt.
const TITLE_EXCERPT: usize = 80;

/// The `name:` of a session's `workspace.yaml`, which is one `key: value` a
/// line, or `key: |-` with the value on the indented lines after it.
fn name_in(workspace: &Path) -> Option<String> {
    let text = std::fs::read_to_string(workspace).ok()?;
    let mut lines = text.lines();
    let inline = lines.by_ref().find_map(|line| line.strip_prefix("name:"))?.trim();
    let name = if inline.starts_with(['|', '>']) {
        lines.take_while(|line| line.starts_with(' ') || line.is_empty()).map(str::trim).find(|line| !line.is_empty())?
    } else {
        inline.trim_matches(['"', '\''])
    };
    (!name.is_empty()).then(|| excerpt(name, TITLE_EXCERPT))
}

pub fn read(path: &Path) -> Option<SessionUsage> {
    let folder = path.parent()?;
    let mut session = SessionUsage {
        tool: TOOL.into(),
        session_id: folder.file_name()?.to_str()?.to_string(),
        title: name_in(&folder.join("workspace.yaml")),
        coverage: Coverage { tokens: true, cache: true, prompts: true, tools: true, subagents: false, priced: false },
        ..Default::default()
    };
    // Which of the two records of a call's tokens this session has: read
    // once through for that, since counting both would count twice.
    let has_records = jsonl::<Line>(path)?.any(|line| line.kind == "session.usage_record");
    for line in jsonl::<Line>(path)? {
        let at = line.timestamp.as_deref().map(millis).unwrap_or(0);
        if at > 0 {
            if session.started_at == 0 {
                session.started_at = at;
            }
            session.ended_at = session.ended_at.max(at);
        }
        let Some(data) = line.data else { continue };
        match line.kind.as_str() {
            "session.start" => session.cwd = data.context.and_then(|context| context.cwd).or(session.cwd.take()),
            "user.message" => {
                let text = data.content.as_ref().and_then(Value::as_str).unwrap_or_default();
                session.turns.push(Turn {
                    at,
                    prompt_chars: text.trim().chars().count() as u32,
                    prompt: excerpt(text, PROMPT_EXCERPT),
                    ..Default::default()
                });
            }
            "session.usage_record" => {
                let Some(usage) = data.usage else { continue };
                let Some(model) = usage.model else { continue };
                let tokens = Tokens {
                    input: usage.input_tokens.saturating_sub(usage.cache_read_tokens + usage.cache_write_tokens),
                    cache_write: usage.cache_write_tokens,
                    cache_read: usage.cache_read_tokens,
                    output: usage.output_tokens,
                };
                request(&mut session, at, model, tokens);
            }
            "model.model_call_success" if !has_records => {
                let (Some(model), Some(usage)) = (data.model_call.and_then(|call| call.model), data.response.and_then(|response| response.copilot_usage)) else {
                    continue;
                };
                let mut tokens = Tokens::default();
                for detail in &usage.token_details {
                    match detail.token_type.as_str() {
                        "input" => tokens.input += detail.token_count,
                        "cache_read" => tokens.cache_read += detail.token_count,
                        "cache_write" => tokens.cache_write += detail.token_count,
                        "output" => tokens.output += detail.token_count,
                        _ => {}
                    }
                }
                request(&mut session, at, model, tokens);
            }
            "tool.execution_start" => {
                if let Some(turn) = session.turns.last_mut() {
                    turn.tool_calls += 1;
                }
            }
            "tool.execution_complete" => {
                if let (Some(turn), Some(Size(chars))) = (session.turns.last_mut(), data.result) {
                    turn.largest_result_chars = turn.largest_result_chars.max(chars);
                }
            }
            _ => {}
        }
    }
    session.finish();
    Some(session)
}

fn request(session: &mut SessionUsage, at: i64, model: String, tokens: Tokens) {
    if session.turns.is_empty() {
        session.turns.push(Turn { at, automatic: true, ..Default::default() });
    }
    let turn = session.turns.last_mut().expect("just made sure of one");
    turn.requests.push(Request { at, model, tokens, ..Default::default() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn details(input: u64, read: u64, output: u64) -> Value {
        json!({ "copilot_usage": { "token_details": [
            { "token_type": "input", "token_count": input },
            { "token_type": "cache_read", "token_count": read },
            { "token_type": "cache_write", "token_count": 0 },
            { "token_type": "output", "token_count": output },
        ] } })
    }

    #[test]
    fn a_sessions_events_read_into_its_turns() {
        let dir = std::env::temp_dir().join(format!("termo-companion-{}", uuid::Uuid::new_v4()));
        let folder = dir.join("60714312-cb3b-4b45-bfd9-47b43fd86708");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("workspace.yaml"), "id: 60714312\ncwd: E:\\work\nname: Identify Programming Language\nuser_named: false\n").unwrap();
        let lines = [
            json!({ "type": "session.start", "timestamp": "2026-10-01T11:54:20Z", "data": { "context": { "cwd": "E:\\work" } } }),
            json!({ "type": "user.message", "timestamp": "2026-10-01T11:55:07Z", "data": { "content": "what language is this?", "turnId": "0" } }),
            // The call that names the session, on a model of its own.
            json!({ "type": "model.model_call_success", "timestamp": "2026-10-01T11:55:08Z",
                "data": { "modelCall": { "model": "gpt-4o-mini" }, "responseChunk": details(155, 0, 11) } }),
            json!({ "type": "tool.execution_start", "timestamp": "2026-10-01T11:55:11Z", "data": { "toolName": "view" } }),
            json!({ "type": "tool.execution_complete", "timestamp": "2026-10-01T11:55:11Z",
                "data": { "success": true, "result": { "content": "0123456789", "detailedContent": "01234" } } }),
            json!({ "type": "model.model_call_success", "timestamp": "2026-10-01T11:55:12Z",
                "data": { "modelCall": { "model": "mai-code-1.1-flash" }, "responseChunk": details(9_000, 6_000, 300) } }),
            json!({ "type": "session.usage_checkpoint", "timestamp": "2026-10-01T11:55:13Z", "data": { "totalPremiumRequests": 1 } }),
        ];
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        let path = folder.join(EVENTS);
        std::fs::write(&path, text.join("\n")).unwrap();
        let found = list_in(&dir, 0);
        let session = read(&path).unwrap();
        let _ = std::fs::remove_dir_all(dir);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].session_id, "60714312-cb3b-4b45-bfd9-47b43fd86708");
        assert_eq!(session.title.as_deref(), Some("Identify Programming Language"));
        assert_eq!(session.cwd.as_deref(), Some("E:\\work"));
        assert_eq!(session.turns.len(), 1);
        let turn = &session.turns[0];
        assert_eq!((turn.prompt.as_str(), turn.prompt_chars), ("what language is this?", 22));
        assert_eq!((turn.request_count, turn.tool_calls, turn.largest_result_chars), (2, 1, 15));
        assert_eq!(turn.model.as_deref(), Some("mai-code-1.1-flash"));
        assert_eq!(turn.tokens, Tokens { input: 9_155, cache_write: 0, cache_read: 6_000, output: 311 });
        // Billed by request: no figure in dollars, even for a model with a price.
        assert_eq!((turn.cost_usd, session.cost_usd), (None, None));
        assert!(find("../outside").is_none());
    }

    #[test]
    fn a_newer_session_is_read_from_its_usage_records() {
        let dir = std::env::temp_dir().join(format!("termo-companion-{}", uuid::Uuid::new_v4()));
        let folder = dir.join("b589d5d3");
        std::fs::create_dir_all(&folder).unwrap();
        // Unnamed: Copilot names it after the whole prompt, as a block.
        std::fs::write(folder.join("workspace.yaml"), "id: b589d5d3\nname: |-\n  Plan the work for this project\n\n  In detail.\nuser_named: false\n").unwrap();
        let lines = [
            json!({ "type": "user.message", "timestamp": "2026-10-10T02:51:42Z", "data": { "content": "Plan the work for this project" } }),
            json!({ "type": "session.usage_record", "timestamp": "2026-10-10T02:51:46Z", "data": { "usage": {
                "model": "gpt-6-luna", "inputTokens": 15_623, "outputTokens": 101, "cacheReadTokens": 0, "cacheWriteTokens": 15_620, "cost": 1 } } }),
            // Were both kinds there, only the records would count.
            json!({ "type": "model.model_call_success", "timestamp": "2026-10-10T02:51:46Z",
                "data": { "modelCall": { "model": "gpt-6-luna" }, "responseChunk": details(3, 0, 101) } }),
        ];
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        let path = folder.join(EVENTS);
        std::fs::write(&path, text.join("\n")).unwrap();
        let session = read(&path).unwrap();
        let _ = std::fs::remove_dir_all(dir);

        assert_eq!(session.title.as_deref(), Some("Plan the work for this project"));
        assert_eq!(session.turns[0].request_count, 1);
        assert_eq!(session.tokens, Tokens { input: 3, cache_write: 15_620, cache_read: 0, output: 101 });
    }
}
