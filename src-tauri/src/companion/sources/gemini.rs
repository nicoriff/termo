//! Gemini CLI's sessions: `~/.gemini/tmp/<project>/chats/session-<time>-<id>.jsonl`,
//! where `<id>` is the start of the session's id and the project's folder is
//! written in `<project>/.project_root`. A header line, then:
//!
//! - `{ "$set": { ... } }`: a field of the session rewritten — its `summary`
//!   (the title), or the messages it started with, which are Gemini CLI's own
//!   setup and no prompt of the user's.
//! - a message: `type` "user" (a prompt) or "gemini" (a response, with its
//!   `tokens`, `model` and `toolCalls`). A message is written again, under
//!   the same id, when it changes: the last one stands.
//!
//! Gemini counts cached tokens inside `input`; here they're apart. Thinking
//! is billed as output.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{excerpt, jsonl, millis, modified_millis, SessionFile, Size};
use crate::companion::{Coverage, Request, SessionUsage, Tokens, Turn};

const TOOL: &str = "gemini";
const PROMPT_EXCERPT: usize = super::PROMPT_KEPT;

#[derive(Deserialize)]
struct Line {
    // The header.
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "startTime")]
    start_time: Option<String>,
    #[serde(rename = "$set")]
    set: Option<Set>,
    // A message.
    id: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "type", default)]
    kind: String,
    content: Option<Content>,
    tokens: Option<Usage>,
    model: Option<String>,
    #[serde(rename = "toolCalls", default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Deserialize)]
struct Set {
    summary: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Parts(Vec<Part>),
}

#[derive(Deserialize)]
struct Part {
    text: Option<String>,
}

impl Content {
    fn text(&self) -> String {
        match self {
            Content::Text(text) => text.clone(),
            Content::Parts(parts) => parts.iter().filter_map(|part| part.text.as_deref()).collect::<Vec<_>>().join("\n"),
        }
    }
}

#[derive(Deserialize)]
struct Usage {
    #[serde(default)]
    input: u64,
    #[serde(default)]
    output: u64,
    #[serde(default)]
    cached: u64,
    #[serde(default)]
    thoughts: u64,
}

#[derive(Deserialize)]
struct ToolCall {
    result: Option<Size>,
}

fn projects_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".gemini").join("tmp"))
}

/// The id a chat's header gives it; its file name only has the start of it.
fn header_id(path: &Path) -> Option<String> {
    jsonl::<Line>(path)?.next()?.session_id
}

/// The sessions changed since `since` (Unix milliseconds).
pub fn list(since: i64) -> Vec<SessionFile> {
    projects_dir().map(|dir| list_in(&dir, since)).unwrap_or_default()
}

fn chats(projects: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for project in std::fs::read_dir(projects).into_iter().flatten().flatten() {
        for entry in std::fs::read_dir(project.path().join("chats")).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                found.push(path);
            }
        }
    }
    found
}

fn list_in(projects: &Path, since: i64) -> Vec<SessionFile> {
    chats(projects)
        .into_iter()
        .filter_map(|path| {
            let meta = path.metadata().ok()?;
            let modified_at = modified_millis(&meta);
            if modified_at < since {
                return None;
            }
            Some(SessionFile { tool: TOOL.into(), session_id: header_id(&path)?, modified_at, bytes: meta.len(), path })
        })
        .collect()
}

pub fn find(session_id: &str) -> Option<PathBuf> {
    find_in(&projects_dir()?, session_id)
}

fn find_in(projects: &Path, session_id: &str) -> Option<PathBuf> {
    // The name ends in the id's first eight characters: enough to pick the
    // few files worth opening.
    let short = session_id.get(..8)?;
    chats(projects)
        .into_iter()
        .filter(|path| path.file_stem().and_then(|stem| stem.to_str()).is_some_and(|stem| stem.ends_with(short)))
        .find(|path| header_id(path).as_deref() == Some(session_id))
}

