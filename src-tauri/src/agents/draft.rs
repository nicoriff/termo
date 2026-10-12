//! The wizard's "Build it with AI": an agent CLI, asked for a structured
//! answer, turns the user's plain description into a first draft of every
//! step — name, task, logic, autonomy, tools, schedule — that the user then
//! reviews.
//!
//! Asking is here too, for everything in Termo that needs an answer in a
//! given shape (this draft, a team's, an orchestration's plan and its
//! supervision): `ask_streaming`. Any engine can answer. Claude Code and
//! Codex hold the model to the schema themselves (`--json-schema`,
//! `--output-schema`); Gemini CLI, Copilot CLI and Grok are given it in the
//! prompt, and what they say is checked and asked for again once if it
//! doesn't fit.

use super::events::describe_input;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::i18n::{self, tr, Lang, Msg};
use crate::marketplace::agent_cli;
use crate::marketplace::model::AgentTarget;

use super::events::{EventKind, Parser};
use super::spec::{
    build_invocation, claude_allowed_tools, claude_settings_json, is_valid_model, AgentSpec, Autonomy, Engine, GIT_SAFETY_ENV,
};

/// Drafting is a single answer, no tools: anything longer has gone wrong.
const TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DraftSchedule {
    /// "none" | "interval" | "daily"
    pub kind: String,
    #[serde(default)]
    pub minutes: u32,
    #[serde(default)]
    pub at: String,
    #[serde(default)]
    pub weekdays: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DraftIntegration {
    /// What to search the MCP registry for.
    pub query: String,
    pub why: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub name: String,
    pub icon: String,
    pub task: String,
    pub instructions: String,
    /// "read" | "edit" | "run"
    pub autonomy: String,
    pub web: bool,
    pub schedule: DraftSchedule,
    pub integrations: Vec<DraftIntegration>,
    /// A few sentences on why it's built this way, shown to the user.
    pub explanation: String,
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "icon": { "type": "string" },
            "task": { "type": "string" },
            "instructions": { "type": "string" },
            "autonomy": { "type": "string", "enum": ["read", "edit", "run"] },
            "web": { "type": "boolean" },
            "schedule": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["none", "interval", "daily"] },
                    "minutes": { "type": "integer" },
                    "at": { "type": "string" },
                    "weekdays": { "type": "array", "items": { "type": "integer" } }
                },
                "required": ["kind", "minutes", "at", "weekdays"]
            },
            "integrations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": { "query": { "type": "string" }, "why": { "type": "string" } },
                    "required": ["query", "why"]
                }
            },
            "explanation": { "type": "string" }
        },
        "required": ["name", "icon", "task", "instructions", "autonomy", "web", "schedule", "integrations", "explanation"]
    })
}

pub fn prompt(description: &str, work_dir: &str, lang: Lang) -> String {
    let language = match lang {
        Lang::Es => "Spanish (Rioplatense, using \"vos\")",
        Lang::En => "English",
    };
    let folder = if work_dir.trim().is_empty() {
        "a private, empty folder of its own".to_string()
    } else {
        format!("the folder {}", work_dir.trim())
    };
    format!(
        "You design small autonomous agents for people with no experience building agents. \
Each agent runs unattended through an agent CLI in headless mode (Claude Code, Codex and the like): it gets a task \
prompt plus instructions appended to its system prompt, works in {folder}, can never ask the \
user anything mid-run, and whatever its permission level doesn't allow is refused.

Turn the user's description below into a draft agent:
- name: short (2-4 words).
- icon: one emoji that fits.
- task: the concrete request it receives on every run, in the imperative, one or two sentences.
- instructions: its logic, as plain text with three short sections — Steps (a numbered \
list), Rules (bullets: what it must and must never do; always include not asking questions \
and keeping the final answer short and readable), and Done when (one line). Write the section \
headings in the same language as the rest.
- autonomy: the LEAST it needs. \"read\" looks at files and can run git commands that only \
read (log, diff, status, show, blame, branch) — enough for reviews and summaries of a repo; \
\"edit\" also creates and changes files; \"run\" also runs any shell command (git pull or \
commit, scripts, moving files). Pick \"run\" only when nothing else can do the job.
- web: true only if it needs current information from the internet.
- schedule: \"none\" unless the description implies it runs by itself; \"interval\" with \
minutes, or \"daily\" with at as HH:MM (24 h) and weekdays (0 = Sunday; empty = every day). \
Fill unused fields with 0, \"\" and [].
- integrations: external services it needs beyond files and the web (GitHub issues, Slack, a \
database…), each as a short search query for the MCP server registry plus why; empty if none.
- explanation: 2-3 sentences for the user on the choices you made, especially the permission \
level.

Write every text field in {language}.

The user's description:
\"\"\"
{description}
\"\"\""
    )
}

