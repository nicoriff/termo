//! Installing and removing what the panel lists.
//!
//! Always through `claude plugin install/uninstall`, never by editing its
//! files — and never with `--accept-command` on the first try. A marketplace
//! can declare a command that runs on this machine as part of installing;
//! the CLI refuses to run it unattended and reports it instead, so Termini
//! hands that back to the UI, shows the exact command and its checksum, and
//! only retries with the approval once a person has said yes to that precise
//! text. Auto-approving would turn this panel into a way for a catalog entry
//! to run anything it likes, silently.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::i18n::{tr, Msg};

use super::agent_cli;
use super::model::AgentTarget;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    /// `<name>@<marketplace>`, as the CLI takes it.
    pub plugin_id: String,
    pub target: AgentTarget,
    /// The sha256 the UI showed and the user approved, when there was one.
    pub accept_command: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredCommand {
    pub sha256: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum InstallOutcome {
    Installed { message: String },
    /// The marketplace declares a command that has to be approved by a
    /// person before it runs. Never auto-approved.
    NeedsCommandApproval { commands: Vec<DeclaredCommand> },
}

/// Pulls the declared command out of a `--json` run, whatever level it sits
/// at: the CLI reports it as `shownCommand`, and its exact nesting isn't
/// something to depend on. A command with no checksum is useless here —
/// `--accept-command` takes the sha256 — so those are ignored.
fn declared_commands(output: &str) -> Vec<DeclaredCommand> {
    let mut found = Vec::new();
    for line in output.lines().rev() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        collect_shown_commands(&value, &mut found);
        if !found.is_empty() {
            break;
        }
    }
    found
}

fn collect_shown_commands(value: &Value, found: &mut Vec<DeclaredCommand>) {
    match value {
        Value::Object(map) => {
            if let Some(shown) = map.get("shownCommand") {
                push_command(shown, found);
            }
            for nested in map.values() {
                collect_shown_commands(nested, found);
            }
        }
        Value::Array(items) => {
            for nested in items {
                collect_shown_commands(nested, found);
            }
        }
        _ => {}
    }
}

fn push_command(value: &Value, found: &mut Vec<DeclaredCommand>) {
    match value {
        Value::Array(items) => {
            for item in items {
                push_command(item, found);
            }
        }
        Value::Object(map) => {
            let Some(sha256) = map.get("sha256").and_then(Value::as_str) else {
                return;
            };
            let text = ["command", "text", "script", "display"]
                .iter()
                .find_map(|key| map.get(*key).and_then(Value::as_str))
                .unwrap_or(sha256);
            if !found.iter().any(|existing| existing.sha256 == sha256) {
                found.push(DeclaredCommand {
                    sha256: sha256.to_string(),
                    text: text.to_string(),
                });
            }
        }
        _ => {}
    }
}

/// What the CLI said, for the panel to show on success. Falls back to the
/// raw output when it isn't the shape we expect.
fn success_message(output: &str) -> String {
    for line in output.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            if let Some(message) = ["message", "result", "status"]
                .iter()
                .find_map(|key| value.get(*key).and_then(Value::as_str))
            {
                return message.to_string();
            }
        }
        return trimmed.to_string();
    }
    String::new()
}

fn require_claude(request: &InstallRequest) -> Result<PathBuf, String> {
    if request.target != AgentTarget::Claude {
        // Codex has no plugin system at all; the UI disables the target, and
        // this is the backstop if something ever gets past it.
        return Err(tr(Msg::CodexHasNoPlugins));
    }
    if !agent_cli::is_valid_component_name(&request.plugin_id) {
        return Err(tr(Msg::InvalidComponentName(&request.plugin_id)));
    }
    agent_cli::require(AgentTarget::Claude, None)
}

