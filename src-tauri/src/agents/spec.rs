//! What an agent is: the definition the wizard builds, and how it turns into
//! a run of the agent's own CLI (`claude -p` / `codex exec`).
//!
//! An agent here is configuration, not code. Everything it can do comes from
//! the CLI the user already has installed and signed in to; this file only
//! decides which of that CLI's flags express the user's choices.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    #[default]
    Claude,
    Codex,
}

/// How far the agent may go on its own, in the terms the wizard explains:
/// only look, also change files, or also run commands. Runs never stop to ask
/// for permission (nobody is there to answer), so whatever isn't allowed here
/// is refused outright.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Autonomy {
    #[default]
    Read,
    Edit,
    Run,
}

/// An MCP server the agent gets, in `.mcp.json` shape (`command`/`args`/`env`
/// or `url`/`headers`), under the name the agent will see it by.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServer {
    pub name: String,
    pub config: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Tools {
    /// Searching and reading the web.
    pub web: bool,
    pub mcp: Vec<McpServer>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Limits {
    /// A run still going after this long is stopped and marked as timed out.
    pub timeout_min: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self { timeout_min: 15 }
    }
}

/// When the agent runs by itself, besides "Run now".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Schedule {
    /// Every N minutes.
    Interval { minutes: u32 },
    /// At `at` ("HH:MM", local time) on the given weekdays (0 = Sunday); an
    /// empty list means every day.
    Daily {
        at: String,
        #[serde(default)]
        weekdays: Vec<u8>,
    },
}

