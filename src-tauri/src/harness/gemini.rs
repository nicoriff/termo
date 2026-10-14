//! Gemini CLI's project settings, `.gemini/settings.json` (read once the
//! folder is trusted): forbidden commands out of its shell tool, allowed
//! ones let through without asking, the checks as an `AfterAgent` hook, and
//! AGENTS.md among the files it reads for context. It has no "ask" rule nor
//! one for paths: those reach it as text.

use serde_json::{json, Map, Value};

use super::checks;
use super::instructions;
use super::model::{Agent, Decision, Harness, Owned};

pub const FILE: &str = ".gemini/settings.json";

pub fn applies(spec: &Harness) -> bool {
    spec.targets(Agent::Gemini)
}

fn shell(prefix: &str) -> Value {
    json!(format!("run_shell_command({prefix})"))
}

pub fn desired(spec: &Harness, current: &Map<String, Value>) -> Owned {
    let mut owned = Owned::new();
    let mut push = |key: &str, value: Value| owned.entry(key.to_string()).or_default().push(value);
    for rule in &spec.commands {
        match rule.decision {
            Decision::Deny => push("tools.exclude", shell(&rule.prefix)),
            Decision::Allow => push("tools.allowed", shell(&rule.prefix)),
            Decision::Ask => {}
        }
    }
    if let Some(command) = checks::posix_exit_code(&spec.checks) {
        // Gemini CLI's timeouts are in milliseconds.
        let timeout = u64::from(checks::timeout_sec(&spec.checks)) * 1000;
        push("hooks.AfterAgent", json!({ "hooks": [{ "type": "command", "command": command, "timeout": timeout }] }));
    }
    if instructions::agents_section(spec).is_some() {
        // Without the setting it reads GEMINI.md alone, so that one stays in
        // too; a list of the user's only gains AGENTS.md; a single name set
        // by them is theirs to change.
        match current.get("context").and_then(|context| context.get("fileName")) {
            None => {
                push("context.fileName", json!(instructions::AGENTS_MD));
                push("context.fileName", json!("GEMINI.md"));
            }
            Some(Value::Array(_)) => push("context.fileName", json!(instructions::AGENTS_MD)),
            Some(_) => {}
        }
    }
    owned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::json_merge;
    use crate::harness::model::{Check, CommandRule};

    fn spec() -> Harness {
        Harness {
            commands: vec![
                CommandRule { prefix: "rm -rf".into(), decision: Decision::Deny },
                CommandRule { prefix: "git push".into(), decision: Decision::Ask },
                CommandRule { prefix: "npm test".into(), decision: Decision::Allow },
            ],
            checks: vec![Check { command: "npm test".into(), timeout_sec: Some(30) }],
            ..Harness::default()
        }
    }

    #[test]
    fn commands_checks_and_context() {
        let owned = desired(&spec(), &Map::new());
        assert_eq!(owned["tools.exclude"], vec![json!("run_shell_command(rm -rf)")]);
        assert_eq!(owned["tools.allowed"], vec![json!("run_shell_command(npm test)")]);
        assert_eq!(owned["hooks.AfterAgent"][0]["hooks"][0]["timeout"], 30_000);
        assert_eq!(owned["context.fileName"], vec![json!("AGENTS.md"), json!("GEMINI.md")]);
    }

    #[test]
    fn the_users_context_files_are_respected() {
        let list = json_merge::parse("s", Some(r#"{ "context": { "fileName": ["CONTEXT.md"] } }"#)).unwrap();
        assert_eq!(desired(&spec(), &list)["context.fileName"], vec![json!("AGENTS.md")]);
        let single = json_merge::parse("s", Some(r#"{ "context": { "fileName": "CONTEXT.md" } }"#)).unwrap();
        assert!(!desired(&spec(), &single).contains_key("context.fileName"));
    }
}