pub fn install_plugin(request: &InstallRequest) -> Result<InstallOutcome, String> {
    let claude = require_claude(request)?;

    let mut args = vec!["plugin", "install", request.plugin_id.as_str(), "--json"];
    if let Some(sha) = request.accept_command.as_deref() {
        args.push("--accept-command");
        args.push(sha);
    }

    let output = agent_cli::run_capture(&claude, &args)?;
    let commands = declared_commands(&output.stdout);
    if !commands.is_empty() && request.accept_command.is_none() {
        return Ok(InstallOutcome::NeedsCommandApproval { commands });
    }
    if !output.success {
        let detail = if output.stderr.trim().is_empty() {
            output.stdout.trim().to_string()
        } else {
            output.stderr.trim().to_string()
        };
        return Err(tr(Msg::MarketplaceCommandFailed(&detail)));
    }
    Ok(InstallOutcome::Installed {
        message: success_message(&output.stdout),
    })
}

pub fn uninstall_plugin(request: &InstallRequest) -> Result<(), String> {
    let claude = require_claude(request)?;
    agent_cli::run(&claude, &["plugin", "uninstall", request.plugin_id.as_str()]).map(|_| ())
}

/// The registry's own entries don't carry install parameters — a remote
/// needs the header values the registry only shows as placeholders (a
/// secret can't come from a public catalog), and a stdio package needs
/// whatever environment variables it declares. `params` is what the UI
/// collected for those from the user, keyed by header or variable name.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McpInstallRequest {
    /// The registry's `name` (e.g. `ai.smithery/some-server`), used both as
    /// the config key and to validate before anything runs.
    pub server_name: String,
    pub target: AgentTarget,
    pub remotes: Vec<Value>,
    pub packages: Vec<Value>,
    pub params: std::collections::HashMap<String, String>,
}

/// A launch command for the registry's handful of known package registries.
/// Anything else is refused rather than guessed at: running an arbitrary
/// `identifier` as a bare command would let a catalog entry name its own
/// executable.
fn stdio_launch(package: &Value) -> Option<(String, Vec<String>)> {
    let registry_type = package.get("registryType").and_then(Value::as_str)?;
    let identifier = package.get("identifier").and_then(Value::as_str)?;
    let version = package.get("version").and_then(Value::as_str);
    let versioned = match version {
        Some(version) => format!("{identifier}@{version}"),
        None => identifier.to_string(),
    };
    match registry_type {
        "npm" => Some(("npx".to_string(), vec!["-y".to_string(), versioned])),
        "pypi" => Some(("uvx".to_string(), vec![versioned])),
        _ => None,
    }
}

/// Fills in a remote's headers or a package's environment variables from
/// what the user supplied. A required field with nothing supplied is left
/// out rather than sent as the registry's own placeholder text (`"Bearer
/// {smithery_api_key}"`) — that string is documentation, not a working
/// value, and would silently produce a server that just fails to
/// authenticate.
fn filled_map(spec: &[Value], params: &std::collections::HashMap<String, String>) -> serde_json::Map<String, Value> {
    let mut map = serde_json::Map::new();
    for entry in spec {
        let Some(name) = entry.get("name").and_then(Value::as_str) else { continue };
        if let Some(value) = params.get(name) {
            map.insert(name.to_string(), Value::String(value.clone()));
        } else if let Some(default) = entry.get("default").and_then(Value::as_str) {
            map.insert(name.to_string(), Value::String(default.to_string()));
        }
    }
    map
}

