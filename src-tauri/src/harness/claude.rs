//! Claude Code's shared project settings, `.claude/settings.json`: path and
//! command rules as `permissions`, the checks as a `Stop` hook. Grok Build
//! reads the same file's permissions and hooks, so it's written for Grok too.

use serde_json::{json, Map, Value};

use super::checks;
use super::model::{Access, Agent, Decision, Harness, Owned};

pub const FILE: &str = ".claude/settings.json";

pub fn applies(spec: &Harness) -> bool {
    spec.targets(Agent::Claude) || spec.targets(Agent::Grok)
}

/// A path pattern as a permission rule's: one with a folder in it from the
/// project's root (`/src/gen/**`), a bare name in any folder (`**/.env`).
pub fn path(pattern: &str) -> String {
    if pattern.starts_with('/') || pattern.starts_with("**") {
        pattern.to_string()
    } else if pattern.contains('/') {
        format!("/{pattern}")
    } else {
        format!("**/{pattern}")
    }
}

/// The command as typed, and followed by anything, in both shells Claude
/// Code runs.
pub fn command_rules(prefix: &str) -> Vec<String> {
    ["Bash", "PowerShell"]
        .iter()
        .flat_map(|tool| [format!("{tool}({prefix})"), format!("{tool}({prefix} *)")])
        .collect()
}

pub fn desired(spec: &Harness, _current: &Map<String, Value>) -> Owned {
    let mut owned = Owned::new();
    let mut push = |key: &str, value: Value| owned.entry(key.to_string()).or_default().push(value);
    for rule in &spec.paths {
        let pattern = path(&rule.pattern);
        if rule.access == Access::NoRead {
            push("permissions.deny", json!(format!("Read({pattern})")));
        }
        push("permissions.deny", json!(format!("Edit({pattern})")));
    }
    for rule in &spec.commands {
        let key = match rule.decision {
            Decision::Deny => "permissions.deny",
            Decision::Ask => "permissions.ask",
            Decision::Allow => "permissions.allow",
        };
        for value in command_rules(&rule.prefix) {
            push(key, json!(value));
        }
    }
    if !spec.checks.is_empty() {
        push(
            "hooks.Stop",
            json!({ "hooks": [{
                "type": "command",
                "command": checks::command(),
                "timeout": checks::timeout_sec(&spec.checks),
            }] }),
        );
    }
    owned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::model::{Check, CommandRule, PathRule};

    #[test]
    fn patterns_are_anchored_or_found_anywhere() {
        assert_eq!(path(".env"), "**/.env");
        assert_eq!(path("src/gen/**"), "/src/gen/**");
        assert_eq!(path("/build"), "/build");
        assert_eq!(path("**/*.pem"), "**/*.pem");
    }

    #[test]
    fn rules_land_in_their_lists() {
        let spec = Harness {
            paths: vec![
                PathRule { pattern: ".env".into(), access: Access::NoRead },
                PathRule { pattern: "gen/**".into(), access: Access::NoEdit },
            ],
            commands: vec![CommandRule { prefix: "git push".into(), decision: Decision::Ask }],
            checks: vec![Check { command: "npm run check".into(), timeout_sec: Some(60) }],
            ..Harness::default()
        };
        let owned = desired(&spec, &Map::new());
        assert_eq!(
            owned["permissions.deny"],
            vec![json!("Read(**/.env)"), json!("Edit(**/.env)"), json!("Edit(/gen/**)")]
        );
        assert!(owned["permissions.ask"].contains(&json!("Bash(git push *)")));
        assert!(owned["permissions.ask"].contains(&json!("PowerShell(git push)")));
        assert_eq!(owned["hooks.Stop"][0]["hooks"][0]["timeout"], 60);
    }
}
