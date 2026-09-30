//! Turning each CLI's machine-readable output into one shape of event:
//! Claude's `--output-format stream-json`, Codex's `exec --json` and Gemini
//! CLI's `--output-format stream-json` all print one JSON object per line, in
//! different vocabularies.

use serde::{Deserialize, Serialize};

use super::spec::Engine;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    Started,
    /// The agent used a tool: `tool` is its name, `text` what it did it on.
    Tool,
    /// Something the agent said.
    Text,
    /// The run's final answer.
    Result,
    /// A line the CLI printed on stderr.
    Stderr,
    Ended,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tool: String,
    #[serde(default)]
    pub text: String,
    /// Unix milliseconds.
    pub at: i64,
}

/// What a line said about the run as a whole, beyond the event itself.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub failed: bool,
    pub cost_usd: Option<f64>,
}

/// How much of a tool's input is worth a line in the log.
const MAX_DETAIL: usize = 160;

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(max).collect();
    clipped.push('…');
    clipped
}

/// One line saying what a Claude tool call was about: the command, the file,
/// the pattern, the URL.
fn describe_input(input: &serde_json::Value) -> String {
    // "agent" and "key": Termo's own tools for working with other agents.
    for key in ["command", "file_path", "path", "pattern", "url", "query", "description", "prompt", "agent", "key"] {
        if let Some(value) = input.get(key).and_then(|value| value.as_str()) {
            return clip(value, MAX_DETAIL);
        }
    }
    String::new()
}

/// Reads a run's output line by line. Claude's and Codex's lines each stand
/// on their own; Gemini CLI streams its answer in pieces, gathered here into
/// one Text event whenever it stops to use a tool, and into the Result at
/// the end.
pub struct Parser {
    engine: Engine,
    /// Gemini CLI's answer so far, since the last tool call.
    pending: String,
}

impl Parser {
    pub fn new(engine: Engine) -> Self {
        Self { engine, pending: String::new() }
    }

    /// The events in one line of output (none for the many bookkeeping
    /// lines), plus what it says about the run's outcome.
    pub fn line(&mut self, line: &str, at: i64) -> (Vec<RunEvent>, Outcome) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            return (Vec::new(), Outcome::default());
        };
        match self.engine {
            Engine::Claude => parse_claude(&value, at),
            Engine::Codex => parse_codex(&value, at),
            Engine::Gemini => self.gemini(&value, at),
        }
    }

    /// What's left when the output ends without a result: the answer so far.
    pub fn finish(&mut self, at: i64) -> Vec<RunEvent> {
        self.flush(at).into_iter().collect()
    }

    fn flush(&mut self, at: i64) -> Option<RunEvent> {
        let text = std::mem::take(&mut self.pending);
        (!text.trim().is_empty()).then(|| event(EventKind::Text, "", text.trim().to_string(), at))
    }

    fn gemini(&mut self, value: &serde_json::Value, at: i64) -> (Vec<RunEvent>, Outcome) {
        let mut events = Vec::new();
        let mut outcome = Outcome::default();
        let text = |key: &str| value.get(key).and_then(|text| text.as_str()).unwrap_or("");
        match text("type") {
            "message" if text("role") == "assistant" => self.pending.push_str(text("content")),
            "tool_use" => {
                events.extend(self.flush(at));
                let detail = value.get("parameters").map(describe_input).unwrap_or_default();
                events.push(event(EventKind::Tool, text("tool_name"), detail, at));
            }
            "tool_result" if text("status") == "error" => {
                let message = value.pointer("/error/message").and_then(|message| message.as_str()).unwrap_or("");
                events.push(event(EventKind::Stderr, "", message.to_string(), at));
            }
            "error" => events.push(event(EventKind::Stderr, "", text("message").to_string(), at)),
            "result" => {
                outcome.failed = text("status") != "success";
                if let Some(message) = value.pointer("/error/message").and_then(|message| message.as_str()) {
                    events.push(event(EventKind::Stderr, "", message.to_string(), at));
                }
                let answer = std::mem::take(&mut self.pending);
                events.push(event(EventKind::Result, "", answer.trim().to_string(), at));
            }
            _ => {}
        }
        (events, outcome)
    }
}

/// One line on its own, for the CLIs whose lines stand alone.
#[cfg(test)]
pub fn parse_line(engine: Engine, line: &str, at: i64) -> (Vec<RunEvent>, Outcome) {
    Parser::new(engine).line(line, at)
}