/// Builds the `.mcp.json`-shaped server config `claude mcp add-json` takes,
/// preferring a remote (nothing to install locally) over a stdio package.
fn server_config(request: &McpInstallRequest) -> Result<Value, String> {
    if let Some(remote) = request.remotes.first() {
        let transport = remote.get("type").and_then(Value::as_str).unwrap_or("http");
        let url = remote
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| tr(Msg::MarketplaceCommandFailed("remote has no url")))?;
        let headers = remote.get("headers").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut config = serde_json::Map::new();
        config.insert("type".to_string(), Value::String(if transport == "sse" { "sse".into() } else { "http".into() }));
        config.insert("url".to_string(), Value::String(url.to_string()));
        let filled = filled_map(&headers, &request.params);
        if !filled.is_empty() {
            config.insert("headers".to_string(), Value::Object(filled));
        }
        return Ok(Value::Object(config));
    }

    if let Some(package) = request.packages.first() {
        let (command, mut args) = stdio_launch(package)
            .ok_or_else(|| tr(Msg::MarketplaceCommandFailed("unsupported package registry")))?;
        if let Some(extra) = package.get("runtimeArguments").and_then(Value::as_array) {
            for arg in extra {
                if let Some(value) = arg.get("value").and_then(Value::as_str) {
                    args.push(value.to_string());
                }
            }
        }
        let env_spec = package.get("environmentVariables").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut config = serde_json::Map::new();
        config.insert("type".to_string(), Value::String("stdio".to_string()));
        config.insert("command".to_string(), Value::String(command));
        config.insert("args".to_string(), Value::Array(args.into_iter().map(Value::String).collect()));
        let env = filled_map(&env_spec, &request.params);
        if !env.is_empty() {
            config.insert("env".to_string(), Value::Object(env));
        }
        return Ok(Value::Object(config));
    }

    Err(tr(Msg::MarketplaceCommandFailed("no remote or package to install")))
}

pub fn install_mcp(request: &McpInstallRequest) -> Result<InstallOutcome, String> {
    if !agent_cli::is_valid_component_name(&request.server_name) {
        return Err(tr(Msg::InvalidComponentName(&request.server_name)));
    }
    match request.target {
        AgentTarget::Claude => install_mcp_claude(request),
        AgentTarget::Codex => install_mcp_codex(request),
    }
}

fn install_mcp_claude(request: &McpInstallRequest) -> Result<InstallOutcome, String> {
    let claude = agent_cli::require(AgentTarget::Claude, None)?;
    let config = server_config(request)?;
    let config_text = serde_json::to_string(&config).map_err(|err| tr(Msg::MarketplaceCommandFailed(&err.to_string())))?;

    agent_cli::run(
        &claude,
        &["mcp", "add-json", request.server_name.as_str(), config_text.as_str(), "-s", "user"],
    )
    .map(|output| InstallOutcome::Installed { message: output.trim().to_string() })
}

fn install_mcp_codex(request: &McpInstallRequest) -> Result<InstallOutcome, String> {
    let codex = agent_cli::require(AgentTarget::Codex, None)?;
    let args = codex_add_args(request)?;
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    agent_cli::run(&codex, &arg_refs).map(|output| InstallOutcome::Installed { message: output.trim().to_string() })
}

/// Builds the argv for `codex mcp add <name> ...` — Codex's own real
/// subcommand, not the config.toml this code used to assume was the only
/// way in. Its flag surface is narrower than `.mcp.json`, though: a stdio
/// package translates one-to-one (command, args, one `--env` per variable),
/// but a remote only ever takes a bearer token read from an *environment
/// variable* at launch time, never a literal header value — so a remote
/// whose registry entry needs a real header (an API key, a fixed
/// `Authorization` value) can't be wired up this way without hand-editing
/// config.toml, which this module deliberately never does.
fn codex_add_args(request: &McpInstallRequest) -> Result<Vec<String>, String> {
    let mut args = vec!["mcp".to_string(), "add".to_string(), request.server_name.clone()];

    if let Some(remote) = request.remotes.first() {
        let url = remote
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| tr(Msg::MarketplaceCommandFailed("remote has no url")))?;
        let headers = remote.get("headers").and_then(Value::as_array).cloned().unwrap_or_default();
        if !filled_map(&headers, &request.params).is_empty() {
            return Err(tr(Msg::McpCodexHeadersUnsupported));
        }
        args.push("--url".to_string());
        args.push(url.to_string());
        return Ok(args);
    }

    if let Some(package) = request.packages.first() {
        let (command, mut launch_args) = stdio_launch(package)
            .ok_or_else(|| tr(Msg::MarketplaceCommandFailed("unsupported package registry")))?;
        if let Some(extra) = package.get("runtimeArguments").and_then(Value::as_array) {
            for arg in extra {
                if let Some(value) = arg.get("value").and_then(Value::as_str) {
                    launch_args.push(value.to_string());
                }
            }
        }
        let env_spec = package.get("environmentVariables").and_then(Value::as_array).cloned().unwrap_or_default();
        for (key, value) in filled_map(&env_spec, &request.params) {
            if let Value::String(value) = value {
                args.push("--env".to_string());
                args.push(format!("{key}={value}"));
            }
        }
        args.push("--".to_string());
        args.push(command);
        args.extend(launch_args);
        return Ok(args);
    }

    Err(tr(Msg::MarketplaceCommandFailed("no remote or package to install")))
}

