//! Browsing and installing Claude Code / Codex extensions from inside
//! Termo: plugins from Claude's marketplaces and servers from the MCP
//! registry, installed through each agent's own CLI (see agent_cli.rs).

pub mod agent_cli;
pub mod catalog;
pub mod claude_plugins;
pub mod install;
pub mod mcp_registry;
pub mod model;
