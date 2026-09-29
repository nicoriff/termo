//! Giving an agent an MCP server from the registry: the same config the
//! marketplace would install into Claude or Codex — built by the same code,
//! with the same checks on what reaches npx/uvx — but kept inside the agent,
//! and with its secret values moved to the keychain (see secrets.rs).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::i18n::{tr, Msg};
use crate::marketplace::agent_cli;
use crate::marketplace::install::{server_config, McpInstallRequest};
use crate::marketplace::model::AgentTarget;

use super::secrets;
use super::spec::{is_env_name, is_header_name, is_reserved_env, is_valid_id, McpServer};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentMcpRequest {
    pub agent_id: String,
    /// The registry's name, e.g. `io.github.someone/github-mcp`.
    pub server_name: String,
    pub remotes: Vec<Value>,
    pub packages: Vec<Value>,
    /// Every value the user filled in, secret or not, keyed by header or
    /// variable name.
    pub params: HashMap<String, String>,
    /// Which of `params` are secrets.
    pub secret_params: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentMcpServer {
    pub server: McpServer,
    /// The names its secrets were stored under.
    pub secrets: Vec<String>,
}

/// The name the agent knows the server by (its tools become
/// `mcp__<name>__…`): the last part of the registry name, in the few
/// characters tool names allow.
pub fn short_name(server_name: &str) -> String {
    let last = server_name.rsplit('/').next().unwrap_or(server_name);
    let name: String = last
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '-' })
        .take(40)
        .collect();
    let name = name.trim_matches('-').to_string();
    if name.is_empty() {
        "server".into()
    } else {
        name
    }
}

/// Stores the secrets and returns the server's config with `${NAME}` where
/// each secret goes.
pub fn build(request: AgentMcpRequest) -> Result<AgentMcpServer, String> {
    if !is_valid_id(&request.agent_id) {
        return Err(tr(Msg::AgentSaveFailed("invalid agent id")));
    }
    if !agent_cli::is_valid_component_name(&request.server_name) {
        return Err(tr(Msg::InvalidComponentName(&request.server_name)));
    }
    let name = short_name(&request.server_name);
    let mut params = request.params.clone();
    let mut stored = Vec::new();
    for param in &request.secret_params {
        let Some(value) = request.params.get(param).filter(|value| !value.trim().is_empty()) else {
            continue;
        };
        let env = secrets::env_name(&name, param);
        secrets::set(&request.agent_id, &env, value).map_err(|err| tr(Msg::AgentSecretFailed(&err)))?;
        params.insert(param.clone(), format!("${{{env}}}"));
        stored.push(env);
    }
    let config = server_config(&McpInstallRequest {
        server_name: request.server_name.clone(),
        target: AgentTarget::Claude,
        remotes: request.remotes,
        packages: request.packages,
        params,
    })?;
    Ok(AgentMcpServer {
        server: McpServer { name, config },
        secrets: stored,
    })
}

/// One environment variable (a local command) or header (a remote server)
/// of a connector the user wrote themselves.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CustomValue {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub secret: bool,
}

/// A connector of the user's own: not from the registry, so no registry
/// checks — it's their command or their URL — but held to the same shape.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CustomMcpRequest {
    pub agent_id: String,
    pub name: String,
    /// "stdio" (a local command) | "http" | "sse"
    pub kind: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub values: Vec<CustomValue>,
}

