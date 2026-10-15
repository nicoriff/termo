//! Turning each CLI's machine-readable output into one shape of event:
//! Claude's `--output-format stream-json`, Codex's `exec --json` and Copilot
//! CLI's `--output-format json` all print one JSON object per line, in
//! different vocabularies; so do Grok and Gemini CLI, whose lines are the
//! messages of an ACP session (see acp.rs).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::diagnose::Failure;
use super::spec::Engine;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum EventKind {
    Started,
    /// What the CLI said about itself when it started: its version, the
    /// model it really uses, how it's logged in (`session`).
    Session,
    /// The CLI warned about the plan's usage limits (`usage`).
    Usage,
    /// Why the run failed, said precisely (`failure`).
    Failure,
    /// The agent used a tool: `tool` is its name, `text` what it did it on.
    Tool,
    /// Something the agent said.
    #[default]
    Text,
    /// The run's final answer.
    Result,
    /// A line the CLI printed on stderr.
    Stderr,
    /// Something the agent asked the user mid-run (see run_questions.rs).
    Question,
    /// What the user answered; empty when nobody did in time.
    Answer,
    Ended,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tool: String,
    #[serde(default)]
    pub text: String,
    /// Unix milliseconds.
    pub at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
}

/// What ran a run: Termo's side when it starts (the CLI, the model asked
/// for, the folder), and the CLI's own report when it gives one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionInfo {
    pub engine: Engine,
    /// The CLI's version, when it says.
    pub version: String,
    /// The model: asked for (empty: the CLI's default), or in use.
    pub model: String,
    /// How the CLI is logged in: "subscription", "apiKey" or empty.
    pub auth: String,
    /// Where the key comes from, when it's one (an environment variable…).
    pub auth_source: String,
    pub cwd: String,
    /// The executable, at the start.
    pub command: String,
    /// The MCP servers the CLI reported and their state ("connected",
    /// "failed"…), when it reported them.
    pub mcp: Vec<McpState>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct McpState {
    pub name: String,
    pub status: String,
}

/// A usage-limit warning from the CLI (Claude Code reports these).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UsageInfo {
    /// "five_hour", "seven_day"… as the CLI names the window.
    pub window: String,
    /// "allowed_warning", "rejected"…
    pub status: String,
    /// 0 to 1.
    pub utilization: f64,
    /// When the window starts over (Unix ms; 0: not said).
    pub resets_at: i64,
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
pub fn describe_input(input: &serde_json::Value) -> String {
    // "agent" and "key": Termo's own tools for working with other agents.
    for key in ["command", "file_path", "path", "pattern", "url", "query", "description", "prompt", "agent", "key"] {
        if let Some(value) = input.get(key).and_then(|value| value.as_str()) {
            return clip(value, MAX_DETAIL);
        }
    }
    String::new()
}

/// Reads a run's output line by line. Claude's and Codex's lines each stand
/// on their own; an ACP session streams its answer in pieces, gathered here
/// into one Text event whenever it stops to use a tool, and into the Result
/// at the end.
pub struct Parser {
    engine: Engine,
    /// ACP: turns cut short so far (see acp::MAX_REFUSED_TURNS).
    cut_short: u32,
    /// Grok: what the run's turns cost so far, in its ticks.
    spent_ticks: f64,
    /// ACP: the answer so far, since the last tool call.
    pending: String,
    /// The errors the CLI reported, to explain a failure (see diagnose.rs).
    pub errors: Vec<String>,
    /// The model the CLI said it uses, when it said.
    pub model: String,
    /// Usage warnings already shown: once per window and state.
    warned: HashSet<String>,
}

