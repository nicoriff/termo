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
use super::spec::{is_valid_id, McpServer};

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

#[cfg(test)]
mod tests {
    use super::*;

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
