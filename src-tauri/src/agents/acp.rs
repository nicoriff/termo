//! The client side of an Agent Client Protocol session, for the engines that
//! run through it: Grok (`grok agent stdio`) and Gemini CLI (`gemini --acp`).
//! Their headless modes take no MCP servers for a run (Gemini CLI's, only
//! from a settings file it won't read from a folder the user owns); a
//! session opened over ACP does.
//!
//! Where the other CLIs take a prompt on stdin and print events, this is a
//! conversation in JSON-RPC, one message per line: Termo opens the session,
//! hands it the run's servers and the prompt, and answers each permission
//! the agent asks for the way the agent's autonomy says. `Driver` is that
//! conversation as a state machine over lines, with no I/O of its own (the
//! runner owns the pipes); what the agent says along the way is turned into
//! run events by events.rs, from the same lines.

use serde_json::{json, Value};

use super::spec::{AcpRun, Autonomy};

const INITIALIZE: u64 = 1;
const AUTHENTICATE: u64 = 2;
const NEW_SESSION: u64 = 3;
/// The first prompt; each one after it (see `MAX_REFUSED_TURNS`) counts on.
const PROMPT: u64 = 4;

/// Grok ends its turn when a permission is refused, waiting to be told what
/// to do instead. Nobody is there to say, so Termo does: it may not, and to
/// go on without. This many times in a run; after that the run has failed.
pub const MAX_REFUSED_TURNS: u32 = 3;

/// How a turn that was cut short says so.
pub const CANCELLED: &str = "cancelled";

/// What a line from the agent calls for.
#[derive(Debug, Default, PartialEq)]
pub struct Step {
    /// Lines to write back, in order.
    pub send: Vec<String>,
    /// The turn is over (or can't go on): close the agent's input, which is
    /// what makes it exit.
    pub done: bool,
}

pub struct Driver {
    run: AcpRun,
    cwd: String,
    session: String,
    /// What was refused in the turn under way, as Grok titled it.
    refused: Vec<String>,
    /// Turns already sent on after a refusal.
    resumed: u32,
}