/// Reads Claude's `--output-format json` answer.
#[cfg(test)]
pub fn parse_answer(stdout: &str) -> Result<Draft, String> {
    serde_json::from_value(parse_structured(stdout)?).map_err(|err| err.to_string())
}

/// The structured part of Claude's `--output-format json` answer.
pub fn parse_structured(stdout: &str) -> Result<serde_json::Value, String> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).map_err(|err| err.to_string())?;
    if value.get("is_error").and_then(|flag| flag.as_bool()) == Some(true) {
        let detail = value.get("result").and_then(|text| text.as_str()).unwrap_or("error");
        return Err(detail.to_string());
    }
    let structured = value
        .get("structured_output")
        .cloned()
        .or_else(|| {
            // Older CLIs put the JSON in the text result instead.
            value
                .get("result")
                .and_then(|text| text.as_str())
                .and_then(|text| serde_json::from_str(text).ok())
        })
        .ok_or_else(|| "no structured output".to_string())?;
    Ok(structured)
}

pub fn draft(scratch: &Path, engine: Engine, description: &str, work_dir: &str) -> Result<Draft, String> {
    let answer = ask(engine, scratch, prompt(description, work_dir, i18n::current()), &schema(), TIMEOUT)
        .map_err(|err| tr(Msg::AgentDraftFailed(&err)))?;
    serde_json::from_value(answer).map_err(|err| tr(Msg::AgentDraftFailed(&err.to_string())))
}

/// One structured answer from `engine`'s CLI, with nothing around to look
/// at: `text` as the prompt, the answer shaped by `schema`. Errors come
/// untranslated, for the caller to word.
pub fn ask(
    engine: Engine,
    scratch: &Path,
    text: String,
    schema: &serde_json::Value,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    // Claude Code's mid model is plenty for a draft; the others' default.
    let model = if engine == Engine::Claude { "sonnet" } else { "" };
    let ask = Ask { engine, dir: scratch, model, read_files: false, timeout };
    ask_with(&ask, text, schema).map(|answer| answer.value)
}

/// How one structured answer is asked for.
pub struct Ask<'a> {
    /// Whose CLI answers.
    pub engine: Engine,
    /// Where the CLI runs. Without `read_files` it's an empty folder, so
    /// there's nothing around for it to go looking at.
    pub dir: &'a Path,
    /// Empty: the CLI's default.
    pub model: &'a str,
    /// Lets it look at `dir` before answering — read, search, and the git
    /// commands that only read, the same an agent that can "only look" gets —
    /// and nothing outside it.
    pub read_files: bool,
    pub timeout: Duration,
}

pub struct Answer {
    pub value: serde_json::Value,
    /// What the answer cost, as the CLI reports it.
    pub cost_usd: Option<f64>,
    /// The model that answered, where the CLI says: Claude Code's only.
    pub model: Option<String>,
}

/// A step of an answer being worked out, as it happens: for showing
/// progress while the CLI looks around and thinks.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Step {
    /// It used a tool: `name`, and what on (a file, a pattern, a command).
    Tool { name: String, detail: String },
    /// Something it said along the way.
    Text { text: String },
    /// Its reasoning, when the model shows it.
    Thinking { text: String },
}

/// The steps in one line of Claude Code's `stream-json` output. The tool it
/// answers with (`StructuredOutput`) isn't a step: it's the answer.
pub fn steps_in(line: &str) -> Vec<Step> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else { return Vec::new() };
    if value.get("type").and_then(|kind| kind.as_str()) != Some("assistant") {
        return Vec::new();
    }
    let content = value.pointer("/message/content").and_then(|content| content.as_array());
    content
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let text = |key: &str| item.get(key).and_then(|text| text.as_str()).unwrap_or("").trim().to_string();
            match item.get("type").and_then(|kind| kind.as_str())? {
                "tool_use" => {
                    let name = text("name");
                    (name != "StructuredOutput" && !name.is_empty()).then(|| Step::Tool {
                        detail: item.get("input").map(describe_input).unwrap_or_default(),
                        name,
                    })
                }
                "text" => Some(Step::Text { text: text("text") }).filter(|_| !text("text").is_empty()),
                "thinking" => Some(Step::Thinking { text: text("thinking") }).filter(|_| !text("thinking").is_empty()),
                _ => None,
            }
        })
        .collect()
}

