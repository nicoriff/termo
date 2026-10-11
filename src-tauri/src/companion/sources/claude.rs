//! Claude Code's sessions: `~/.claude/projects/<folder>/<session>.jsonl`,
//! one JSON object a line, appended to as the session goes. What matters
//! here, among much else it logs:
//!
//! - `user` lines: a prompt (text) or the results of the tools the model
//!   called (`tool_result` blocks).
//! - `assistant` lines: one per block of a response, each repeating the whole
//!   response's `usage`, so a response's tokens are counted once, by its
//!   message id.
//! - `system` / `compact_boundary`: the conversation was summarized.
//! - `ai-title` and `cost-state`: the session's name and Claude Code's own
//!   cost figure, the latter only once it has written one.
//!
//! Subagents write their own files, under `<session>/subagents/`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::{excerpt, jsonl, millis, modified_millis, SessionFile, Size};
use crate::companion::{pricing, Compaction, Coverage, Request, SessionUsage, Tokens, Turn};

const TOOL: &str = "claude";

/// How much of a prompt is kept to show.
const PROMPT_EXCERPT: usize = 400;

/// What Claude Code answers itself with when the API fails: not a model.
const SYNTHETIC_MODEL: &str = "<synthetic>";

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type", default)]
    kind: String,
    subtype: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "isSidechain", default)]
    sidechain: bool,
    #[serde(rename = "isMeta", default)]
    meta: bool,
    #[serde(rename = "isCompactSummary", default)]
    compact_summary: bool,
    cwd: Option<String>,
    effort: Option<Value>,
    origin: Option<Value>,
    message: Option<Message>,
    #[serde(rename = "compactMetadata")]
    compact: Option<CompactMetadata>,
    #[serde(rename = "aiTitle")]
    title: Option<String>,
    #[serde(rename = "totalCostUSD")]
    total_cost: Option<f64>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
    content: Option<Content>,
}

#[derive(Deserialize, Default)]
struct Usage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    cache_creation: Option<CacheCreation>,
}

#[derive(Deserialize)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type", default)]
    kind: String,
    text: Option<String>,
    name: Option<String>,
    input: Option<ToolInput>,
    /// A tool result's content, of which only the size matters.
    content: Option<Size>,
}

#[derive(Deserialize)]
struct ToolInput {
    file_path: Option<String>,
    offset: Option<Value>,
    limit: Option<Value>,
}

#[derive(Deserialize)]
struct CompactMetadata {
    trigger: Option<String>,
    #[serde(rename = "preTokens", default)]
    before: u64,
    #[serde(rename = "postTokens", default)]
    after: u64,
}

fn projects_dir() -> Option<PathBuf> {
    crate::agents::models::claude_dir().map(|dir| dir.join("projects"))
}

/// The sessions changed since `since` (Unix milliseconds), newest first.
pub fn list(since: i64) -> Vec<SessionFile> {
    projects_dir().map(|dir| list_in(&dir, since)).unwrap_or_default()
}

fn list_in(projects: &Path, since: i64) -> Vec<SessionFile> {
    let mut found = Vec::new();
    for project in std::fs::read_dir(projects).into_iter().flatten().flatten() {
        for entry in std::fs::read_dir(project.path()).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            let (Ok(meta), Some(session_id)) = (entry.metadata(), path.file_stem().and_then(|stem| stem.to_str())) else {
                continue;
            };
            let modified_at = modified_millis(&meta);
            if modified_at < since {
                continue;
            }
            found.push(SessionFile { tool: TOOL.into(), session_id: session_id.to_string(), modified_at, bytes: meta.len(), path });
        }
    }
    found.sort_by_key(|file| std::cmp::Reverse(file.modified_at));
    found
}

/// The file of one session, wherever its project's folder is.
pub fn find(session_id: &str) -> Option<PathBuf> {
    // The id names a file: nothing in it may walk out of the folder.
    if session_id.is_empty() || !session_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let name = format!("{session_id}.jsonl");
    std::fs::read_dir(projects_dir()?)
        .ok()?
        .flatten()
        .map(|project| project.path().join(&name))
        .find(|path| path.is_file())
}

