//! Copilot CLI's repository hooks, `.github/hooks/`: a file of Termo's own
//! with the checks as an `agentStop` hook, which sends the agent back with a
//! reason when one fails. Copilot CLI has no repository rule for paths or
//! commands: those reach it as text, in AGENTS.md.

use serde_json::json;

use super::checks;
use super::model::{Agent, Harness};

pub const FILE: &str = ".github/hooks/termo-harness.json";

pub fn applies(spec: &Harness) -> bool {
    spec.targets(Agent::Copilot)
}

/// The hook file, all Termo's; `None` when there are no checks. Copilot
/// takes one command per shell; the gate's reads the same in both.
pub fn hooks(spec: &Harness) -> Option<String> {
    if !applies(spec) || spec.checks.is_empty() {
        return None;
    }
    let command = checks::command(checks::Mode::Json);
    let root = json!({
        "version": 1,
        "hooks": {
            "agentStop": [{
                "type": "command",
                "bash": command,
                "powershell": command,
                "timeoutSec": checks::timeout_sec(&spec.checks),
            }],
        },
    });
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
        assert_eq!(hook["bash"], checks::command(checks::Mode::Json));
        assert_eq!(hook["powershell"], hook["bash"]);
        assert_eq!(hooks(&Harness::default()), None);
    }
}
