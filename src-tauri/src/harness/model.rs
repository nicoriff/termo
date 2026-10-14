//! A repository's harness: the rules its agents work under, kept in
//! `.termo/harness.json` and turned into each agent's own files (see sync.rs).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::i18n::{tr, Msg};

/// Where the harness lives, relative to the repository's root.
pub const SPEC_FILE: &str = ".termo/harness.json";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
    Gemini,
    Copilot,
    Grok,
}

impl Agent {
    pub const ALL: [Agent; 5] = [Agent::Claude, Agent::Codex, Agent::Gemini, Agent::Copilot, Agent::Grok];
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Access {
    /// Read, but not changed.
    NoEdit,
    /// Neither read nor changed.
    NoRead,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PathRule {
    /// Relative to the repository: `src/generated/**`, or a bare name such
    /// as `.env` for that name in any folder.
    pub pattern: String,
    pub access: Access,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CommandRule {
    /// The command's first words: `git push` covers `git push origin main`.
    pub prefix: String,
    pub decision: Decision,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_sec: Option<u32>,
}

/// What Termo wrote into a JSON file, by the dotted path of the list it
/// went into (`permissions.deny`, `hooks.Stop`): taken out again before the
/// next write, so the file's other entries are never touched.
pub type Owned = BTreeMap<String, Vec<Value>>;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Harness {
    #[serde(default = "current_version")]
    pub version: u32,
    #[serde(default = "all_agents")]
    pub agents: Vec<Agent>,
    #[serde(default)]
    pub paths: Vec<PathRule>,
    #[serde(default)]
    pub commands: Vec<CommandRule>,
    #[serde(default)]
    pub checks: Vec<Check>,
    #[serde(default)]
    pub instructions: String,
    /// By file, relative to the root. Kept by Termo, never edited in the UI.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub managed: BTreeMap<String, Owned>,
}

fn current_version() -> u32 {
    1
}

fn all_agents() -> Vec<Agent> {
    Agent::ALL.to_vec()
}

impl Default for Harness {
    fn default() -> Self {
        Harness {
            version: current_version(),
            agents: all_agents(),
            paths: Vec::new(),
            commands: Vec::new(),
            checks: Vec::new(),
            instructions: String::new(),
            managed: BTreeMap::new(),
        }
    }
}

impl Harness {
    pub fn targets(&self, agent: Agent) -> bool {
        self.agents.contains(&agent)
    }

    /// No rule and nothing to tell the agents: nothing to write.
    pub fn is_blank(&self) -> bool {
        self.paths.is_empty() && self.commands.is_empty() && self.checks.is_empty() && self.instructions.trim().is_empty()
    }

    /// The same rules without blank entries or stray spaces, agents in a
    /// fixed order: what gets written and compared.
    pub fn normalized(&self) -> Harness {
        let mut agents = self.agents.clone();
        agents.sort();
        agents.dedup();
        let mut paths: Vec<PathRule> = Vec::new();
        for rule in &self.paths {
            let pattern = normalize_pattern(&rule.pattern);
            if pattern.is_empty() {
                continue;
            }
            match paths.iter_mut().find(|known| known.pattern == pattern) {
                // The stricter of two rules for one path.
                Some(known) => {
                    if rule.access == Access::NoRead {
                        known.access = Access::NoRead;
                    }
                }
                None => paths.push(PathRule { pattern, access: rule.access }),
            }
        }
        let mut commands: Vec<CommandRule> = Vec::new();
        for rule in &self.commands {
            let prefix = rule.prefix.split_whitespace().collect::<Vec<_>>().join(" ");
            if prefix.is_empty() || commands.iter().any(|known| known.prefix == prefix) {
                continue;
            }
            commands.push(CommandRule { prefix, decision: rule.decision });
        }
        let checks = self
            .checks
            .iter()
            .filter(|check| !check.command.trim().is_empty())
            .map(|check| Check { command: check.command.trim().to_string(), timeout_sec: check.timeout_sec })
            .collect();
        Harness {
            version: current_version(),
            agents,
            paths,
            commands,
            checks,
            instructions: self.instructions.trim().to_string(),
            managed: self.managed.clone(),
        }
    }
}

/// Forward slashes, no leading `./`, no trailing slash.
pub fn normalize_pattern(pattern: &str) -> String {
    let pattern = pattern.trim().replace('\\', "/");
    let pattern = pattern.trim_start_matches("./");
    pattern.trim_end_matches('/').to_string()
}

/// The repository's harness; an empty one when it has none.
pub fn load(root: &Path) -> anyhow::Result<Option<Harness>> {
    let file = root.join(SPEC_FILE);
    let Ok(text) = fs::read_to_string(&file) else { return Ok(None) };
    if text.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|err| anyhow::anyhow!(tr(Msg::HarnessInvalid(SPEC_FILE, &err.to_string()))))
}

pub fn to_text(spec: &Harness) -> String {
    serde_json::to_string_pretty(spec).unwrap_or_default() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_field_takes_its_default() {
        let spec: Harness = serde_json::from_str(r#"{ "paths": [{ "pattern": ".env", "access": "noRead" }] }"#).unwrap();
        assert_eq!(spec.version, 1);
        assert_eq!(spec.agents, Agent::ALL.to_vec());
        assert_eq!(spec.paths[0].access, Access::NoRead);
    }

    #[test]
    fn normalizing_drops_blanks_and_keeps_the_stricter_rule() {
        let spec = Harness {
            agents: vec![Agent::Grok, Agent::Claude, Agent::Grok],
            paths: vec![
                PathRule { pattern: " ./src\\gen/ ".into(), access: Access::NoEdit },
                PathRule { pattern: "src/gen".into(), access: Access::NoRead },
                PathRule { pattern: "  ".into(), access: Access::NoEdit },
            ],
            commands: vec![
                CommandRule { prefix: " git   push ".into(), decision: Decision::Ask },
                CommandRule { prefix: "git push".into(), decision: Decision::Deny },
            ],
            checks: vec![Check { command: " ".into(), timeout_sec: None }],
            ..Harness::default()
        }
        .normalized();
        assert_eq!(spec.agents, vec![Agent::Claude, Agent::Grok]);
        assert_eq!(spec.paths, vec![PathRule { pattern: "src/gen".into(), access: Access::NoRead }]);
        assert_eq!(spec.commands, vec![CommandRule { prefix: "git push".into(), decision: Decision::Ask }]);
        assert!(spec.checks.is_empty());
    }
}