/// Reads a session and its subagents'. None when the file can't be opened.
pub fn read(path: &Path) -> Option<SessionUsage> {
    let mut session = SessionUsage {
        tool: TOOL.into(),
        session_id: path.file_stem()?.to_str()?.to_string(),
        coverage: Coverage { tokens: true, cache: true, prompts: true, tools: true, subagents: true, priced: true },
        ..Default::default()
    };
    let mut subagents = Subagents::default();
    let mut seen = HashSet::new();
    for line in jsonl::<Line>(path)? {
        take(&mut session, &mut subagents, &mut seen, line);
    }
    // Next to the session's file, in a folder of its name.
    let own = path.with_extension("").join("subagents");
    for entry in std::fs::read_dir(own).into_iter().flatten().flatten() {
        let mut seen = HashSet::new();
        for line in jsonl::<Line>(&entry.path()).into_iter().flatten() {
            if let Some(request) = request_of(&line, &mut seen) {
                subagents.add(&request);
            }
        }
    }
    session.subagent_tokens = subagents.tokens;
    session.subagent_cost_usd = if subagents.unpriced { None } else { Some(subagents.cost) };
    session.finish();
    Some(session)
}

#[derive(Default)]
struct Subagents {
    tokens: Tokens,
    cost: f64,
    unpriced: bool,
}

impl Subagents {
    fn add(&mut self, request: &Request) {
        self.tokens.add(&request.tokens);
        match pricing::cost(request) {
            Some(cost) => self.cost += cost,
            None => self.unpriced = true,
        }
    }
}

/// The request an `assistant` line stands for, the first time its message
/// shows up.
fn request_of(line: &Line, seen: &mut HashSet<String>) -> Option<Request> {
    if line.kind != "assistant" {
        return None;
    }
    let message = line.message.as_ref()?;
    let model = message.model.as_deref().filter(|model| *model != SYNTHETIC_MODEL)?;
    let usage = message.usage.as_ref()?;
    if let Some(id) = &message.id {
        if !seen.insert(id.clone()) {
            return None;
        }
    }
    Some(Request {
        at: line.timestamp.as_deref().map(millis).unwrap_or(0),
        model: model.to_string(),
        tokens: Tokens {
            input: usage.input_tokens,
            cache_write: usage.cache_creation_input_tokens,
            cache_read: usage.cache_read_input_tokens,
            output: usage.output_tokens,
        },
        cache_write_1h: usage.cache_creation.as_ref().map(|cache| cache.ephemeral_1h_input_tokens).unwrap_or(0),
        reported_cost_usd: None,
    })
}

/// What a `user` line is, of the things Claude Code logs as one.
enum Said {
    /// Typed by the user.
    Prompt(String),
    /// Put there by Claude Code and answered by the model all the same: a
    /// background task reporting back.
    Automatic,
    /// Results of the tools the model called, with the largest one's size.
    Results(u64),
    /// Nothing the model answers: a local command and its output.
    Nothing,
}

fn said(line: &Line) -> Said {
    let Some(content) = line.message.as_ref().and_then(|message| message.content.as_ref()) else {
        return Said::Nothing;
    };
    let text = match content {
        Content::Text(text) => text.clone(),
        Content::Blocks(blocks) => {
            if blocks.iter().any(|block| block.kind == "tool_result") {
                let largest = blocks.iter().filter_map(|block| block.content.as_ref()).map(|size| size.0).max();
                return Said::Results(largest.unwrap_or(0));
            }
            let texts: Vec<&str> = blocks.iter().filter(|block| block.kind == "text").filter_map(|block| block.text.as_deref()).collect();
            texts.join("\n")
        }
    };
    if line.meta || line.compact_summary {
        return Said::Nothing;
    }
    let text = text.trim();
    let origin = line.origin.as_ref().and_then(|origin| origin.get("kind")).and_then(Value::as_str);
    if origin.is_some_and(|kind| kind != "human") || text.starts_with("<task-notification>") {
        return Said::Automatic;
    }
    if text.is_empty() || text.starts_with("<local-command-") || text.starts_with("<command-name>") {
        return Said::Nothing;
    }
    Said::Prompt(text.to_string())
}