/// The last `result` line of a `stream-json` output: it carries what
/// `--output-format json` would (the structured answer, its cost).
fn result_line(stdout: &str) -> Option<&str> {
    stdout.lines().rev().find(|line| {
        serde_json::from_str::<serde_json::Value>(line.trim())
            .ok()
            .is_some_and(|value| value.get("type").and_then(|kind| kind.as_str()) == Some("result"))
    })
}

/// What the CLI says its answer cost.
pub fn parse_cost(stdout: &str) -> Option<f64> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;
    value.get("total_cost_usd").and_then(|cost| cost.as_f64())
}

/// The model behind Claude Code's `stream-json` output: the one its last
/// message came from, which is the one that wrote the answer.
pub fn parse_model(stdout: &str) -> Option<String> {
    stdout.lines().rev().find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        if value.get("type").and_then(|kind| kind.as_str()) != Some("assistant") {
            return None;
        }
        let model = value.pointer("/message/model")?.as_str()?;
        // What Claude Code answers itself with when the API fails.
        (model != "<synthetic>").then(|| model.to_string())
    })
}

pub fn ask_with(ask: &Ask, text: String, schema: &serde_json::Value) -> Result<Answer, String> {
    ask_streaming(ask, text, schema, &|_| {})
}

/// The same, telling `on_step` what the CLI does on the way to its answer.
pub fn ask_streaming(ask: &Ask, text: String, schema: &serde_json::Value, on_step: &dyn Fn(Step)) -> Result<Answer, String> {
    // Drafting with AI is an AI feature too (see ai_mode.rs).
    if !crate::ai_mode::enabled() {
        return Err(crate::i18n::tr(crate::i18n::Msg::AiModeOff));
    }
    if ask.engine == Engine::Claude {
        return ask_claude(ask, text, schema, on_step);
    }
    // The others answer in text: what comes back is checked against the
    // schema's shape, and asked for once more when it doesn't fit.
    let mut cost: Option<f64> = None;
    let mut problem = String::new();
    for attempt in 0..2 {
        let said = run_reader(ask, &shaped_prompt(&text, schema, (attempt > 0).then_some(problem.as_str())), schema, on_step)?;
        cost = match (cost, said.cost_usd) {
            (Some(before), Some(now)) => Some(before + now),
            (before, now) => before.or(now),
        };
        match json_in(&said.text, schema) {
            Ok(value) => return Ok(Answer { value, cost_usd: cost, model: None }),
            Err(why) => problem = why,
        }
    }
    Err(problem)
}

/// The prompt for an engine that takes no schema of its own: the request,
/// then the shape the answer has to have, and what was wrong with the last
/// one when this is the second time of asking.
fn shaped_prompt(text: &str, schema: &serde_json::Value, problem: Option<&str>) -> String {
    let again = match problem {
        Some(problem) => format!("\n\nYour previous answer could not be used ({problem}). Answer again."),
        None => String::new(),
    };
    format!(
        "{text}\n\n---\nAnswer with one JSON object and nothing else — no explanation before or after it, no code fence — \
that matches this JSON Schema:\n{schema}{again}"
    )
}

/// The JSON object in what an engine said, with the keys the schema requires
/// at its top. What's inside is the caller's to read into its own types.
pub fn json_in(said: &str, schema: &serde_json::Value) -> Result<serde_json::Value, String> {
    let said = said.trim();
    let whole = serde_json::from_str::<serde_json::Value>(said).ok();
    // Around it, some still put a sentence or a code fence.
    let inside = || {
        let (start, end) = (said.find('{')?, said.rfind('}')?);
        serde_json::from_str::<serde_json::Value>(said.get(start..=end)?).ok()
    };
    let value = whole.filter(serde_json::Value::is_object).or_else(inside).ok_or("the answer has no JSON object in it")?;
    let required = schema.get("required").and_then(|keys| keys.as_array()).into_iter().flatten();
    for key in required.filter_map(|key| key.as_str()) {
        if value.get(key).is_none() {
            return Err(format!("the answer is missing \"{key}\""));
        }
    }
    Ok(value)
}

