//! The wizard's "Build it with AI": Claude Code, asked through its own CLI
//! for a structured answer (`--json-schema`), turns the user's plain
//! description into a first draft of every step — name, task, logic,
//! autonomy, tools, schedule — that the user then reviews.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::i18n::{self, tr, Lang, Msg};
use crate::marketplace::agent_cli;
use crate::marketplace::model::AgentTarget;

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
Each agent runs unattended through Claude Code in print mode (`claude -p`): it gets a task \
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

pub fn draft(scratch: &Path, description: &str, work_dir: &str) -> Result<Draft, String> {
    let answer = ask_claude(scratch, prompt(description, work_dir, i18n::current()), &schema(), TIMEOUT)
        .map_err(|err| tr(Msg::AgentDraftFailed(&err)))?;
    serde_json::from_value(answer).map_err(|err| tr(Msg::AgentDraftFailed(&err.to_string())))
}

/// One structured answer from Claude Code: `text` as the prompt, the answer
/// shaped by `schema`. Errors come untranslated, for the caller to word.
pub fn ask_claude(scratch: &Path, text: String, schema: &serde_json::Value, timeout: Duration) -> Result<serde_json::Value, String> {
    let bin = agent_cli::require(AgentTarget::Claude, None)?;
    std::fs::create_dir_all(scratch).map_err(|err| err.to_string())?;
    let mut cmd = Command::new(bin);
    // The prompt (the user's description, several lines) goes through stdin:
    // see spec::Invocation for why no free text rides on the command line.
    cmd.args([
        "-p",
        "--output-format",
        "json",
        "--json-schema",
        &schema.to_string(),
        // A single answer: no tools to wait on and no servers to start.
        "--permission-mode",
        "dontAsk",
        "--strict-mcp-config",
        "--model",
        "sonnet",
    ])
    // An empty folder, so there's nothing around for it to go looking at.
    .current_dir(scratch)
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
    let mut stdout = child.stdout.take().expect("piped");
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("timeout".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    parse_structured(&reader.join().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

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