fn event(kind: EventKind, tool: &str, text: String, at: i64) -> RunEvent {
    RunEvent {
        kind,
        tool: tool.to_string(),
        text,
        at,
    }
}

fn parse_claude(value: &serde_json::Value, at: i64) -> (Vec<RunEvent>, Outcome) {
    let mut events = Vec::new();
    let mut outcome = Outcome::default();
    match value.get("type").and_then(|kind| kind.as_str()) {
        Some("assistant") => {
            let content = value.pointer("/message/content").and_then(|content| content.as_array());
            for item in content.into_iter().flatten() {
                match item.get("type").and_then(|kind| kind.as_str()) {
                    Some("tool_use") => {
                        let name = item.get("name").and_then(|name| name.as_str()).unwrap_or("?");
                        let detail = item.get("input").map(describe_input).unwrap_or_default();
                        events.push(event(EventKind::Tool, name, detail, at));
                    }
                    Some("text") => {
                        let text = item.get("text").and_then(|text| text.as_str()).unwrap_or("");
                        if !text.trim().is_empty() {
                            events.push(event(EventKind::Text, "", text.trim().to_string(), at));
                        }
                    }
                    _ => {}
                }
            }
        }
        Some("result") => {
            outcome.failed = value.get("is_error").and_then(|flag| flag.as_bool()).unwrap_or(false)
                || value
                    .get("subtype")
                    .and_then(|subtype| subtype.as_str())
                    .is_some_and(|subtype| subtype != "success");
            outcome.cost_usd = value.get("total_cost_usd").and_then(|cost| cost.as_f64());
            let text = value.get("result").and_then(|text| text.as_str()).unwrap_or("");
            events.push(event(EventKind::Result, "", text.trim().to_string(), at));
        }
        _ => {}
    }
    (events, outcome)
}