pub fn read(path: &Path) -> Option<SessionUsage> {
    let mut session = SessionUsage {
        tool: TOOL.into(),
        coverage: Coverage { tokens: true, cache: true, prompts: true, tools: true, subagents: false, priced: true },
        ..Default::default()
    };
    // `<project>/chats/<file>`: the project's folder is written beside `chats`.
    let project_root = path.parent().and_then(Path::parent).map(|project| project.join(".project_root"));
    session.cwd = project_root.and_then(|file| std::fs::read_to_string(file).ok()).map(|cwd| cwd.trim().to_string()).filter(|cwd| !cwd.is_empty());

    // Messages in the order they first appeared, each as it was last written.
    let mut order: Vec<String> = Vec::new();
    let mut latest: HashMap<String, Line> = HashMap::new();
    for line in jsonl::<Line>(path)? {
        if let Some(id) = line.session_id.clone() {
            session.session_id = id;
            session.started_at = line.start_time.as_deref().map(millis).unwrap_or(0);
        }
        if let Some(summary) = line.set.as_ref().and_then(|set| set.summary.clone()) {
            session.title = Some(summary);
        }
        let Some(id) = line.id.clone() else { continue };
        if !latest.contains_key(&id) {
            order.push(id.clone());
        }
        latest.insert(id, line);
    }

    for line in order.iter().filter_map(|id| latest.get(id)) {
        let at = line.timestamp.as_deref().map(millis).unwrap_or(0);
        if at > 0 {
            if session.started_at == 0 {
                session.started_at = at;
            }
            session.ended_at = session.ended_at.max(at);
        }
        match line.kind.as_str() {
            "user" => {
                let text = line.content.as_ref().map(Content::text).unwrap_or_default();
                session.turns.push(Turn {
                    at,
                    prompt_chars: text.trim().chars().count() as u32,
                    prompt: excerpt(&text, PROMPT_EXCERPT),
                    ..Default::default()
                });
            }
            "gemini" => {
                if session.turns.is_empty() {
                    session.turns.push(Turn { at, automatic: true, ..Default::default() });
                }
                let turn = session.turns.last_mut().expect("just made sure of one");
                turn.tool_calls += line.tool_calls.len() as u32;
                let largest = line.tool_calls.iter().filter_map(|call| call.result.as_ref()).map(|size| size.0).max().unwrap_or(0);
                turn.largest_result_chars = turn.largest_result_chars.max(largest);
                if let (Some(usage), Some(model)) = (&line.tokens, &line.model) {
                    turn.requests.push(Request {
                        at,
                        model: model.clone(),
                        tokens: Tokens {
                            input: usage.input.saturating_sub(usage.cached),
                            cache_write: 0,
                            cache_read: usage.cached,
                            output: usage.output + usage.thoughts,
                        },
                        ..Default::default()
                    });
                }
            }
            _ => {}
        }
    }
    session.finish();
    Some(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn chat(lines: &[Value]) -> (PathBuf, PathBuf) {
        let projects = std::env::temp_dir().join(format!("termo-companion-{}", uuid::Uuid::new_v4()));
        let project = projects.join("ormi");
        std::fs::create_dir_all(project.join("chats")).unwrap();
        std::fs::write(project.join(".project_root"), "e:\\proyectos\\ormi\n").unwrap();
        let path = project.join("chats").join("session-2026-09-30T23-44-853a7230.jsonl");
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(&path, text.join("\n")).unwrap();
        (projects, path)
    }

    #[test]
    fn a_chat_reads_into_its_turns() {
        let id = "853a7230-df75-4ea1-9b02-15a327a60162";
        let (projects, path) = chat(&[
            json!({ "sessionId": id, "startTime": "2026-09-30T23:44:50Z", "kind": "main" }),
            // What Gemini CLI starts every session with: not a prompt.
            json!({ "$set": { "messages": [{ "id": "setup", "type": "user", "content": [{ "text": "<session_context>" }] }] } }),
            json!({ "id": "u1", "timestamp": "2026-09-30T23:46:08Z", "type": "user", "content": [{ "text": "what is this app written in?" }] }),
            // Written first without its tokens, then again once it's whole.
            json!({ "id": "g1", "timestamp": "2026-09-30T23:46:40Z", "type": "gemini", "content": "" }),
            json!({ "id": "g1", "timestamp": "2026-09-30T23:46:49Z", "type": "gemini", "content": "",
                "tokens": { "input": 11_499, "output": 115, "cached": 4_000, "thoughts": 289, "tool": 0, "total": 11_903 },
                "model": "gemini-3.8-flash",
                "toolCalls": [{ "name": "read_file", "result": [{ "functionResponse": { "response": { "output": "0123456789" } } }] }] }),
            json!({ "$set": { "summary": "Identify the stack" } }),
            json!({ "$set": { "lastUpdated": "2026-09-30T23:47:00Z" } }),
        ]);
        let found = list_in(&projects, 0);
        let by_id = find_in(&projects, id);
        let session = read(&path).unwrap();
        let _ = std::fs::remove_dir_all(projects);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].session_id, id);
        assert_eq!(by_id.as_deref(), Some(path.as_path()));
        assert_eq!((session.session_id.as_str(), session.title.as_deref()), (id, Some("Identify the stack")));
        assert_eq!(session.cwd.as_deref(), Some("e:\\proyectos\\ormi"));
        assert_eq!(session.turns.len(), 1);
        let turn = &session.turns[0];
        assert_eq!(turn.prompt, "what is this app written in?");
        assert_eq!((turn.request_count, turn.tool_calls), (1, 1));
        assert_eq!(turn.model.as_deref(), Some("gemini-3.8-flash"));
        assert_eq!(turn.tokens, Tokens { input: 7_499, cache_write: 0, cache_read: 4_000, output: 404 });
        assert_eq!(turn.largest_result_chars, 10);
        assert_eq!(session.started_at, millis("2026-09-30T23:44:50Z"));
    }
}