/// A schema as Codex's `--output-schema` takes it: every object closed, and
/// every one of its properties required (the caller's types fill in the
/// rest by default anyway).
pub fn strict_schema(schema: &serde_json::Value) -> serde_json::Value {
    let mut schema = schema.clone();
    fn close(node: &mut serde_json::Value) {
        let Some(object) = node.as_object_mut() else { return };
        if let Some(properties) = object.get("properties").and_then(|properties| properties.as_object()) {
            let keys: Vec<serde_json::Value> = properties.keys().cloned().map(serde_json::Value::String).collect();
            object.insert("required".into(), serde_json::Value::Array(keys));
            object.insert("additionalProperties".into(), serde_json::Value::Bool(false));
        }
        for (_, child) in object.iter_mut() {
            match child {
                serde_json::Value::Object(children) if children.get("type").is_none() && children.get("properties").is_none() => {
                    // A map of named schemas ("properties" itself).
                    children.values_mut().for_each(close);
                }
                serde_json::Value::Object(_) => close(child),
                serde_json::Value::Array(items) => items.iter_mut().for_each(close),
                _ => {}
            }
        }
    }
    close(&mut schema);
    schema
}

/// What an engine said in a run that only looks, and what it cost when the
/// CLI says.
struct Said {
    text: String,
    cost_usd: Option<f64>,
}

/// How long an ACP agent gets to leave once its turn is over (see runner.rs).
const ACP_EXIT_GRACE: Duration = Duration::from_secs(20);