fn take(session: &mut SessionUsage, subagents: &mut Subagents, seen: &mut HashSet<String>, line: Line) {
    let at = line.timestamp.as_deref().map(millis).unwrap_or(0);
    if at > 0 {
        if session.started_at == 0 {
            session.started_at = at;
        }
        session.ended_at = session.ended_at.max(at);
    }
    if session.cwd.is_none() {
        session.cwd = line.cwd.clone();
    }
    match line.kind.as_str() {
        "ai-title" => session.title = line.title.or(session.title.take()),
        "cost-state" => session.reported_cost_usd = line.total_cost.or(session.reported_cost_usd),
        "system" if line.subtype.as_deref() == Some("compact_boundary") => {
            if let Some(compact) = line.compact {
                session.compactions.push(Compaction {
                    at,
                    automatic: compact.trigger.as_deref() == Some("auto"),
                    tokens_before: compact.before,
                    tokens_after: compact.after,
                });
            }
        }
        // Older versions logged subagents in the session's own file.
        "assistant" if line.sidechain => {
            if let Some(request) = request_of(&line, seen) {
                subagents.add(&request);
            }
        }
        "assistant" => {
            if session.turns.is_empty() {
                // A resumed session picks up without a prompt of its own.
                session.turns.push(Turn { at, automatic: true, ..Default::default() });
            }
            let request = request_of(&line, seen);
            let turn = session.turns.last_mut().expect("just made sure of one");
            if let Some(effort) = line.effort.as_ref().and_then(Value::as_str) {
                turn.effort = Some(effort.to_string());
            }
            if let Some(Content::Blocks(blocks)) = line.message.as_ref().and_then(|message| message.content.as_ref()) {
                for block in blocks.iter().filter(|block| block.kind == "tool_use") {
                    turn.tool_calls += 1;
                    let whole_file = block.input.as_ref().filter(|input| input.offset.is_none() && input.limit.is_none());
                    if let (Some("Read"), Some(path)) = (block.name.as_deref(), whole_file.and_then(|input| input.file_path.clone())) {
                        turn.files_read.push(path);
                    }
                }
            }
            turn.requests.extend(request);
        }
        "user" if !line.sidechain => match said(&line) {
            Said::Prompt(text) => session.turns.push(Turn {
                at,
                prompt_chars: text.chars().count() as u32,
                prompt: excerpt(&text, PROMPT_EXCERPT),
                ..Default::default()
            }),
            Said::Automatic => session.turns.push(Turn { at, automatic: true, ..Default::default() }),
            Said::Results(largest) => {
                if let Some(turn) = session.turns.last_mut() {
                    turn.largest_result_chars = turn.largest_result_chars.max(largest);
                }
            }
            Said::Nothing => {}
        },
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assistant(id: &str, at: &str, model: &str, usage: Value, blocks: Value) -> Value {
        json!({
            "type": "assistant", "timestamp": at, "effort": "high", "cwd": "/work",
            "message": { "id": id, "model": model, "usage": usage, "content": blocks }
        })
    }

    fn usage(input: u64, write: u64, read: u64, output: u64) -> Value {
        json!({
            "input_tokens": input, "cache_creation_input_tokens": write, "cache_read_input_tokens": read,
            "output_tokens": output, "cache_creation": { "ephemeral_1h_input_tokens": write, "ephemeral_5m_input_tokens": 0 }
        })
    }

    fn session_file(lines: &[Value]) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("termo-companion-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("abc-123.jsonl");
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(&path, text.join("\n")).unwrap();
        (dir, path)
    }

    #[test]
    fn a_session_reads_into_its_turns() {
        let (dir, path) = session_file(&[
            json!({ "type": "user", "timestamp": "2026-10-01T10:00:00Z", "cwd": "/work", "origin": { "kind": "human" },
                "message": { "role": "user", "content": "fix the login bug" } }),
            // One response logged as two blocks: its tokens count once.
            assistant("m1", "2026-10-01T10:00:05Z", "claude-opus-5-5", usage(10, 2000, 0, 50), json!([{ "type": "thinking", "thinking": "" }])),
            assistant("m1", "2026-10-01T10:00:05Z", "claude-opus-5-5", usage(10, 2000, 0, 50),
                json!([{ "type": "tool_use", "name": "Read", "input": { "file_path": "/work/login.rs" } }])),
            json!({ "type": "user", "timestamp": "2026-10-01T10:00:06Z",
                "message": { "role": "user", "content": [{ "type": "tool_result", "content": [{ "type": "text", "text": "0123456789" }] }] } }),
            assistant("m2", "2026-10-01T10:00:09Z", "claude-opus-5-5", usage(5, 100, 2000, 30),
                json!([{ "type": "tool_use", "name": "Read", "input": { "file_path": "/work/big.rs", "offset": 10, "limit": 20 } }])),
            json!({ "type": "ai-title", "aiTitle": "Fix login" }),
            json!({ "type": "system", "subtype": "compact_boundary", "timestamp": "2026-10-01T10:05:00Z",
                "compactMetadata": { "trigger": "auto", "preTokens": 900000, "postTokens": 20000 } }),
            json!({ "type": "user", "timestamp": "2026-10-01T10:06:00Z", "isCompactSummary": true,
                "message": { "role": "user", "content": "summary of the conversation so far" } }),
            json!({ "type": "user", "timestamp": "2026-10-01T10:07:00Z", "origin": { "kind": "task-notification" },
                "message": { "role": "user", "content": "<task-notification>done</task-notification>" } }),
            assistant("m3", "2026-10-01T10:07:02Z", "claude-haiku-5-5", usage(1, 0, 0, 1), json!([{ "type": "text", "text": "ok" }])),
            // An API error Claude Code answered itself: not a request.
            assistant("m4", "2026-10-01T10:07:03Z", "<synthetic>", usage(0, 0, 0, 0), json!([{ "type": "text", "text": "API Error" }])),
            json!({ "type": "user", "timestamp": "2026-10-01T10:08:00Z",
                "message": { "role": "user", "content": "<local-command-stdout>Enabled plan mode</local-command-stdout>" } }),
            json!({ "type": "cost-state", "totalCostUSD": 1.5 }),
            json!("not an object at all"),
        ]);
        let session = read(&path).unwrap();
        let _ = std::fs::remove_dir_all(dir);

        assert_eq!((session.tool.as_str(), session.session_id.as_str()), ("claude", "abc-123"));
        assert_eq!(session.cwd.as_deref(), Some("/work"));
        assert_eq!(session.title.as_deref(), Some("Fix login"));
        assert_eq!(session.reported_cost_usd, Some(1.5));
        assert_eq!(session.turns.len(), 2);

        let first = &session.turns[0];
        assert_eq!((first.prompt.as_str(), first.prompt_chars, first.automatic), ("fix the login bug", 17, false));
        assert_eq!((first.request_count, first.tool_calls), (2, 2));
        assert_eq!(first.tokens, Tokens { input: 15, cache_write: 2100, cache_read: 2000, output: 80 });
        assert_eq!(first.peak_context, 2105);
        assert_eq!(first.largest_result_chars, 10);
        assert_eq!(first.effort.as_deref(), Some("high"));
        // Only the file read in full counts as read.
        assert_eq!(first.files_read, ["/work/login.rs"]);
        assert_eq!(first.requests[0].cache_write_1h, 2000);

        let second = &session.turns[1];
        assert!(second.automatic && second.prompt.is_empty());
        assert_eq!(second.model.as_deref(), Some("claude-haiku-5-5"));

        assert_eq!(session.compactions, [Compaction { at: millis("2026-10-01T10:05:00Z"), automatic: true, tokens_before: 900000, tokens_after: 20000 }]);
        assert_eq!(session.started_at, millis("2026-10-01T10:00:00Z"));
        assert_eq!(session.ended_at, millis("2026-10-01T10:08:00Z"));
        assert!(session.cost_usd.is_some());
    }

    #[test]
    fn subagents_are_counted_from_their_own_files() {
        let (dir, path) = session_file(&[
            json!({ "type": "user", "timestamp": "2026-10-01T10:00:00Z", "message": { "role": "user", "content": "look around" } }),
            assistant("m1", "2026-10-01T10:00:05Z", "claude-opus-5-5", usage(100, 0, 0, 10), json!([])),
        ]);
        let subagents = dir.join("abc-123").join("subagents");
        std::fs::create_dir_all(&subagents).unwrap();
        let mut line = assistant("s1", "2026-10-01T10:00:06Z", "claude-haiku-5-5", usage(1_000_000, 0, 0, 0), json!([]));
        line["isSidechain"] = json!(true);
        std::fs::write(subagents.join("agent-a.jsonl"), format!("{line}\n{line}")).unwrap();
        let session = read(&path).unwrap();
        let _ = std::fs::remove_dir_all(dir);

        assert_eq!(session.subagent_tokens.input, 1_000_000);
        assert_eq!(session.subagent_cost_usd, Some(0.1));
        assert_eq!(session.tokens.input, 1_000_100);
        assert_eq!(session.turns.len(), 1);
    }

    #[test]
    fn sessions_are_listed_newest_first_and_found_by_id() {
        let projects = std::env::temp_dir().join(format!("termo-companion-{}", uuid::Uuid::new_v4()));
        let project = projects.join("C--work");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("one.jsonl"), "{}").unwrap();
        std::fs::write(project.join("notes.txt"), "").unwrap();
        let found = list_in(&projects, 0);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].session_id.as_str(), found[0].bytes), ("one", 2));
        assert!(list_in(&projects, found[0].modified_at + 1).is_empty());
        let _ = std::fs::remove_dir_all(projects);

        assert!(find("../../etc/passwd").is_none());
        assert!(find("").is_none());
    }
}
