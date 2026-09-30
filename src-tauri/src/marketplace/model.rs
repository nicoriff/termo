//! The shape the marketplace browser works in.
//!
//! Two very different catalogs feed it — Claude Code's plugin marketplaces
//! (a `marketplace.json` in a cloned repo, read through the `claude` CLI) and
//! the MCP registry (a paginated HTTP API) — so `CatalogItem` carries what
//! both have in common and keeps each one's install parameters in a tagged
//! `detail`, rather than flattening them into optional fields nobody can tell
//! apart downstream.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ComponentClass {
    Plugin,
    Skill,
    Mcp,
}

/// Which agent an item can be installed into. Same two-tool split as
/// pty::agent_hooks::AgentTool, kept separate because that enum is about
/// hook files and this one is about install targets.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum AgentTarget {
    Claude,
    Codex,
    Gemini,
}

impl AgentTarget {
    pub fn binary_stem(self) -> &'static str {
        match self {
            AgentTarget::Claude => "claude",
            AgentTarget::Codex => "codex",
            AgentTarget::Gemini => "gemini",
        }
    }
}

/// Which agents can take an item of this class, as a function of the class
/// alone — not stored per item, because it isn't per-item data: only Claude
/// has plugins, skills go to Claude and Codex, and MCP servers to all three.
pub fn targets_for(class: ComponentClass) -> Vec<AgentTarget> {
    match class {
        ComponentClass::Plugin => vec![AgentTarget::Claude],
        ComponentClass::Skill => vec![AgentTarget::Claude, AgentTarget::Codex],
        // An MCP server goes into Gemini CLI's settings too.
        ComponentClass::Mcp => vec![AgentTarget::Claude, AgentTarget::Codex, AgentTarget::Gemini],
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    /// Stable across refreshes and unique across sources: `plugin:<name>@<marketplace>`
    /// or `mcp:<server name>`.
    pub id: String,
    pub class: ComponentClass,
    /// The machine name, i.e. what goes on the command line.
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub source_id: String,
    pub source_label: String,
    pub category: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    pub tags: Vec<String>,
    pub install_count: Option<u64>,
    /// An icon for the entry. Nothing in these catalogs publishes one, so
    /// it's derived from where the code is published (see icon_for in
    /// claude_plugins.rs); None means the UI draws initials instead.
    pub icon_url: Option<String>,
    /// 0-5 if a source ever publishes one. Neither Claude's marketplaces nor
    /// the MCP registry does today, so this is almost always None and the UI
    /// simply shows nothing rather than inventing a score.
    pub rating: Option<f64>,
    /// The skills a marketplace entry declares, when it declares any. Almost
    /// none do (3 of 308 in the official marketplace today), and a plugin's
    /// real component inventory isn't knowable before installing it — so this
    /// is a facet to filter and badge by, never the item's class.
    pub declares_skills: Vec<String>,
    pub targets: Vec<AgentTarget>,
    pub installed_in: Vec<AgentTarget>,
    pub detail: ItemDetail,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ItemDetail {
    Plugin {
        /// `<name>@<marketplace>`, the id the CLI takes.
        plugin_id: String,
        marketplace: String,
        /// The entry's `source` object, passed through untouched: it's what
        /// tells the user where the code they're about to run comes from.
        source: serde_json::Value,
        strict: Option<bool>,
    },
    Mcp {
        server_name: String,
        remotes: Vec<serde_json::Value>,
        packages: Vec<serde_json::Value>,
        repository: Option<serde_json::Value>,
    },
}

/// What the frontend needs to know before it offers to install anything:
/// which CLIs are actually available on this machine.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceStatus {
    pub claude_cli: Option<String>,
    pub codex_cli: Option<String>,
    pub gemini_cli: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_never_takes_a_plugin() {
        assert_eq!(targets_for(ComponentClass::Plugin), vec![AgentTarget::Claude]);
        assert!(targets_for(ComponentClass::Mcp).contains(&AgentTarget::Codex));
        assert!(targets_for(ComponentClass::Skill).contains(&AgentTarget::Codex));
    }
}