/// Runs `ask.engine`'s CLI once as an agent that may only look (the same
/// invocation an agent of the user's at that level gets), with `prompt`,
/// and gathers its answer. Its steps go to `on_step` as they happen.
fn run_reader(ask: &Ask, prompt: &str, schema: &serde_json::Value, on_step: &dyn Fn(Step)) -> Result<Said, String> {
    let bin = agent_cli::require(super::runner::target(ask.engine), None)?;
    if !ask.read_files {
        std::fs::create_dir_all(ask.dir).map_err(|err| err.to_string())?;
    }
    let reader = AgentSpec {
        engine: ask.engine,
        autonomy: Autonomy::Read,
        model: ask.model.trim().to_string(),
        task: prompt.to_string(),
        work_dir: ask.dir.display().to_string(),
        ..AgentSpec::default()
    };
    let invocation = build_invocation(&reader, None, None, &std::collections::HashMap::new());
    let mut args = invocation.args.clone();
    // Files for this one answer, out of the folder it may be reading.
    let scratch = std::env::temp_dir();
    let id = uuid::Uuid::new_v4();
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut cmd = Command::new(&bin);
    if ask.engine == Engine::Codex {
        let file = scratch.join(format!("termo-schema-{id}.json"));
        std::fs::write(&file, strict_schema(schema).to_string()).map_err(|err| err.to_string())?;
        args.push("--output-schema".into());
        args.push(file.display().to_string());
        files.push(file);
    }
    if let (Engine::Gemini, Some(settings)) = (ask.engine, &invocation.settings) {
        // Its settings for the run, on top of the system's (see runner.rs).
        let file = scratch.join(format!("termo-gemini-{id}.json"));
        super::console_bridge::write_private(&file, &super::gemini_system::with_system(settings)).map_err(|err| err.to_string())?;
        cmd.env("GEMINI_CLI_SYSTEM_SETTINGS_PATH", &file);
        files.push(file);
    }
    if ask.engine == Engine::Gemini {
        // Headless, Gemini CLI refuses a folder it hasn't been told to trust.
        // This one only looks, and the folder is the one the user picked.
        cmd.env("GEMINI_CLI_TRUST_WORKSPACE", "true");
    }
    cmd.args(&args)
        .current_dir(ask.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Never read: piped, a chatty stderr would fill up and stall the CLI.
        .stderr(Stdio::null())
        .env("NO_COLOR", "1");
    for key in super::runner::INHERITED_NOISE {
        cmd.env_remove(key);
    }
    cmd.envs(invocation.env.iter().map(|(name, value)| (name, value)));
    cmd.envs(GIT_SAFETY_ENV.iter().copied());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let cleanup = |files: &[std::path::PathBuf]| files.iter().for_each(|file| drop(std::fs::remove_file(file)));
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            cleanup(&files);
            return Err(err.to_string());
        }
    };
    // Grok's is a conversation (see acp.rs): its input stays open until the
    // turn is over. The others take the prompt and are left to it.
    let mut conversation = None;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        match invocation.acp {
            Some(run) => {
                let driver = super::acp::Driver::new(run, ask.dir.display().to_string());
                let _ = writeln!(stdin, "{}", driver.start());
                conversation = Some((driver, stdin));
            }
            None => {
                let text = invocation.stdin;
                std::thread::spawn(move || {
                    let _ = stdin.write_all(text.as_bytes());
                });
            }
        }
    }
    let stdout = child.stdout.take().expect("piped");
    let (lines, received) = std::sync::mpsc::channel::<String>();
    let reading = std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                break;
            }
        }
    });

    let mut parser = Parser::new(ask.engine);
    let mut said = Said { text: String::new(), cost_usd: None };
    let mut failed = false;
    let mut errors: Vec<String> = Vec::new();
    let mut turn_over: Option<Instant> = None;
    let mut take = |events: Vec<super::events::RunEvent>, cost: Option<f64>, line_failed: bool| {
        failed |= line_failed;
        said.cost_usd = cost.or(said.cost_usd);
        for event in events {
            match event.kind {
                EventKind::Tool => on_step(Step::Tool { name: event.tool, detail: event.text }),
                // The last thing it says is the answer; on the way, a step.
                EventKind::Text | EventKind::Result if !event.text.trim().is_empty() => {
                    on_step(Step::Text { text: event.text.clone() });
                    said.text = event.text;
                }
                EventKind::Stderr if !event.text.trim().is_empty() => errors.push(event.text),
                _ => {}
            }
        }
    };
    let deadline = Instant::now() + ask.timeout;
    let timed_out = loop {
        while let Ok(line) = received.try_recv() {
            let (events, outcome) = parser.line(&line, 0);
            take(events, outcome.cost_usd, outcome.failed);
            if let Some((driver, stdin)) = conversation.as_mut() {
                use std::io::Write;
                let step = driver.line(&line);
                for reply in &step.send {
                    let _ = writeln!(stdin, "{reply}");
                }
                if step.done {
                    conversation = None;
                    turn_over = Some(Instant::now());
                }
            }
        }
        if child.try_wait().ok().flatten().is_some() {
            break false;
        }
        if turn_over.is_some_and(|since| since.elapsed() >= ACP_EXIT_GRACE) {
            super::runner::kill_tree(&mut child);
            let _ = child.wait();
            break false;
        }
        if Instant::now() >= deadline {
            super::runner::kill_tree(&mut child);
            let _ = child.wait();
            break true;
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    let _ = reading.join();
    while let Ok(line) = received.try_recv() {
        let (events, outcome) = parser.line(&line, 0);
        take(events, outcome.cost_usd, outcome.failed);
    }
    take(parser.finish(0), None, false);
    cleanup(&files);
    if timed_out {
        return Err("timeout".into());
    }
    if failed || said.text.trim().is_empty() {
        return Err(if errors.is_empty() { "no answer".to_string() } else { errors.join("\n") });
    }
    Ok(said)
}

/// Claude Code's own way: the schema is its to hold the answer to, and the
/// answer comes apart from what it says.
fn ask_claude(ask: &Ask, text: String, schema: &serde_json::Value, on_step: &dyn Fn(Step)) -> Result<Answer, String> {
    let bin = agent_cli::require(AgentTarget::Claude, None)?;
    if !ask.read_files {
        std::fs::create_dir_all(ask.dir).map_err(|err| err.to_string())?;
    }
    let mut cmd = Command::new(bin);
    // The prompt (the user's description, several lines) goes through stdin:
    // see spec::Invocation for why no free text rides on the command line.
    cmd.args([
        "-p",
        // One JSON object per line as it works, ending with the same result
        // `json` would give (see result_line).
        "--output-format",
        "stream-json",
        "--verbose",
        "--json-schema",
        &schema.to_string(),
        // Nobody is there to approve anything: what isn't allowed is denied.
        "--permission-mode",
        "dontAsk",
        // No servers to start, and none of the user's own tools.
        "--strict-mcp-config",
    ]);
    if ask.read_files {
        let reader = AgentSpec { autonomy: Autonomy::Read, ..AgentSpec::default() };
        cmd.args(["--allowedTools", &claude_allowed_tools(&reader).join(","), "--settings", &claude_settings_json(&reader)]);
        cmd.envs(GIT_SAFETY_ENV.iter().copied());
    }
    if !ask.model.trim().is_empty() && is_valid_model(ask.model) {
        cmd.args(["--model", ask.model.trim()]);
    }
    cmd.current_dir(ask.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Never read: piped, a chatty stderr would fill up and stall the CLI.
        .stderr(Stdio::null())
        .env("NO_COLOR", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|err| err.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    let stdout = child.stdout.take().expect("piped");
    // Lines come through a channel, so the steps are told here, on the
    // caller's thread, while this waits.
    let (lines, received) = std::sync::mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                break;
            }
        }
    });
    let mut output = String::new();
    let take = |line: String, output: &mut String| {
        for step in steps_in(&line) {
            on_step(step);
        }
        output.push_str(&line);
        output.push('\n');
    };
    let deadline = Instant::now() + ask.timeout;
    loop {
        while let Ok(line) = received.try_recv() {
            take(line, &mut output);
        }
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("timeout".into());
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    let _ = reader.join();
    while let Ok(line) = received.try_recv() {
        take(line, &mut output);
    }
    let result = result_line(&output).ok_or_else(|| "no result".to_string())?;
    Ok(Answer { value: parse_structured(result)?, cost_usd: parse_cost(result), model: parse_model(&output) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_json_an_engine_answered_with_whatever_is_around_it() {
        let schema = serde_json::json!({ "type": "object", "properties": { "name": {}, "task": {} }, "required": ["name"] });
        assert_eq!(json_in(r#"{"name":"a"}"#, &schema).unwrap()["name"], "a");
        let fenced = "Here it is:\n```json\n{\"name\": \"b\", \"task\": \"x {y}\"}\n```\nDone.";
        assert_eq!(json_in(fenced, &schema).unwrap()["task"], "x {y}");
        assert!(json_in("I could not do that.", &schema).unwrap_err().contains("no JSON object"));
        assert!(json_in(r#"{"task":"x"}"#, &schema).unwrap_err().contains("\"name\""));
        // A list isn't an answer, though it's JSON.
        assert!(json_in("[1, 2]", &schema).is_err());
    }

    #[test]
    fn the_model_that_answered_is_the_last_messages() {
        let output = [
            r#"{"type":"system","subtype":"init","model":"claude-opus-5-5"}"#,
            r#"{"type":"assistant","message":{"model":"claude-haiku-5-5-20260801","content":[]}}"#,
            r#"{"type":"assistant","message":{"model":"<synthetic>","content":[]}}"#,
            r#"{"type":"result","total_cost_usd":0.002}"#,
            "not json",
        ]
        .join("\n");
        assert_eq!(parse_model(&output).as_deref(), Some("claude-haiku-5-5-20260801"));
        assert_eq!(parse_model(r#"{"type":"result"}"#), None);
    }

    #[test]
    fn the_second_time_of_asking_says_what_was_wrong() {
        let schema = serde_json::json!({ "type": "object" });
        let first = shaped_prompt("Plan it", &schema, None);
        assert!(first.starts_with("Plan it") && first.contains(r#"{"type":"object"}"#) && !first.contains("previous answer"));
        assert!(shaped_prompt("Plan it", &schema, Some("the answer is missing \"name\"")).contains("missing \"name\""));
    }

    #[test]
    fn a_schema_for_codex_closes_every_object_and_requires_all_of_it() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "agents": { "type": "array", "items": { "type": "object", "properties": { "key": { "type": "string" }, "web": { "type": "boolean" } }, "required": ["key"] } }
            },
            "required": ["name"]
        });
        let strict = strict_schema(&schema);
        assert_eq!(strict["required"], serde_json::json!(["name", "agents"]));
        assert_eq!(strict["additionalProperties"], false);
        let item = &strict["properties"]["agents"]["items"];
        assert_eq!(item["required"], serde_json::json!(["key", "web"]));
        assert_eq!(item["additionalProperties"], false);
        // A property is a schema, not an object to close.
        assert!(strict["properties"]["name"].get("additionalProperties").is_none());
    }

    /// Against the real CLIs, logged in: `cargo test ask_live -- --ignored
    /// --nocapture`. Each one installed answers in the shape asked for.
    #[test]
    #[ignore]
    fn ask_live_every_installed_engine_answers_in_shape() {
        crate::ai_mode::set_enabled_for_tests(true);
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "capital": { "type": "string" },
                "facts": { "type": "array", "items": { "type": "object", "properties": { "about": { "type": "string" } }, "required": ["about"] } }
            },
            "required": ["capital", "facts"]
        });
        let dir = std::env::temp_dir().join(format!("termo-ask-live-{}", uuid::Uuid::new_v4()));
        for engine in [Engine::Codex, Engine::Gemini, Engine::Copilot, Engine::Grok] {
            if agent_cli::locate(crate::agents::runner::target(engine), None).is_none() {
                println!("{engine:?}: not installed, skipped");
                continue;
            }
            let ask = Ask { engine, dir: &dir, model: "", read_files: false, timeout: Duration::from_secs(240) };
            let steps = std::cell::Cell::new(0);
            let answer = ask_streaming(&ask, "What is the capital of France? Give two short facts about it.".into(), &schema, &|_| {
                steps.set(steps.get() + 1)
            });
            match answer {
                Ok(answer) => println!("{engine:?}: {} ({} steps, cost {:?})", answer.value, steps.get(), answer.cost_usd),
                Err(err) => println!("{engine:?}: FAILED {err}"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What the CLI does on the way is told as steps; its answer isn't one.
    #[test]
    fn streamed_lines_become_steps_and_the_result_is_found() {
        let read = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"Veamos la estructura"},{"type":"tool_use","name":"Read","input":{"file_path":"src/main.rs"}}]}}"#;
        assert_eq!(
            steps_in(read),
            vec![
                Step::Thinking { text: "Veamos la estructura".into() },
                Step::Tool { name: "Read".into(), detail: "src/main.rs".into() },
            ]
        );
        let answer = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"StructuredOutput","input":{}}]}}"#;
        assert!(steps_in(answer).is_empty());
        assert!(steps_in(r#"{"type":"rate_limit_event"}"#).is_empty());
        let output = format!("{read}\n{}\n", r#"{"type":"result","is_error":false,"structured_output":{"a":1},"total_cost_usd":0.5}"#);
        let result = result_line(&output).unwrap();
        assert_eq!(parse_structured(result).unwrap()["a"], 1);
        assert_eq!(parse_cost(result), Some(0.5));
    }

    #[test]
    fn the_prompt_carries_the_description_and_the_language() {
        let text = prompt("resumí mis commits", "C:\\repo", Lang::Es);
        assert!(text.contains("resumí mis commits"));
        assert!(text.contains("C:\\repo"));
        assert!(text.contains("Spanish"));
        assert!(prompt("x", "", Lang::En).contains("private, empty folder"));
    }

    #[test]
    fn reads_the_structured_answer() {
        let answer = serde_json::json!({
            "type": "result",
            "is_error": false,
            "structured_output": {
                "name": "Resumen", "icon": "📊", "task": "Resumí", "instructions": "Pasos",
                "autonomy": "read", "web": false,
                "schedule": { "kind": "daily", "minutes": 0, "at": "09:00", "weekdays": [] },
                "integrations": [], "explanation": "Solo lee."
            }
        });
        let draft = parse_answer(&answer.to_string()).unwrap();
        assert_eq!(draft.name, "Resumen");
        assert_eq!(draft.schedule.at, "09:00");
    }

    #[test]
    fn reads_what_the_answer_cost() {
        let answer = serde_json::json!({ "type": "result", "is_error": false, "total_cost_usd": 0.04, "structured_output": {} });
        assert_eq!(parse_cost(&answer.to_string()), Some(0.04));
        assert_eq!(parse_cost("{}"), None);
    }

    #[test]
    fn an_error_answer_is_an_error() {
        let answer = serde_json::json!({ "type": "result", "is_error": true, "result": "rate limited" });
        assert_eq!(parse_answer(&answer.to_string()).unwrap_err(), "rate limited");
        assert!(parse_answer("garbage").is_err());
    }

    #[test]
    fn the_schema_requires_every_field_the_wizard_fills() {
        let required = schema()["required"].as_array().unwrap().len();
        assert_eq!(required, 9);
    }
}