fn parse_codex(value: &serde_json::Value, at: i64) -> (Vec<RunEvent>, Outcome) {
    let mut events = Vec::new();
    let mut outcome = Outcome::default();
    let item = value.get("item");
    let item_type = item.and_then(|item| item.get("type")).and_then(|kind| kind.as_str());
    let field = |key: &str| {
        item.and_then(|item| item.get(key))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string()
    };
    match value.get("type").and_then(|kind| kind.as_str()) {
        Some("item.started") => match item_type {
            Some("command_execution") => events.push(event(EventKind::Tool, "Shell", clip(&field("command"), MAX_DETAIL), at)),
            Some("mcp_tool_call") => {
                let name = format!("{}:{}", field("server"), field("tool"));
                events.push(event(EventKind::Tool, &name, String::new(), at));
            }
            Some("web_search") => events.push(event(EventKind::Tool, "WebSearch", clip(&field("query"), MAX_DETAIL), at)),
            _ => {}
        },
        Some("item.completed") => match item_type {
            // Codex's last message is its answer; there's no separate result
            // line, so every message is kept as text and the runner promotes
            // the last one.
            Some("agent_message") => events.push(event(EventKind::Text, "", field("text").trim().to_string(), at)),
            Some("file_change") => {
                let paths: Vec<String> = item
                    .and_then(|item| item.get("changes"))
                    .and_then(|changes| changes.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|change| change.get("path").and_then(|path| path.as_str()).map(String::from))
                    .collect();
                events.push(event(EventKind::Tool, "Edit", clip(&paths.join(", "), MAX_DETAIL), at));
            }
            _ => {}
        },
        Some("turn.failed") | Some("error") => {
            outcome.failed = true;
            let message = value
                .pointer("/error/message")
                .or_else(|| value.get("message"))
                .and_then(|message| message.as_str())
                .unwrap_or("");
            events.push(event(EventKind::Stderr, "", message.to_string(), at));
        }
        _ => {}
    }
    (events, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_tool_calls_say_what_they_touched() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Glob","input":{"pattern":"*.ps1"}}]}}"#;
        let (events, outcome) = parse_line(Engine::Claude, line, 1);
        assert_eq!(events, vec![event(EventKind::Tool, "Glob", "*.ps1".into(), 1)]);
        assert!(!outcome.failed);
    }

    #[test]
    fn claude_result_carries_the_answer_and_the_cost() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"result":" Listo. ","total_cost_usd":0.2}"#;
        let (events, outcome) = parse_line(Engine::Claude, line, 1);
        assert_eq!(events[0].kind, EventKind::Result);
        assert_eq!(events[0].text, "Listo.");
        assert_eq!(outcome.cost_usd, Some(0.2));
        assert!(!outcome.failed);
        let (_, failed) = parse_line(Engine::Claude, r#"{"type":"result","subtype":"error_max_turns","is_error":true}"#, 1);
        assert!(failed.failed);
    }

    #[test]
    fn bookkeeping_and_garbage_lines_are_ignored() {
        assert!(parse_line(Engine::Claude, r#"{"type":"system","subtype":"init"}"#, 1).0.is_empty());
        assert!(parse_line(Engine::Claude, "not json", 1).0.is_empty());
        assert!(parse_line(Engine::Codex, r#"{"type":"turn.started"}"#, 1).0.is_empty());
    }

    #[test]
    fn codex_commands_messages_and_failures() {
        let started = r#"{"type":"item.started","item":{"type":"command_execution","command":"ls"}}"#;
        assert_eq!(parse_line(Engine::Codex, started, 1).0[0].tool, "Shell");
        let message = r#"{"type":"item.completed","item":{"type":"agent_message","text":"Hay 17."}}"#;
        assert_eq!(parse_line(Engine::Codex, message, 1).0[0].text, "Hay 17.");
        let (events, outcome) = parse_line(Engine::Codex, r#"{"type":"turn.failed","error":{"message":"quota"}}"#, 1);
        assert!(outcome.failed);
        assert_eq!(events[0].text, "quota");
    }

    /// Gemini CLI's pieces of text come out whole: one Text before a tool
    /// call, the rest in the Result.
    #[test]
    fn gemini_answers_are_gathered_from_their_pieces() {
        let mut parser = Parser::new(Engine::Gemini);
        let lines = [
            r#"{"type":"init","session_id":"s1","model":"gemini-2.5-pro"}"#,
            r#"{"type":"message","role":"user","content":"Revisá"}"#,
            r#"{"type":"message","role":"assistant","content":"Voy a ","delta":true}"#,
            r#"{"type":"message","role":"assistant","content":"mirar.","delta":true}"#,
            r#"{"type":"tool_use","tool_name":"read_file","tool_id":"t1","parameters":{"file_path":"a.txt"}}"#,
            r#"{"type":"tool_result","tool_id":"t1","status":"success","output":"hola"}"#,
            r#"{"type":"message","role":"assistant","content":"Dice ","delta":true}"#,
            r#"{"type":"message","role":"assistant","content":"hola.","delta":true}"#,
            r#"{"type":"result","status":"success","stats":{}}"#,
        ];
        let events: Vec<RunEvent> = lines.iter().flat_map(|line| parser.line(line, 1).0).collect();
        assert_eq!(
            events,
            vec![
                event(EventKind::Text, "", "Voy a mirar.".into(), 1),
                event(EventKind::Tool, "read_file", "a.txt".into(), 1),
                event(EventKind::Result, "", "Dice hola.".into(), 1),
            ]
        );
    }

    #[test]
    fn gemini_failures_say_why() {
        let mut parser = Parser::new(Engine::Gemini);
        let (events, outcome) = parser.line(r#"{"type":"result","status":"error","error":{"type":"API","message":"quota"}}"#, 1);
        assert!(outcome.failed);
        assert_eq!(events[0], event(EventKind::Stderr, "", "quota".into(), 1));
        let (events, _) = parser.line(r#"{"type":"tool_result","tool_id":"t","status":"error","error":{"message":"denied"}}"#, 1);
        assert_eq!(events[0].text, "denied");
        // Output that ends without a result keeps what was said.
        parser.line(r#"{"type":"message","role":"assistant","content":"a medias","delta":true}"#, 1);
        assert_eq!(parser.finish(2), vec![event(EventKind::Text, "", "a medias".into(), 2)]);
    }

    #[test]
    fn long_inputs_are_clipped() {
        let long = "x".repeat(500);
        let line = format!(r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Bash","input":{{"command":"{long}"}}}}]}}}}"#);
        let text = &parse_line(Engine::Claude, &line, 1).0[0].text;
        assert!(text.chars().count() <= MAX_DETAIL + 1 && text.ends_with('…'));
    }
}
