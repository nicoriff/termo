//! The "done" gate: the repository's checks, run when an agent says it has
//! finished, sending it back to work when one fails. What's written here
//! goes into the repository's files and is read by the agents, so it's in
//! English whatever language Termo is in.

use super::model::Check;

/// For a check that doesn't say how long it may take.
pub const DEFAULT_TIMEOUT_SEC: u32 = 300;

/// How long the whole gate may take: every check, one after the other.
pub fn timeout_sec(checks: &[Check]) -> u32 {
    checks.iter().map(|check| check.timeout_sec.unwrap_or(DEFAULT_TIMEOUT_SEC)).sum()
}

/// Skips the checks when the working tree has nothing new (a turn that only
/// answered a question) — from the repository's root, wherever the agent is.
const POSIX_PREAMBLE: &str = "cd \"$(git rev-parse --show-toplevel)\" || exit 0; \
if git diff --quiet HEAD 2>/dev/null && [ -z \"$(git ls-files --others --exclude-standard)\" ]; then exit 0; fi";

fn posix_chain(checks: &[Check]) -> String {
    checks.iter().map(|check| format!("( {} )", check.command)).collect::<Vec<_>>().join(" && ")
}

/// For agents that take exit code 2 as "not done" and hand what was printed
/// on stderr to the model (Claude Code, Codex, Gemini CLI): everything the
/// checks print goes there, so the agent sees why.
pub fn posix_exit_code(checks: &[Check]) -> Option<String> {
    (!checks.is_empty()).then(|| format!("{POSIX_PREAMBLE}; {{ {}; }} 1>&2 || exit 2", posix_chain(checks)))
}

/// Why the agent was sent back, for agents that only take a reason as JSON
/// on stdout: the checks' own output is left out, since the agent can run
/// them again itself.
pub fn reason(checks: &[Check]) -> String {
    let list = checks.iter().map(|check| format!("`{}`", check.command)).collect::<Vec<_>>().join(", ");
    format!("The repository's checks failed ({list}). Run them, fix what fails, and finish again.")
}

/// For agents that read a JSON decision on stdout (Copilot CLI's
/// `agentStop`): nothing but that JSON may be printed there.
pub fn posix_json(checks: &[Check], decision_key: &str, block: &str) -> Option<String> {
    if checks.is_empty() {
        return None;
    }
    let json = serde_json::json!({ decision_key: block, "reason": reason(checks) }).to_string();
    let quoted = format!("'{}'", json.replace('\'', r"'\''"));
    Some(format!("{POSIX_PREAMBLE}; {{ {}; }} >/dev/null 2>&1 || echo {quoted}", posix_chain(checks)))
}

/// The same in PowerShell, for Copilot CLI on Windows.
pub fn powershell_json(checks: &[Check], decision_key: &str, block: &str) -> Option<String> {
    if checks.is_empty() {
        return None;
    }
    let json = serde_json::json!({ decision_key: block, "reason": reason(checks) }).to_string();
    let quoted = format!("'{}'", json.replace('\'', "''"));
    let runs = checks
        .iter()
        .map(|check| {
            let command = check.command.replace('\'', "''");
            format!("cmd /c '{command}' *> $null; if ($LASTEXITCODE -ne 0) {{ Write-Output {quoted}; exit 0 }}")
        })
        .collect::<Vec<_>>()
        .join("; ");
    Some(format!(
        "Set-Location (git rev-parse --show-toplevel); \
git diff --quiet HEAD 2>$null; if ($LASTEXITCODE -eq 0 -and -not (git ls-files --others --exclude-standard)) {{ exit 0 }}; {runs}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checks(commands: &[&str]) -> Vec<Check> {
        commands.iter().map(|command| Check { command: command.to_string(), timeout_sec: None }).collect()
    }

    #[test]
    fn no_checks_no_gate() {
        assert_eq!(posix_exit_code(&[]), None);
        assert_eq!(posix_json(&[], "decision", "block"), None);
        assert_eq!(powershell_json(&[], "decision", "block"), None);
    }

    #[test]
    fn checks_run_in_order_and_fail_with_exit_code_two() {
        let gate = posix_exit_code(&checks(&["npm run check", "cargo test"])).unwrap();
        assert!(gate.ends_with("{ ( npm run check ) && ( cargo test ); } 1>&2 || exit 2"));
    }

    #[test]
    fn the_json_reason_survives_shell_quoting() {
        let gate = posix_json(&checks(&["echo 'hi'"]), "decision", "block").unwrap();
        let quoted = gate.split_once("|| echo ").unwrap().1;
        // Undo POSIX single quoting: '...'\''...'.
        let unquoted = quoted.trim_matches('\'').replace(r"'\''", "'");
        let value: serde_json::Value = serde_json::from_str(&unquoted).unwrap();
        assert_eq!(value["decision"], "block");
        assert!(value["reason"].as_str().unwrap().contains("`echo 'hi'`"));
    }

    #[test]
    fn the_timeout_adds_up() {
        let mut list = checks(&["a", "b"]);
        list[0].timeout_sec = Some(60);
        assert_eq!(timeout_sec(&list), 60 + DEFAULT_TIMEOUT_SEC);
    }
}
