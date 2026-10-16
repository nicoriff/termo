//! Copilot CLI's repository hooks, `.github/hooks/`: a file of Termo's own
//! with the checks as an `agentStop` hook, which sends the agent back with a
//! reason when one fails, and the command and path rules as a `preToolUse`
//! hook (see guard.rs): Copilot CLI has no repository rule of its own for
//! either. Commands allowed without asking reach it as nothing: a hook can
//! refuse or ask, and Termo doesn't make one approve.

use serde_json::json;

use super::checks;
use super::guard;
use super::model::{Agent, Harness};

pub const FILE: &str = ".github/hooks/termo-harness.json";

pub fn applies(spec: &Harness) -> bool {
    spec.targets(Agent::Copilot)
}

/// The hook file, all Termo's; `None` when there's nothing to hook. Copilot
/// takes one command per shell; both gates read the same in either.
pub fn hooks(spec: &Harness) -> Option<String> {
    if !applies(spec) {
        return None;
    }
    let mut hooks = serde_json::Map::new();
    if guard::for_copilot(spec) {
        let command = guard::command(guard::Caller::Copilot);
        hooks.insert(
            "preToolUse".into(),
            json!([{ "type": "command", "bash": command, "powershell": command, "timeoutSec": 30 }]),
        );
    }
    if !spec.checks.is_empty() {
        let command = checks::command();
        hooks.insert(
            "agentStop".into(),
            json!([{
                "type": "command",
                "bash": command,
                "powershell": command,
                "timeoutSec": checks::timeout_sec(&spec.checks),
            }]),
        );
    }
    if hooks.is_empty() {
        return None;
    }
    let root = json!({ "version": 1, "hooks": hooks });
    Some(serde_json::to_string_pretty(&root).unwrap_or_default() + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::model::Check;

    #[test]
    fn checks_become_an_agent_stop_hook_for_both_shells() {
        let spec = Harness { checks: vec![Check { command: "npm test".into(), timeout_sec: None }], ..Harness::default() };
        let text = hooks(&spec).unwrap();
        let root: serde_json::Value = serde_json::from_str(&text).unwrap();
        let hook = &root["hooks"]["agentStop"][0];
        assert_eq!(hook["bash"], checks::command());
        assert_eq!(hook["powershell"], hook["bash"]);
        assert_eq!(hooks(&Harness::default()), None);
        assert!(root["hooks"].get("preToolUse").is_none());
    }

    #[test]
    fn rules_become_a_pre_tool_use_hook() {
        use crate::harness::model::{CommandRule, Decision};
        let spec = Harness {
            commands: vec![CommandRule { prefix: "git push --force".into(), decision: Decision::Deny }],
            ..Harness::default()
        };
        let root: serde_json::Value = serde_json::from_str(&hooks(&spec).unwrap()).unwrap();
        assert_eq!(root["hooks"]["preToolUse"][0]["powershell"], guard::command(guard::Caller::Copilot));
        assert!(root["hooks"].get("agentStop").is_none());
    }
}
