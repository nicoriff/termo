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

/// The hook file, all Termo's; `None` when there are no checks.
pub fn hooks(spec: &Harness) -> Option<String> {
    if !applies(spec) {
        return None;
    }
    let bash = checks::posix_json(&spec.checks, "decision", "block")?;
    let powershell = checks::powershell_json(&spec.checks, "decision", "block")?;
    let root = json!({
        "version": 1,
        "hooks": {
            "agentStop": [{
                "type": "command",
                "bash": bash,
                "powershell": powershell,
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
        assert!(hook["bash"].as_str().unwrap().contains("( npm test )"));
        assert!(hook["powershell"].as_str().unwrap().contains("cmd /c 'npm test'"));
        assert_eq!(hooks(&Harness::default()), None);
    }
}