impl Parser {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            cut_short: 0,
            spent_ticks: 0.0,
            pending: String::new(),
            errors: Vec::new(),
            model: String::new(),
            warned: HashSet::new(),
        }
    }

    /// The events in one line of output (none for the many bookkeeping
    /// lines), plus what it says about the run's outcome.
    pub fn line(&mut self, line: &str, at: i64) -> (Vec<RunEvent>, Outcome) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            return (Vec::new(), Outcome::default());
        };
        let (mut events, outcome) = match self.engine {
            Engine::Claude => parse_claude(&value, at),
            Engine::Codex => parse_codex(&value, at),
            Engine::Copilot => parse_copilot(&value, at),
            Engine::Gemini | Engine::Grok => self.acp(&value, at),
        };
        events.extend(self.notice(&value, at));
        // What it reported as errors, kept to explain a failure.
        let failed_result = outcome.failed && self.engine == Engine::Claude;
        for event in &events {
            let error = match event.kind {
                EventKind::Stderr => true,
                EventKind::Result => failed_result,
                _ => false,
            };
            if error && !event.text.trim().is_empty() {
                self.errors.push(event.text.clone());
            }
        }
        (events, outcome)
    }

    /// What the CLI says about itself and its limits, beyond the run's own
    /// steps: the session it started, usage warnings.
    fn notice(&mut self, value: &serde_json::Value, at: i64) -> Option<RunEvent> {
        let text = |key: &str| value.get(key).and_then(|text| text.as_str()).unwrap_or("").to_string();
        let kind = text("type");
        // Claude Code's first line.
        if self.engine == Engine::Claude && kind == "system" && text("subtype") == "init" {
            let model = text("model");
            self.model.clone_from(&model);
            let key_source = text("apiKeySource");
            let (auth, auth_source) = match key_source.as_str() {
                "" => (String::new(), String::new()),
                // Logged in with a Claude account, no key.
                "none" => ("subscription".to_string(), String::new()),
                source => ("apiKey".to_string(), source.to_string()),
            };
            let mcp = value
                .get("mcp_servers")
                .and_then(|servers| servers.as_array())
                .into_iter()
                .flatten()
                .map(|server| McpState {
                    name: server.get("name").and_then(|name| name.as_str()).unwrap_or("").to_string(),
                    status: server.get("status").and_then(|status| status.as_str()).unwrap_or("").to_string(),
                })
                .filter(|server| !server.name.is_empty())
                .collect();
            let session = SessionInfo {
                engine: self.engine,
                version: text("claude_code_version"),
                model,
                auth,
                auth_source,
                cwd: text("cwd"),
                mcp,
                ..SessionInfo::default()
            };
            return Some(RunEvent { kind: EventKind::Session, at, session: Some(session), ..RunEvent::default() });
        }
        if self.engine == Engine::Claude && kind == "rate_limit_event" {
            let info = value.get("rate_limit_info")?;
            let field = |key: &str| info.get(key).and_then(|text| text.as_str()).unwrap_or("").to_string();
            let status = field("status");
            // Only what's worth a line: a warning, or a refusal.
            if status.is_empty() || status == "allowed" {
                return None;
            }
            let usage = UsageInfo {
                window: field("rateLimitType"),
                utilization: info.get("utilization").and_then(|value| value.as_f64()).unwrap_or(0.0),
                resets_at: info.get("resetsAt").and_then(|value| value.as_i64()).unwrap_or(0) * 1000,
                status,
            };
            if !self.warned.insert(format!("{}:{}", usage.window, usage.status)) {
                return None;
            }
            return Some(RunEvent { kind: EventKind::Usage, at, usage: Some(usage), ..RunEvent::default() });
        }
        None
    }

    /// What's left when the output ends without a result: the answer so far.
    pub fn finish(&mut self, at: i64) -> Vec<RunEvent> {
        self.flush(at).into_iter().collect()
    }

    fn flush(&mut self, at: i64) -> Option<RunEvent> {
        let text = std::mem::take(&mut self.pending);
        (!text.trim().is_empty()).then(|| event(EventKind::Text, "", text.trim().to_string(), at))
    }
}

/// What Grok bills a turn at comes in "ticks": ten thousand million to the
/// dollar.
const GROK_TICKS_PER_USD: f64 = 1e10;