/// Run when something changes inside a folder.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Watch {
    pub path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Triggers {
    pub schedule: Option<Schedule>,
    pub watch: Option<Watch>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GithubDeploy {
    /// `owner/repo` on GitHub, when known.
    pub repo: String,
    pub repo_path: String,
    pub workflow_file: String,
    pub deployed_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Deploy {
    pub github: Option<GithubDeploy>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSpec {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub engine: Engine,
    /// Empty: the CLI's own default.
    pub model: String,
    /// The folder the agent works in.
    pub work_dir: String,
    /// What to do on each run: the prompt.
    pub task: String,
    /// The agent's logic: the steps it follows, its rules, when it's done.
    pub instructions: String,
    pub autonomy: Autonomy,
    pub tools: Tools,
    pub limits: Limits,
    pub triggers: Triggers,
    /// Names of the secrets the agent's integrations need; the values live in
    /// the system keychain, never in this file.
    pub secrets: Vec<String>,
    /// Off: scheduled and watched runs are skipped; "Run now" still works.
    pub enabled: bool,
    pub deploy: Deploy,
    /// Unix seconds.
    pub created_at: i64,
    pub updated_at: i64,
}

impl Default for AgentSpec {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            icon: String::new(),
            engine: Engine::default(),
            model: String::new(),
            work_dir: String::new(),
            task: String::new(),
            instructions: String::new(),
            autonomy: Autonomy::default(),
            tools: Tools::default(),
            limits: Limits::default(),
            triggers: Triggers::default(),
            secrets: Vec::new(),
            enabled: true,
            deploy: Deploy::default(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

/// Ids name a folder on disk, so they're held to a safe alphabet.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// git commands that only look: "only look" agents (reviews, summaries)
/// need them, and none of them changes the repository.
const READ_ONLY_GIT: &[&str] = &["log", "diff", "status", "show", "blame", "branch", "rev-parse", "ls-files"];

/// Claude Code's tools for each autonomy level. Each level includes the one
/// before it.
pub fn claude_allowed_tools(spec: &AgentSpec) -> Vec<String> {
    let git: Vec<String> = READ_ONLY_GIT
        .iter()
        .flat_map(|command| [format!("Bash(git {command}:*)"), format!("PowerShell(git {command}:*)")])
        .collect();
    let mut tools: Vec<&str> = vec!["Read", "Glob", "Grep", "TodoWrite"];
    tools.extend(git.iter().map(String::as_str));
    if matches!(spec.autonomy, Autonomy::Edit | Autonomy::Run) {
        tools.extend(["Edit", "Write", "NotebookEdit"]);
    }
    if spec.autonomy == Autonomy::Run {
        tools.extend(["Bash", "PowerShell"]);
    }
    if spec.tools.web {
        tools.extend(["WebFetch", "WebSearch"]);
    }
    let mut tools: Vec<String> = tools.into_iter().map(String::from).collect();
    // Every tool of each of the agent's own MCP servers.
    tools.extend(spec.tools.mcp.iter().map(|server| format!("mcp__{}", server.name)));
    tools
}

/// `--mcp-config`'s JSON: `{"mcpServers": {name: config}}`.
pub fn mcp_config_json(spec: &AgentSpec) -> Option<String> {
    if spec.tools.mcp.is_empty() {
        return None;
    }
    let servers: serde_json::Map<String, serde_json::Value> = spec
        .tools
        .mcp
        .iter()
        .map(|server| (server.name.clone(), server.config.clone()))
        .collect();
    Some(serde_json::json!({ "mcpServers": servers }).to_string())
}

/// The arguments after the CLI's own path.
pub fn build_args(spec: &AgentSpec) -> Vec<String> {
    match spec.engine {
        Engine::Claude => claude_args(spec),
        Engine::Codex => codex_args(spec),
    }
}

fn claude_args(spec: &AgentSpec) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-p".into(),
        spec.task.clone(),
        "--output-format".into(),
        "stream-json".into(),
        // stream-json in print mode refuses to run without it.
        "--verbose".into(),
        // Nobody is watching a background run to approve anything: what
        // --allowedTools doesn't cover is denied instead of waiting forever.
        "--permission-mode".into(),
        "dontAsk".into(),
        "--allowedTools".into(),
        claude_allowed_tools(spec).join(","),
    ];
    if !spec.instructions.trim().is_empty() {
        args.push("--append-system-prompt".into());
        args.push(spec.instructions.clone());
    }
    if let Some(mcp) = mcp_config_json(spec) {
        args.push("--mcp-config".into());
        args.push(mcp);
        // Only its own servers: the user's global ones could hand it tools
        // nobody chose for it.
        args.push("--strict-mcp-config".into());
    }
    if !spec.model.trim().is_empty() {
        args.push("--model".into());
        args.push(spec.model.trim().into());
    }
    args
}

/// A TOML string literal, for `-c key=value` overrides.
fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

fn codex_args(spec: &AgentSpec) -> Vec<String> {
    let sandbox = match spec.autonomy {
        Autonomy::Read => "read-only",
        Autonomy::Edit | Autonomy::Run => "workspace-write",
    };
    let mut args: Vec<String> = vec![
        "exec".into(),
        "--json".into(),
        // An agent can work in any folder, repository or not.
        "--skip-git-repo-check".into(),
        "--sandbox".into(),
        sandbox.into(),
        "-c".into(),
        "approval_policy=\"never\"".into(),
    ];
    if !spec.work_dir.trim().is_empty() {
        args.push("-C".into());
        args.push(spec.work_dir.clone());
    }
    for server in &spec.tools.mcp {
        if let Some(object) = server.config.as_object() {
            for (key, value) in object {
                let value = match value {
                    serde_json::Value::String(text) => toml_string(text),
                    other => other.to_string(),
                };
                args.push("-c".into());
                args.push(format!("mcp_servers.{}.{key}={value}", server.name));
            }
        }
    }
    if !spec.model.trim().is_empty() {
        args.push("-m".into());
        args.push(spec.model.trim().into());
    }
    args.push(codex_prompt(spec));
    args
}

/// Codex has no separate system prompt flag, so the logic goes ahead of the
/// task in the prompt itself.
fn codex_prompt(spec: &AgentSpec) -> String {
    if spec.instructions.trim().is_empty() {
        return spec.task.clone();
    }
    format!("{}\n\n---\n\n{}", spec.instructions.trim(), spec.task)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(engine: Engine, autonomy: Autonomy) -> AgentSpec {
        AgentSpec {
            id: "a1".into(),
            engine,
            autonomy,
            task: "Resumí los cambios".into(),
            instructions: "Sé breve".into(),
            work_dir: "C:\\repo".into(),
            ..AgentSpec::default()
        }
    }

    fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter().position(|arg| arg == flag).map(|i| args[i + 1].as_str())
    }

    #[test]
    fn each_autonomy_level_adds_to_the_one_before() {
        let read = claude_allowed_tools(&spec(Engine::Claude, Autonomy::Read));
        let edit = claude_allowed_tools(&spec(Engine::Claude, Autonomy::Edit));
        let run = claude_allowed_tools(&spec(Engine::Claude, Autonomy::Run));
        assert!(read.contains(&"Read".into()) && !read.contains(&"Edit".into()));
        assert!(read.contains(&"Bash(git log:*)".into()) && !read.contains(&"Bash".into()));
        assert!(edit.contains(&"Edit".into()) && !edit.contains(&"Bash".into()));
        assert!(run.contains(&"Bash".into()) && run.contains(&"Edit".into()));
    }

    #[test]
    fn claude_runs_never_wait_for_approval() {
        let args = build_args(&spec(Engine::Claude, Autonomy::Edit));
        assert_eq!(value_after(&args, "-p"), Some("Resumí los cambios"));
        assert_eq!(value_after(&args, "--permission-mode"), Some("dontAsk"));
        assert_eq!(value_after(&args, "--output-format"), Some("stream-json"));
        assert_eq!(value_after(&args, "--append-system-prompt"), Some("Sé breve"));
        assert!(!args.contains(&"--mcp-config".into()));
    }

    #[test]
    fn mcp_servers_are_the_only_ones_and_their_tools_are_allowed() {
        let mut agent = spec(Engine::Claude, Autonomy::Read);
        agent.tools.mcp.push(McpServer {
            name: "github".into(),
            config: serde_json::json!({ "command": "npx", "args": ["-y", "gh-mcp"] }),
        });
        let args = build_args(&agent);
        let config: serde_json::Value = serde_json::from_str(value_after(&args, "--mcp-config").unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["github"]["command"], "npx");
        assert!(args.contains(&"--strict-mcp-config".into()));
        assert!(value_after(&args, "--allowedTools").unwrap().contains("mcp__github"));
    }

    #[test]
    fn codex_gets_a_sandbox_and_the_logic_ahead_of_the_task() {
        let read = build_args(&spec(Engine::Codex, Autonomy::Read));
        assert_eq!(value_after(&read, "--sandbox"), Some("read-only"));
        assert_eq!(value_after(&read, "-C"), Some("C:\\repo"));
        let prompt = read.last().unwrap();
        assert!(prompt.starts_with("Sé breve") && prompt.ends_with("Resumí los cambios"));
        let run = build_args(&spec(Engine::Codex, Autonomy::Run));
        assert_eq!(value_after(&run, "--sandbox"), Some("workspace-write"));
    }

    #[test]
    fn codex_mcp_servers_become_config_overrides() {
        let mut agent = spec(Engine::Codex, Autonomy::Read);
        agent.tools.mcp.push(McpServer {
            name: "docs".into(),
            config: serde_json::json!({ "command": "npx", "args": ["-y", "docs"] }),
        });
        let args = build_args(&agent);
        assert!(args.contains(&"mcp_servers.docs.command=\"npx\"".into()));
        assert!(args.contains(&"mcp_servers.docs.args=[\"-y\",\"docs\"]".into()));
    }

    #[test]
    fn ids_are_safe_folder_names() {
        assert!(is_valid_id("3f2a-b_9"));
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("../x"));
        assert!(!is_valid_id("a b"));
    }

    #[test]
    fn a_partial_file_still_loads_with_defaults() {
        let spec: AgentSpec = serde_json::from_str(r#"{"id":"x","name":"Y"}"#).unwrap();
        assert!(spec.enabled);
        assert_eq!(spec.limits.timeout_min, 15);
        assert_eq!(spec.autonomy, Autonomy::Read);
    }
}