/// The connector's config, with its secrets moved to the keychain.
pub fn build_custom(request: CustomMcpRequest) -> Result<AgentMcpServer, String> {
    if !is_valid_id(&request.agent_id) {
        return Err(tr(Msg::AgentSaveFailed("invalid agent id")));
    }
    let name = short_name(request.name.trim());
    let remote = request.kind == "http" || request.kind == "sse";
    let mut config = serde_json::Map::new();
    if remote {
        let url = request.url.trim();
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(tr(Msg::UnsafeMcpRemoteUrl(url)));
        }
        config.insert("type".into(), Value::String(request.kind.clone()));
        config.insert("url".into(), Value::String(url.into()));
    } else {
        let command = request.command.trim();
        if command.is_empty() {
            return Err(tr(Msg::AgentConnectorIncomplete));
        }
        config.insert("type".into(), Value::String("stdio".into()));
        config.insert("command".into(), Value::String(command.into()));
        let args: Vec<Value> = request
            .args
            .iter()
            .map(|arg| arg.trim())
            .filter(|arg| !arg.is_empty())
            .map(|arg| Value::String(arg.into()))
            .collect();
        config.insert("args".into(), Value::Array(args));
    }

    let mut values = serde_json::Map::new();
    let mut stored = Vec::new();
    for entry in &request.values {
        let key = entry.name.trim();
        if key.is_empty() {
            continue;
        }
        let valid = if remote { is_header_name(key) } else { is_env_name(key) };
        if !valid {
            return Err(tr(Msg::AgentConnectorBadName(key)));
        }
        // Codex hands a secret variable to the connector through its own
        // environment (see spec::check).
        if !remote && entry.secret && is_reserved_env(key) {
            return Err(tr(Msg::AgentBadVariable(key)));
        }
        let value = if entry.secret && !entry.value.is_empty() {
            let env = secrets::env_name(&name, key);
            secrets::set(&request.agent_id, &env, &entry.value).map_err(|err| tr(Msg::AgentSecretFailed(&err)))?;
            stored.push(env.clone());
            format!("${{{env}}}")
        } else {
            entry.value.clone()
        };
        values.insert(key.into(), Value::String(value));
    }
    if !values.is_empty() {
        config.insert(if remote { "headers" } else { "env" }.into(), Value::Object(values));
    }

    Ok(AgentMcpServer {
        server: McpServer { name, config: Value::Object(config) },
        secrets: stored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(kind: &str) -> CustomMcpRequest {
        CustomMcpRequest {
            agent_id: "a1".into(),
            name: "Mi API".into(),
            kind: kind.into(),
            command: "npx".into(),
            args: vec!["-y".into(), " mi-server ".into(), "".into()],
            url: "https://api.example.com/mcp".into(),
            values: vec![CustomValue { name: "REGION".into(), value: "eu".into(), secret: false }],
        }
    }

    #[test]
    fn a_local_command_keeps_its_arguments_and_variables() {
        let result = build_custom(custom("stdio")).unwrap();
        assert_eq!(result.server.name, "mi-api");
        assert_eq!(result.server.config["command"], "npx");
        assert_eq!(result.server.config["args"], serde_json::json!(["-y", "mi-server"]));
        assert_eq!(result.server.config["env"]["REGION"], "eu");
        assert!(result.server.config.get("url").is_none());
    }

    #[test]
    fn a_remote_server_gets_headers_and_only_a_web_url() {
        let mut request = custom("http");
        request.values = vec![CustomValue { name: "X-Region".into(), value: "eu".into(), secret: false }];
        let result = build_custom(request).unwrap();
        assert_eq!(result.server.config["url"], "https://api.example.com/mcp");
        assert_eq!(result.server.config["headers"]["X-Region"], "eu");
        let mut bad = custom("http");
        bad.url = "file:///etc/passwd".into();
        assert!(build_custom(bad).is_err());
    }

    #[test]
    fn names_that_cant_be_variables_or_headers_are_refused() {
        let mut request = custom("stdio");
        request.values = vec![CustomValue { name: "MY VAR".into(), value: "x".into(), secret: false }];
        assert!(build_custom(request).is_err());
        // A secret variable reaches Codex's own environment.
        let mut reserved = custom("stdio");
        reserved.values = vec![CustomValue { name: "NODE_OPTIONS".into(), value: "x".into(), secret: true }];
        assert!(build_custom(reserved).is_err());
        let mut empty = custom("stdio");
        empty.command = " ".into();
        assert!(build_custom(empty).is_err());
    }

    #[test]
    fn short_names_fit_tool_names() {
        assert_eq!(short_name("io.github.someone/GitHub-MCP"), "github-mcp");
        assert_eq!(short_name("ai.smithery/some server!"), "some-server");
        assert_eq!(short_name("///"), "server");
    }

    #[test]
    fn a_remote_server_keeps_non_secret_values() {
        let result = build(AgentMcpRequest {
            agent_id: "a1".into(),
            server_name: "io.example/docs".into(),
            remotes: vec![serde_json::json!({
                "type": "streamable-http",
                "url": "https://docs.example.com/mcp",
                "headers": [{ "name": "X-Region" }]
            })],
            packages: vec![],
            params: HashMap::from([("X-Region".to_string(), "eu".to_string())]),
            secret_params: vec![],
        })
        .unwrap();
        assert_eq!(result.server.name, "docs");
        assert_eq!(result.server.config["url"], "https://docs.example.com/mcp");
        assert_eq!(result.server.config["headers"]["X-Region"], "eu");
        assert!(result.secrets.is_empty());
    }
}