impl Parser {
    /// An ACP session's messages (Grok's, Gemini CLI's). The answer streams
    /// in pieces: gathered into one Text event whenever it stops to use a
    /// tool, and into the Result when the turn's reply comes.
    fn acp(&mut self, value: &serde_json::Value, at: i64) -> (Vec<RunEvent>, Outcome) {
        let mut events = Vec::new();
        let mut outcome = Outcome::default();
        let text = |pointer: &str| value.pointer(pointer).and_then(|text| text.as_str()).unwrap_or("");
        match text("/method") {
            "session/update" => match text("/params/update/sessionUpdate") {
                "agent_message_chunk" => self.pending.push_str(text("/params/update/content/text")),
                "tool_call" => {
                    events.extend(self.flush(at));
                    let detail = value.pointer("/params/update/rawInput").map(describe_input).unwrap_or_default();
                    events.push(event(EventKind::Tool, text("/params/update/title"), detail, at));
                }
                "tool_call_update" if text("/params/update/status") == "failed" => {
                    let said = text("/params/update/content/0/content/text");
                    let message = if said.is_empty() { text("/params/update/title") } else { said };
                    events.push(event(EventKind::Stderr, "", clip(message, MAX_DETAIL), at));
                }
                _ => {}
            },
            // Each turn says what it cost; the run's total goes with its answer.
            "_x.ai/session_notification" if text("/params/update/sessionUpdate") == "turn_completed" => {
                let ticks = value.pointer("/params/update/usage/costUsdTicks").and_then(|ticks| ticks.as_f64());
                self.spent_ticks += ticks.unwrap_or(0.0);
            }
            // A reply to one of Termo's requests (acp.rs sends them).
            "" => {
                if let Some(message) = value.pointer("/error/message").and_then(|message| message.as_str()) {
                    outcome.failed = true;
                    events.push(event(EventKind::Stderr, "", message.to_string(), at));
                } else if value.pointer("/result/agentCapabilities").is_some() {
                    // Its answer to `initialize`: what it is and what it runs.
                    let model = text("/result/_meta/modelState/currentModelId").to_string();
                    self.model.clone_from(&model);
                    // Grok's own field; the protocol's, which Gemini CLI fills.
                    let version = [text("/result/_meta/agentVersion"), text("/result/agentInfo/version")]
                        .into_iter()
                        .find(|version| !version.is_empty())
                        .unwrap_or_default();
                    let session = SessionInfo {
                        engine: self.engine,
                        version: version.to_string(),
                        model,
                        ..SessionInfo::default()
                    };
                    events.push(RunEvent { kind: EventKind::Session, at, session: Some(session), ..RunEvent::default() });
                } else if let Some(model) = value.pointer("/result/models/currentModelId").and_then(|model| model.as_str()) {
                    // Gemini CLI's answer to `session/new`: the model it runs.
                    self.model = model.to_string();
                } else if let Some(reason) = value.pointer("/result/stopReason").and_then(|reason| reason.as_str()) {
                    // The turn's own reply. One cut short by a permission
                    // Termo refused isn't the end: acp.rs sends it on, up to
                    // a point.
                    self.cut_short += u32::from(reason == super::acp::CANCELLED);
                    if reason == super::acp::CANCELLED && self.cut_short <= super::acp::MAX_REFUSED_TURNS {
                        events.extend(self.flush(at));
                    } else {
                        // Anything but a normal end (the model refusing, the
                        // token or turn limit) left it unfinished.
                        outcome.failed = reason != "end_turn";
                        outcome.cost_usd = (self.spent_ticks > 0.0).then(|| self.spent_ticks / GROK_TICKS_PER_USD);
                        let answer = std::mem::take(&mut self.pending);
                        events.push(event(EventKind::Result, "", answer.trim().to_string(), at));
                    }
                }
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

pub fn event(kind: EventKind, tool: &str, text: String, at: i64) -> RunEvent {
    RunEvent {
        kind,
        tool: tool.to_string(),
        text,
        at,
        ..RunEvent::default()
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

/// Copilot CLI's events keep their fields under `data`. Its messages stand
/// alone, like Codex's: the runner promotes the last one to the answer.
fn parse_copilot(value: &serde_json::Value, at: i64) -> (Vec<RunEvent>, Outcome) {
    let mut events = Vec::new();
    let mut outcome = Outcome::default();
    let data = value.get("data").unwrap_or(&serde_json::Value::Null);
    let text = |pointer: &str| data.pointer(pointer).and_then(|text| text.as_str()).unwrap_or("");
    match value.get("type").and_then(|kind| kind.as_str()) {
        Some("assistant.message") => {
            let content = text("/content").trim();
            if !content.is_empty() {
                events.push(event(EventKind::Text, "", content.to_string(), at));
            }
            let requests = data.get("toolRequests").and_then(|requests| requests.as_array());
            for request in requests.into_iter().flatten() {
                let name = request.get("name").and_then(|name| name.as_str()).unwrap_or("?");
                let detail = request.get("arguments").map(describe_input).unwrap_or_default();
                events.push(event(EventKind::Tool, name, detail, at));
            }
        }
        Some("tool.execution_complete") if data.get("success").and_then(|flag| flag.as_bool()) == Some(false) => {
            let message = ["/error/message", "/error", "/result/content"]
                .iter()
                .map(|pointer| text(pointer))
                .find(|message| !message.is_empty())
                .unwrap_or("");
            events.push(event(EventKind::Stderr, "", clip(message, MAX_DETAIL), at));
        }
        Some("session.error") => {
            outcome.failed = true;
            events.push(event(EventKind::Stderr, "", text("/message").to_string(), at));
        }
        Some("result") => {
            outcome.failed = value.get("exitCode").and_then(|code| code.as_i64()).is_some_and(|code| code != 0);
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
        assert!(parse_line(Engine::Claude, r#"{"type":"system","subtype":"hook_started"}"#, 1).0.is_empty());
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

    /// Gemini CLI, over ACP: the protocol's own fields for its version and
    /// model, no cost, and a refused tool said as such. Lines as it really
    /// sends them.
    #[test]
    fn gemini_over_acp_reports_its_version_model_tools_and_refusals() {
        let mut parser = Parser::new(Engine::Gemini);
        let mut events = Vec::new();
        let mut outcome = Outcome::default();
        for line in [
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"authMethods":[{"id":"oauth-personal"}],"agentInfo":{"name":"gemini-cli","title":"Gemini CLI","version":"0.63.0"},"agentCapabilities":{"loadSession":true}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"result":{"sessionId":"s1","modes":{"currentModeId":"default"},"models":{"currentModelId":"auto"}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Miro."}}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"tool_call","toolCallId":"web_fetch__call_1","status":"pending","title":"Fetch https://example.com","content":[],"locations":[],"kind":"fetch"}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"tool_call_update","toolCallId":"web_fetch__call_1","status":"failed","content":[{"type":"content","content":{"type":"text","text":"Tool \"web_fetch\" was canceled by the user."}}],"kind":"fetch"}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Sin web."}}}}"#,
            r#"{"jsonrpc":"2.0","id":4,"result":{"stopReason":"end_turn","_meta":{"quota":{"token_count":{"input_tokens":10,"output_tokens":2}}}}}"#,
        ] {
            let (mut found, said) = parser.line(line, 1);
            events.append(&mut found);
            outcome.failed |= said.failed;
            outcome.cost_usd = said.cost_usd.or(outcome.cost_usd);
        }
        let said: Vec<(EventKind, &str, &str)> =
            events.iter().map(|event| (event.kind, event.tool.as_str(), event.text.as_str())).collect();
        assert_eq!(
            said,
            [
                (EventKind::Session, "", ""),
                (EventKind::Text, "", "Miro."),
                (EventKind::Tool, "Fetch https://example.com", ""),
                (EventKind::Stderr, "", "Tool \"web_fetch\" was canceled by the user."),
                (EventKind::Result, "", "Sin web."),
            ]
        );
        assert_eq!(events[0].session.as_ref().unwrap().version, "0.63.0");
        assert_eq!(parser.model, "auto");
        assert_eq!(outcome, Outcome::default());
    }

    /// Grok, over ACP: its answer comes in chunks, cut wherever it stops for
    /// a tool, and the turn's reply closes it. Lines as it really sends them.
    #[test]
    fn grok_gathers_its_answer_around_tools_and_reports_the_cost() {
        let mut parser = Parser::new(Engine::Grok);
        let mut events = Vec::new();
        let mut cost = None;
        let mut failed = false;
        for line in [
            r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{},"_meta":{"agentVersion":"1.0.50","modelState":{"currentModelId":"grok-4.7"}}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"hmm"}}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Voy a "}}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"crearlo."}}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"tool_call","toolCallId":"c0","title":"search_replace","rawInput":{"file_path":"a.txt","new_string":"hi","old_string":""}}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"tool_call_update","toolCallId":"c0","status":"completed"}}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"ok"}}}}"#,
            r#"{"jsonrpc":"2.0","method":"_x.ai/session_notification","params":{"update":{"sessionUpdate":"turn_completed","stop_reason":"end_turn","usage":{"costUsdTicks":272760000}}}}"#,
            r#"{"jsonrpc":"2.0","method":"_x.ai/session_notification","params":{"update":{"sessionUpdate":"turn_completed","stop_reason":"end_turn","usage":{"costUsdTicks":100000000}}}}"#,
            r#"{"jsonrpc":"2.0","id":4,"result":{"stopReason":"end_turn"}}"#,
        ] {
            let (mut found, outcome) = parser.line(line, 1);
            events.append(&mut found);
            cost = outcome.cost_usd.or(cost);
            failed |= outcome.failed;
        }
        let said: Vec<(EventKind, &str, &str)> =
            events.iter().map(|event| (event.kind, event.tool.as_str(), event.text.as_str())).collect();
        assert_eq!(
            said,
            [
                (EventKind::Session, "", ""),
                (EventKind::Text, "", "Voy a crearlo."),
                (EventKind::Tool, "search_replace", "a.txt"),
                (EventKind::Result, "", "ok"),
            ]
        );
        let session = events[0].session.as_ref().unwrap();
        assert_eq!((session.version.as_str(), session.model.as_str()), ("1.0.50", "grok-4.7"));
        assert_eq!(parser.model, "grok-4.7");
        assert!((cost.unwrap() - 0.037276).abs() < 1e-9);
        assert!(!failed);
    }

    #[test]
    fn grok_turns_that_end_early_or_are_refused_fail() {
        let limit = r#"{"jsonrpc":"2.0","id":4,"result":{"stopReason":"max_tokens"}}"#;
        assert!(parse_line(Engine::Grok, limit, 1).1.failed);
        // Cut short by a refused permission: sent on, until it's too many.
        let cancelled = r#"{"jsonrpc":"2.0","id":4,"result":{"stopReason":"cancelled"}}"#;
        let mut parser = Parser::new(Engine::Grok);
        for _ in 0..crate::agents::acp::MAX_REFUSED_TURNS {
            assert_eq!(parser.line(cancelled, 1), (Vec::new(), Outcome::default()));
        }
        let (events, outcome) = parser.line(cancelled, 1);
        assert!(outcome.failed);
        assert_eq!(events[0].kind, EventKind::Result);
        let refused = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"Authentication required"}}"#;
        let (events, outcome) = parse_line(Engine::Grok, refused, 1);
        assert!(outcome.failed);
        assert_eq!(events[0], event(EventKind::Stderr, "", "Authentication required".into(), 1));
        let tool = r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"tool_call_update","status":"failed","title":"Execute `x`","content":[{"type":"content","content":{"type":"text","text":"denied"}}]}}}"#;
        assert_eq!(parse_line(Engine::Grok, tool, 1).0[0], event(EventKind::Stderr, "", "denied".into(), 1));
    }

    /// Copilot CLI: what it says and the tools it asks for come in one
    /// message; a tool that failed says why.
    #[test]
    fn copilot_messages_tools_and_failures() {
        let line = r#"{"type":"assistant.message","data":{"content":"Miro.","toolRequests":[{"toolCallId":"c1","name":"view","arguments":{"path":"README.md"}}]}}"#;
        assert_eq!(
            parse_line(Engine::Copilot, line, 1).0,
            vec![
                event(EventKind::Text, "", "Miro.".into(), 1),
                event(EventKind::Tool, "view", "README.md".into(), 1),
            ]
        );
        let failed = r#"{"type":"tool.execution_complete","data":{"toolCallId":"c1","success":false,"error":{"message":"denied"}}}"#;
        assert_eq!(parse_line(Engine::Copilot, failed, 1).0[0], event(EventKind::Stderr, "", "denied".into(), 1));
        let ok = r#"{"type":"tool.execution_complete","data":{"toolCallId":"c1","success":true,"result":{"content":"x"}}}"#;
        assert!(parse_line(Engine::Copilot, ok, 1).0.is_empty());
        let (events, outcome) = parse_line(Engine::Copilot, r#"{"type":"session.error","data":{"message":"quota"}}"#, 1);
        assert!(outcome.failed);
        assert_eq!(events[0].text, "quota");
        assert!(parse_line(Engine::Copilot, r#"{"type":"result","exitCode":1}"#, 1).1.failed);
        assert!(!parse_line(Engine::Copilot, r#"{"type":"result","exitCode":0}"#, 1).1.failed);
    }

    /// Claude Code's first line says what really runs: the model, the
    /// version, how it's logged in and its MCP servers.
    #[test]
    fn claude_says_what_runs_it() {
        let mut parser = Parser::new(Engine::Claude);
        let init = r#"{"type":"system","subtype":"init","cwd":"E:/p","model":"claude-opus-5-5","claude_code_version":"2.1.290","apiKeySource":"none","mcp_servers":[{"name":"termo","status":"connected"},{"name":"x","status":"failed"}]}"#;
        let (events, _) = parser.line(init, 1);
        let session = events[0].session.clone().unwrap();
        assert_eq!(events[0].kind, EventKind::Session);
        assert_eq!((session.model.as_str(), session.version.as_str(), session.auth.as_str()), ("claude-opus-5-5", "2.1.290", "subscription"));
        assert_eq!(session.mcp[1], McpState { name: "x".into(), status: "failed".into() });
        assert_eq!(parser.model, "claude-opus-5-5");
        let (events, _) = parser.line(r#"{"type":"system","subtype":"init","apiKeySource":"ANTHROPIC_API_KEY"}"#, 1);
        let session = events[0].session.clone().unwrap();
        assert_eq!((session.auth.as_str(), session.auth_source.as_str()), ("apiKey", "ANTHROPIC_API_KEY"));
    }

    #[test]
    fn usage_warnings_show_once() {
        let mut parser = Parser::new(Engine::Claude);
        let warning = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":1791273600,"rateLimitType":"seven_day","utilization":0.92}}"#;
        let (events, _) = parser.line(warning, 1);
        let usage = events[0].usage.clone().unwrap();
        assert_eq!((usage.window.as_str(), usage.utilization, usage.resets_at), ("seven_day", 0.92, 1_791_273_600_000));
        assert!(parser.line(warning, 2).0.is_empty());
        let fine = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour"}}"#;
        assert!(parser.line(fine, 3).0.is_empty());
    }

    /// What a CLI reports as an error is kept to explain the failure; a
    /// result that isn't one isn't.
    #[test]
    fn errors_are_kept_for_the_diagnosis() {
        let mut parser = Parser::new(Engine::Claude);
        parser.line(r#"{"type":"result","subtype":"success","is_error":false,"result":"Listo"}"#, 1);
        assert!(parser.errors.is_empty());
        parser.line(r#"{"type":"result","subtype":"success","is_error":true,"result":"Failed to authenticate. API Error: 403 x"}"#, 1);
        assert_eq!(parser.errors, ["Failed to authenticate. API Error: 403 x"]);
        let mut codex = Parser::new(Engine::Codex);
        codex.line(r#"{"type":"error","message":"unexpected status 401 Unauthorized"}"#, 1);
        assert_eq!(codex.errors, ["unexpected status 401 Unauthorized"]);
    }

    #[test]
    fn long_inputs_are_clipped() {
        let long = "x".repeat(500);
        let line = format!(r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Bash","input":{{"command":"{long}"}}}}]}}}}"#);
        let text = &parse_line(Engine::Claude, &line, 1).0[0].text;
        assert!(text.chars().count() <= MAX_DETAIL + 1 && text.ends_with('…'));
    }
}