fn request(id: u64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

/// Whether the agent's autonomy covers a tool call of this kind. `built_in`
/// and `read_only` are Grok's own account of the tool, when it gives one.
fn allowed(run: &AcpRun, kind: &str, built_in: bool, read_only: bool) -> bool {
    match kind {
        "edit" | "delete" | "move" => run.autonomy >= Autonomy::Edit,
        "execute" => run.autonomy == Autonomy::Run,
        "fetch" => run.web,
        "read" | "search" | "think" => true,
        // A kind this doesn't know. One of Grok's own tools that changes
        // something counts as an edit; anything else is a server the agent
        // was given, whose tools are its to use.
        _ => !built_in || read_only || run.autonomy >= Autonomy::Edit,
    }
}

impl Driver {
    pub fn new(run: AcpRun, cwd: String) -> Self {
        Self { run, cwd, session: String::new(), refused: Vec::new(), resumed: 0 }
    }

    /// The line that opens the conversation.
    pub fn start(&self) -> String {
        request(
            INITIALIZE,
            "initialize",
            // No file or terminal access through Termo: the agent uses its own.
            json!({
                "protocolVersion": 1,
                "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false },
            }),
        )
    }

    pub fn line(&mut self, line: &str) -> Step {
        let Ok(message) = serde_json::from_str::<Value>(line.trim()) else { return Step::default() };
        let id = message.get("id");
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            // Something the agent asks of Termo; a notification (no id) asks nothing.
            return match id {
                Some(id) => {
                    let id = id.clone();
                    Step { send: vec![self.answer(&id, method, &message)], done: false }
                }
                None => Step::default(),
            };
        }
        let Some(id) = id.and_then(Value::as_u64) else { return Step::default() };
        // A step the agent refused: nothing more to say to it (events.rs
        // reports the error).
        let Some(result) = message.get("result") else { return Step { send: Vec::new(), done: true } };
        match id {
            // Gemini CLI: the login its settings already name.
            INITIALIZE if !self.run.sign_in => self.send(self.new_session()),
            INITIALIZE => {
                // Its saved login when it has one, as `grok login` leaves it.
                let methods: Vec<&str> = result
                    .get("authMethods")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|method| method.get("id").and_then(Value::as_str))
                    .collect();
                let preferred = result.pointer("/_meta/defaultAuthMethodId").and_then(Value::as_str);
                match preferred.filter(|id| methods.contains(id)).or(methods.first().copied()) {
                    Some(method) => self.send(request(AUTHENTICATE, "authenticate", json!({ "methodId": method }))),
                    None => self.send(self.new_session()),
                }
            }
            AUTHENTICATE => self.send(self.new_session()),
            NEW_SESSION => match result.get("sessionId").and_then(Value::as_str) {
                Some(session) => {
                    self.session = session.to_string();
                    self.send(self.prompt(PROMPT, &self.run.prompt))
                }
                None => Step { send: Vec::new(), done: true },
            },
            // A turn's own reply.
            id if id >= PROMPT => {
                let refused = std::mem::take(&mut self.refused);
                let cut_short = result.get("stopReason").and_then(Value::as_str) == Some(CANCELLED);
                if !cut_short || refused.is_empty() || self.resumed >= MAX_REFUSED_TURNS {
                    return Step { send: Vec::new(), done: true };
                }
                self.resumed += 1;
                let text = format!(
                    "That was refused, and will be every time: this run isn't allowed to do it ({}). Don't try it again \
or work around it. Go on with what you are allowed to do, and give your answer.",
                    refused.join("; ")
                );
                self.send(self.prompt(PROMPT + u64::from(self.resumed), &text))
            }
            _ => Step::default(),
        }
    }

    fn prompt(&self, id: u64, text: &str) -> String {
        request(id, "session/prompt", json!({ "sessionId": self.session, "prompt": [{ "type": "text", "text": text }] }))
    }

    fn send(&self, line: String) -> Step {
        Step { send: vec![line], done: false }
    }

    fn new_session(&self) -> String {
        request(NEW_SESSION, "session/new", json!({ "cwd": self.cwd, "mcpServers": self.run.mcp_servers }))
    }

    /// Termo's answer to a request from the agent. Nobody is watching a run
    /// to approve anything: a permission is granted or refused on the spot.
    fn answer(&mut self, id: &Value, method: &str, message: &Value) -> String {
        if method != "session/request_permission" {
            return json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } })
                .to_string();
        }
        let call = message.pointer("/params/toolCall").unwrap_or(&Value::Null);
        let tool = call.pointer("/_meta/x.ai~1tool").unwrap_or(&Value::Null);
        let kind = call.get("kind").or(tool.get("kind")).and_then(Value::as_str).unwrap_or("");
        let built_in = tool.get("namespace").and_then(Value::as_str) == Some("grok_build");
        let read_only = tool.get("read_only").and_then(Value::as_bool).unwrap_or(false);
        // Once, never "always": each call is judged on its own.
        let wanted: &[&str] = if allowed(&self.run, kind, built_in, read_only) {
            &["allow_once", "allow_always"]
        } else {
            let title = call.get("title").and_then(Value::as_str).filter(|title| !title.is_empty());
            self.refused.push(title.unwrap_or(kind).chars().take(120).collect());
            &["reject_once", "reject_always"]
        };
        let options: Vec<&Value> =
            message.pointer("/params/options").and_then(Value::as_array).into_iter().flatten().collect();
        let option = wanted.iter().find_map(|kind| {
            options
                .iter()
                .find(|option| option.get("kind").and_then(Value::as_str) == Some(kind))
                .and_then(|option| option.get("optionId"))
        });
        let outcome = match option {
            Some(option) => json!({ "outcome": "selected", "optionId": option }),
            // Nothing to pick that says what Termo means: the call is dropped.
            None => json!({ "outcome": "cancelled" }),
        };
        json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": outcome } }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn driver(autonomy: Autonomy, web: bool) -> Driver {
        let run = AcpRun {
            prompt: "Resumí los cambios".into(),
            mcp_servers: vec![json!({ "type": "http", "name": "termo", "url": "http://127.0.0.1:1/mcp", "headers": [] })],
            autonomy,
            web,
            sign_in: true,
        };
        Driver::new(run, "C:/repo".into())
    }

    fn sent(step: &Step) -> Value {
        assert_eq!(step.send.len(), 1, "{step:?}");
        serde_json::from_str(&step.send[0]).unwrap()
    }

    fn permission(kind: &str, tool: Value, id: u64) -> String {
        json!({
            "jsonrpc": "2.0", "id": id, "method": "session/request_permission",
            "params": { "sessionId": "s1", "toolCall": { "toolCallId": "c1", "kind": kind, "title": "Edit `a.txt`", "_meta": { "x.ai/tool": tool } },
                "options": [
                    { "optionId": "allow-edits-session", "name": "Yes, always", "kind": "allow_always" },
                    { "optionId": "allow-once", "name": "Yes", "kind": "allow_once" },
                    { "optionId": "reject-once", "name": "No", "kind": "reject_once" }
                ] }
        })
        .to_string()
    }

    fn decision(autonomy: Autonomy, web: bool, kind: &str, tool: Value) -> String {
        let reply = sent(&driver(autonomy, web).line(&permission(kind, tool, 0)));
        assert_eq!(reply["id"], 0);
        reply["result"]["outcome"]["optionId"].as_str().unwrap().to_string()
    }

    /// Against the real CLI, logged in: `cargo test acp_live -- --ignored
    /// --nocapture`. A Read agent is asked to write a file and to answer: the
    /// answer comes, the file doesn't.
    #[test]
    #[ignore]
    fn acp_live_a_read_agent_answers_and_cannot_write() {
        use crate::agents::events::{EventKind, Parser};
        use crate::agents::spec::Engine;
        use crate::marketplace::{agent_cli, model::AgentTarget};
        use std::io::{BufRead, BufReader, Write};
        use std::process::{Command, Stdio};

        let bin = agent_cli::locate(AgentTarget::Grok, None).expect("grok is installed");
        let dir = std::env::temp_dir().join(format!("termo-acp-live-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let run = AcpRun {
            prompt: "Create a file named a.txt containing the word hi. Whether or not that works, end your reply with the word listo."
                .into(),
            mcp_servers: Vec::new(),
            autonomy: Autonomy::Read,
            web: false,
            sign_in: true,
        };
        let mut child = Command::new(&bin)
            .args(["agent", "--no-leader", "stdio"])
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = Some(child.stdin.take().unwrap());
        let mut driver = Driver::new(run, dir.display().to_string());
        writeln!(stdin.as_mut().unwrap(), "{}", driver.start()).unwrap();
        let mut parser = Parser::new(Engine::Grok);
        let (mut answer, mut failed, mut cost) = (String::new(), false, None);
        for line in BufReader::new(child.stdout.take().unwrap()).lines().map_while(Result::ok) {
            if line.contains("turn_completed") {
                println!("<- {}", line.chars().skip(150).take(330).collect::<String>());
            }
            let (events, outcome) = parser.line(&line, 0);
            failed |= outcome.failed;
            cost = outcome.cost_usd.or(cost);
            for event in events {
                println!("{:?} {} {}", event.kind, event.tool, event.text);
                if event.kind == EventKind::Result {
                    answer = event.text;
                }
            }
            let step = driver.line(&line);
            for reply in &step.send {
                if reply.contains("\"outcome\"") || reply.contains("session/prompt") {
                    println!("-> {}", reply.chars().take(240).collect::<String>());
                }
                writeln!(stdin.as_mut().unwrap(), "{reply}").unwrap();
            }
            if step.done {
                stdin = None;
            }
        }
        let status = child.wait().unwrap();
        let written = dir.join("a.txt").exists();
        let _ = std::fs::remove_dir_all(&dir);
        println!("exit {status:?}, cost {cost:?}, model {}", parser.model);
        assert!(status.success() && !failed);
        assert!(answer.to_lowercase().contains("listo"), "{answer}");
        assert!(!written, "a Read agent wrote a file");
    }

    #[test]
    fn opens_a_session_with_the_runs_servers_and_sends_the_prompt() {
        let mut driver = driver(Autonomy::Read, false);
        assert_eq!(serde_json::from_str::<Value>(&driver.start()).unwrap()["method"], "initialize");

        let auth = sent(&driver.line(
            r#"{"jsonrpc":"2.0","id":1,"result":{"authMethods":[{"id":"cached_token"},{"id":"grok.com"}],"_meta":{"defaultAuthMethodId":"cached_token"}}}"#,
        ));
        assert_eq!((auth["method"].as_str(), auth["params"]["methodId"].as_str()), (Some("authenticate"), Some("cached_token")));

        let session = sent(&driver.line(r#"{"jsonrpc":"2.0","id":2,"result":{}}"#));
        assert_eq!(session["method"], "session/new");
        assert_eq!(session["params"]["cwd"], "C:/repo");
        assert_eq!(session["params"]["mcpServers"][0]["name"], "termo");

        let prompt = sent(&driver.line(r#"{"jsonrpc":"2.0","id":3,"result":{"sessionId":"s1"}}"#));
        assert_eq!(prompt["method"], "session/prompt");
        assert_eq!(prompt["params"]["sessionId"], "s1");
        assert_eq!(prompt["params"]["prompt"][0]["text"], "Resumí los cambios");

        // What it says meanwhile asks nothing of Termo.
        let chunk = r#"{"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk"}}}"#;
        assert_eq!(driver.line(chunk), Step::default());
        assert!(driver.line(r#"{"jsonrpc":"2.0","id":4,"result":{"stopReason":"end_turn"}}"#).done);
    }

    #[test]
    fn a_turn_cut_short_by_a_refusal_is_sent_on_a_few_times() {
        let mut driver = driver(Autonomy::Read, false);
        driver.line(r#"{"jsonrpc":"2.0","id":3,"result":{"sessionId":"s1"}}"#);
        let cancelled = |id: u64| format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{"stopReason":"cancelled"}}}}"#);
        for turn in 0..u64::from(MAX_REFUSED_TURNS) {
            driver.line(&permission("edit", json!({ "namespace": "grok_build" }), turn));
            let next = sent(&driver.line(&cancelled(PROMPT + turn)));
            assert_eq!((next["method"].as_str(), next["id"].as_u64()), (Some("session/prompt"), Some(PROMPT + turn + 1)));
            assert_eq!(next["params"]["sessionId"], "s1");
            assert!(next["params"]["prompt"][0]["text"].as_str().unwrap().contains("Edit `a.txt`"));
        }
        // Enough: the run ends there.
        driver.line(&permission("edit", json!({ "namespace": "grok_build" }), 9));
        assert!(driver.line(&cancelled(PROMPT + u64::from(MAX_REFUSED_TURNS))).done);
    }

    #[test]
    fn a_turn_cut_short_by_anything_else_ends_the_run() {
        let mut driver = driver(Autonomy::Run, false);
        driver.line(r#"{"jsonrpc":"2.0","id":3,"result":{"sessionId":"s1"}}"#);
        assert!(driver.line(r#"{"jsonrpc":"2.0","id":4,"result":{"stopReason":"cancelled"}}"#).done);
    }

    #[test]
    fn a_login_it_has_no_method_for_goes_straight_to_the_session() {
        let mut driver = driver(Autonomy::Read, false);
        assert_eq!(sent(&driver.line(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#))["method"], "session/new");
    }

    /// Gemini CLI offers a Google login first even to whoever uses a key:
    /// it isn't asked to log in at all, and keeps the login it has.
    #[test]
    fn without_sign_in_the_methods_offered_are_left_alone() {
        let mut driver = driver(Autonomy::Read, false);
        driver.run.sign_in = false;
        let offered = r#"{"jsonrpc":"2.0","id":1,"result":{"authMethods":[{"id":"oauth-personal"},{"id":"gemini-api-key"}]}}"#;
        assert_eq!(sent(&driver.line(offered))["method"], "session/new");
    }

    #[test]
    fn a_refused_step_ends_the_conversation() {
        let mut driver = driver(Autonomy::Read, false);
        let refused = driver.line(r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"not logged in"}}"#);
        assert_eq!(refused, Step { send: Vec::new(), done: true });
    }

    #[test]
    fn each_autonomy_level_grants_what_it_covers_once() {
        let built_in = json!({ "namespace": "grok_build", "read_only": false });
        assert_eq!(decision(Autonomy::Read, false, "edit", built_in.clone()), "reject-once");
        assert_eq!(decision(Autonomy::Read, false, "execute", built_in.clone()), "reject-once");
        assert_eq!(decision(Autonomy::Read, false, "read", built_in.clone()), "allow-once");
        assert_eq!(decision(Autonomy::Edit, false, "edit", built_in.clone()), "allow-once");
        assert_eq!(decision(Autonomy::Edit, false, "delete", built_in.clone()), "allow-once");
        assert_eq!(decision(Autonomy::Edit, false, "execute", built_in.clone()), "reject-once");
        assert_eq!(decision(Autonomy::Run, false, "execute", built_in.clone()), "allow-once");
        // The web is its own switch, whatever the level.
        assert_eq!(decision(Autonomy::Run, false, "fetch", built_in.clone()), "reject-once");
        assert_eq!(decision(Autonomy::Read, true, "fetch", built_in), "allow-once");
    }

    #[test]
    fn a_kind_it_doesnt_know_is_an_edit_for_groks_tools_and_free_for_the_agents_servers() {
        assert_eq!(decision(Autonomy::Read, false, "other", json!({ "namespace": "grok_build", "read_only": false })), "reject-once");
        assert_eq!(decision(Autonomy::Read, false, "other", json!({ "namespace": "grok_build", "read_only": true })), "allow-once");
        assert_eq!(decision(Autonomy::Edit, false, "other", json!({ "namespace": "grok_build", "read_only": false })), "allow-once");
        assert_eq!(decision(Autonomy::Read, false, "other", json!({ "namespace": "mcp" })), "allow-once");
    }

    #[test]
    fn with_no_option_that_says_it_the_call_is_dropped_and_other_requests_are_refused() {
        let mut driver = driver(Autonomy::Read, false);
        let only_allow = json!({
            "jsonrpc": "2.0", "id": 7, "method": "session/request_permission",
            "params": { "toolCall": { "kind": "execute" }, "options": [{ "optionId": "yes", "kind": "allow_once" }] }
        });
        assert_eq!(sent(&driver.line(&only_allow.to_string()))["result"]["outcome"]["outcome"], "cancelled");
        let read_file = r#"{"jsonrpc":"2.0","id":8,"method":"fs/read_text_file","params":{"path":"x"}}"#;
        assert_eq!(sent(&driver.line(read_file))["error"]["code"], -32601);
    }
}