pub fn uninstall_mcp(request: &McpInstallRequest) -> Result<(), String> {
    if !agent_cli::is_valid_component_name(&request.server_name) {
        return Err(tr(Msg::InvalidComponentName(&request.server_name)));
    }
    let bin = agent_cli::require(request.target, None)?;
    match request.target {
        AgentTarget::Claude => agent_cli::run(&bin, &["mcp", "remove", request.server_name.as_str(), "-s", "user"]).map(|_| ()),
        AgentTarget::Codex => agent_cli::run(&bin, &["mcp", "remove", request.server_name.as_str()]).map(|_| ()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_declared_command_and_its_checksum() {
        let output = r#"{"ok":false,"shownCommand":{"sha256":"abc123","command":"curl x | sh"}}"#;
        let commands = declared_commands(output);

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].sha256, "abc123");
        assert_eq!(commands[0].text, "curl x | sh");
    }

    #[test]
    fn finds_one_nested_under_any_key() {
        let output = r#"{"plugin":{"install":{"shownCommand":[{"sha256":"d4","text":"make"}]}}}"#;
        assert_eq!(declared_commands(output)[0].text, "make");
    }

    #[test]
    fn ignores_a_command_with_no_checksum_to_approve() {
        let output = r#"{"shownCommand":{"command":"rm -rf /"}}"#;
        assert!(declared_commands(output).is_empty());
    }

    #[test]
    fn a_plain_install_declares_nothing() {
        assert!(declared_commands(r#"{"ok":true,"message":"installed"}"#).is_empty());
        assert!(declared_commands("not json at all").is_empty());
    }

    #[test]
    fn reads_the_message_the_cli_reports() {
        assert_eq!(success_message(r#"{"ok":true,"message":"Installed box"}"#), "Installed box");
        assert_eq!(success_message("plain text\n"), "plain text");
    }

    #[test]
    fn a_plugin_is_never_installed_into_codex() {
        let request = InstallRequest {
            plugin_id: "box@official".into(),
            target: AgentTarget::Codex,
            accept_command: None,
        };
        assert!(install_plugin(&request).is_err());
    }

    #[test]
    fn a_name_that_could_become_a_flag_is_refused_before_running_anything() {
        let request = InstallRequest {
            plugin_id: "--help".into(),
            target: AgentTarget::Claude,
            accept_command: None,
        };
        let err = install_plugin(&request).unwrap_err();
        assert!(err.contains("--help"), "err: {err}");
    }

    fn mcp_request(remotes: Vec<Value>, packages: Vec<Value>, params: &[(&str, &str)]) -> McpInstallRequest {
        McpInstallRequest {
            server_name: "ai.smithery/example".into(),
            target: AgentTarget::Claude,
            remotes,
            packages,
            params: params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }
    }

    #[test]
    fn builds_a_remote_config_and_fills_in_a_supplied_header() {
        let remote = serde_json::json!({
            "type": "streamable-http",
            "url": "https://server.smithery.ai/x/mcp",
            "headers": [{ "name": "Authorization", "isRequired": true, "isSecret": true, "value": "Bearer {key}" }]
        });
        let request = mcp_request(vec![remote], vec![], &[("Authorization", "Bearer real-token")]);
        let config = server_config(&request).unwrap();

        assert_eq!(config["type"], "http");
        assert_eq!(config["url"], "https://server.smithery.ai/x/mcp");
        assert_eq!(config["headers"]["Authorization"], "Bearer real-token");
    }

    #[test]
    fn never_sends_the_registrys_own_placeholder_as_a_real_value() {
        let remote = serde_json::json!({
            "type": "streamable-http",
            "url": "https://server.smithery.ai/x/mcp",
            "headers": [{ "name": "Authorization", "isRequired": true, "isSecret": true, "value": "Bearer {key}" }]
        });
        // Nothing supplied for the required header: the placeholder text
        // must never end up in the config as if it were a working value.
        let request = mcp_request(vec![remote], vec![], &[]);
        let config = server_config(&request).unwrap();

        assert!(config.get("headers").is_none() || config["headers"].get("Authorization").is_none());
    }

    #[test]
    fn builds_an_npx_launch_for_an_npm_package() {
        let package = serde_json::json!({
            "registryType": "npm",
            "identifier": "remote-filesystem-mcp-server",
            "version": "0.1.5",
            "environmentVariables": [{ "name": "GCS_BUCKET", "isRequired": true }]
        });
        let request = mcp_request(vec![], vec![package], &[("GCS_BUCKET", "my-bucket")]);
        let config = server_config(&request).unwrap();

        assert_eq!(config["type"], "stdio");
        assert_eq!(config["command"], "npx");
        assert_eq!(config["args"][0], "-y");
        assert_eq!(config["args"][1], "remote-filesystem-mcp-server@0.1.5");
        assert_eq!(config["env"]["GCS_BUCKET"], "my-bucket");
    }

    #[test]
    fn refuses_a_package_registry_it_does_not_know_how_to_launch() {
        let package = serde_json::json!({ "registryType": "cargo", "identifier": "whatever" });
        let request = mcp_request(vec![], vec![package], &[]);
        assert!(server_config(&request).is_err());
    }

    #[test]
    fn builds_a_codex_add_command_for_a_header_free_remote() {
        let remote = serde_json::json!({ "type": "streamable-http", "url": "https://server.example/mcp" });
        let request = mcp_request(vec![remote], vec![], &[]);
        let args = codex_add_args(&request).unwrap();
        assert_eq!(args, vec!["mcp", "add", "ai.smithery/example", "--url", "https://server.example/mcp"]);
    }

    #[test]
    fn refuses_a_codex_remote_that_needs_a_real_header() {
        let remote = serde_json::json!({
            "type": "streamable-http",
            "url": "https://server.smithery.ai/x/mcp",
            "headers": [{ "name": "Authorization", "isRequired": true, "value": "Bearer {key}" }]
        });
        // Codex's `mcp add` has no flag for a literal header value, only a
        // bearer token read from an env var — a filled-in header can't be
        // expressed, so this must fail rather than silently drop it.
        let request = mcp_request(vec![remote], vec![], &[("Authorization", "Bearer real-token")]);
        let err = codex_add_args(&request).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn a_header_free_remote_still_installs_fine_into_codex() {
        let remote = serde_json::json!({ "type": "streamable-http", "url": "https://server.example/mcp" });
        let request = mcp_request(vec![remote], vec![], &[]);
        assert!(codex_add_args(&request).is_ok());
    }

    #[test]
    fn builds_a_codex_add_command_for_an_npx_stdio_package() {
        let package = serde_json::json!({
            "registryType": "npm",
            "identifier": "remote-filesystem-mcp-server",
            "version": "0.1.5",
            "environmentVariables": [{ "name": "GCS_BUCKET", "isRequired": true }]
        });
        let request = mcp_request(vec![], vec![package], &[("GCS_BUCKET", "my-bucket")]);
        let args = codex_add_args(&request).unwrap();
        assert_eq!(
            args,
            vec![
                "mcp",
                "add",
                "ai.smithery/example",
                "--env",
                "GCS_BUCKET=my-bucket",
                "--",
                "npx",
                "-y",
                "remote-filesystem-mcp-server@0.1.5",
            ]
        );
    }

    #[test]
    fn an_mcp_name_that_could_become_a_flag_is_refused_before_running_anything() {
        let mut request = mcp_request(vec![serde_json::json!({"type":"http","url":"https://x"})], vec![], &[]);
        request.server_name = "--help".into();
        let err = install_mcp(&request).unwrap_err();
        assert!(err.contains("--help"), "err: {err}");
    }
}
