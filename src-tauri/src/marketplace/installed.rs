//! What's installed where. Each agent keeps its user-level MCP servers in a
//! file of its own; reading the names out of them says which agents already
//! have a server — the badge in the list, and what "uninstall" applies to.
//! Only ever read here: installing goes through install.rs.
//!
//! Termo also keeps its own note of which catalog entry each name it
//! installed belongs to (`Owners`), so installing never lands on top of a
//! server of the same name that came from somewhere else.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::AgentTarget;

fn home() -> Option<PathBuf> {
    dirs::home_dir()
}

/// The file with an agent's user-level MCP servers.
pub fn config_file(target: AgentTarget) -> Option<PathBuf> {
    let home = home()?;
    Some(match target {
        // Claude Code's user scope lives in `.claude.json`, beside its settings.
        AgentTarget::Claude => std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or(home).join(".claude.json"),
        AgentTarget::Codex => {
            std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex")).join("config.toml")
        }
        AgentTarget::Gemini => home.join(".gemini").join("settings.json"),
        AgentTarget::Copilot => crate::agents::spec::copilot_home(&home).join("mcp-config.json"),
    })
}

/// The server names under `mcpServers` in a JSON settings file.
pub fn json_names(text: &str) -> HashSet<String> {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value.get("mcpServers").and_then(Value::as_object).map(|servers| servers.keys().cloned().collect()))
        .unwrap_or_default()
}

/// The server names in Codex's `config.toml`: every `[mcp_servers.<name>]`
/// table, quoted or not, counted once (its `.env` and other subtables are
/// the same server).
pub fn toml_names(text: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("[mcp_servers.") else { continue };
        let Some(path) = rest.strip_suffix(']') else { continue };
        let name = match path.strip_prefix('"') {
            Some(quoted) => quoted.split('"').next().unwrap_or_default(),
            None => path.split('.').next().unwrap_or_default(),
        };
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
    names
}

/// The names of the servers `target` has installed for the user.
pub fn names(target: AgentTarget) -> HashSet<String> {
    let Some(text) = config_file(target).and_then(|path| std::fs::read_to_string(path).ok()) else {
        return HashSet::new();
    };
    match target {
        AgentTarget::Codex => toml_names(&text),
        _ => json_names(&text),
    }
}

/// What every agent has installed, read once for a whole page of results.
pub struct Installed {
    by_target: Vec<(AgentTarget, HashSet<String>)>,
    owners: Owners,
}

impl Installed {
    pub fn read(owners: &Path) -> Self {
        let targets = [AgentTarget::Claude, AgentTarget::Codex, AgentTarget::Gemini, AgentTarget::Copilot];
        Installed {
            by_target: targets.into_iter().map(|target| (target, names(target))).collect(),
            owners: Owners::load(owners),
        }
    }

    /// The agents that have the catalog entry `id` (installed as `name`):
    /// under the name Termo gives it, unless Termo knows that name there
    /// belongs to another entry — or under its full id, as older versions
    /// installed it.
    pub fn agents_with(&self, id: &str, name: &str) -> Vec<AgentTarget> {
        self.by_target
            .iter()
            .filter(|(target, names)| {
                let ours = names.contains(name) && self.owners.get(*target, name).is_none_or(|owner| owner == id);
                ours || names.contains(id)
            })
            .map(|(target, _)| *target)
            .collect()
    }
}

/// Which catalog entry each name Termo installed belongs to, per agent.
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct Owners {
    /// agent → installed name → the catalog entry's id.
    #[serde(default)]
    agents: BTreeMap<String, BTreeMap<String, String>>,
}

impl Owners {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|err| err.to_string())?;
        std::fs::write(path, text).map_err(|err| err.to_string())
    }

    pub fn get(&self, target: AgentTarget, name: &str) -> Option<&str> {
        self.agents.get(target.binary_stem())?.get(name).map(String::as_str)
    }

    pub fn set(&mut self, target: AgentTarget, name: &str, id: &str) {
        self.agents.entry(target.binary_stem().to_string()).or_default().insert(name.to_string(), id.to_string());
    }

    pub fn remove(&mut self, target: AgentTarget, name: &str) {
        if let Some(names) = self.agents.get_mut(target.binary_stem()) {
            names.remove(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_names_out_of_a_json_settings_file() {
        let text = r#"{"theme":"dark","mcpServers":{"github":{"url":"x"},"acme-files":{"command":"npx"}}}"#;
        let names = json_names(text);
        assert!(names.contains("github") && names.contains("acme-files"));
        assert_eq!(names.len(), 2);
        assert!(json_names("not json").is_empty());
        assert!(json_names(r#"{"x":1}"#).is_empty());
    }

    #[test]
    fn reads_the_names_out_of_codex_config() {
        let text = "model = \"o3\"\n\n[mcp_servers.github]\nurl = \"x\"\n\n[mcp_servers.github.env]\nA = \"1\"\n\n[mcp_servers.\"ai.smithery/old-one\"]\ncommand = \"npx\"\n\n[other.table]\n";
        let names = toml_names(text);
        assert_eq!(names.len(), 2);
        assert!(names.contains("github"));
        assert!(names.contains("ai.smithery/old-one"));
    }

    #[test]
    fn a_name_termo_knows_as_someone_elses_isnt_this_entry() {
        let mut owners = Owners::default();
        owners.set(AgentTarget::Claude, "github", "github");
        let installed = Installed {
            by_target: vec![
                (AgentTarget::Claude, ["github".to_string()].into()),
                (AgentTarget::Codex, ["github".to_string()].into()),
                (AgentTarget::Gemini, ["io.github.acme/github".to_string()].into()),
            ],
            owners,
        };
        // Smithery's "github": Termo installed it into Claude; in Codex the
        // name is there with no note, so it's taken as the same server.
        assert_eq!(installed.agents_with("github", "github"), vec![AgentTarget::Claude, AgentTarget::Codex]);
        // Another entry that would get the same name isn't "installed" in
        // Claude — that one is Smithery's — but an old install by full id is.
        let other = installed.agents_with("io.github.acme/github", "github");
        assert_eq!(other, vec![AgentTarget::Codex, AgentTarget::Gemini]);
    }

    #[test]
    fn owners_are_kept_per_agent() {
        let mut owners = Owners::default();
        owners.set(AgentTarget::Claude, "exa", "exa");
        assert_eq!(owners.get(AgentTarget::Claude, "exa"), Some("exa"));
        assert_eq!(owners.get(AgentTarget::Codex, "exa"), None);
        owners.remove(AgentTarget::Claude, "exa");
        assert_eq!(owners.get(AgentTarget::Claude, "exa"), None);
    }
}
