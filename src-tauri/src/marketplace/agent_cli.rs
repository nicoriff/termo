//! Running the agents' own CLIs on the user's behalf.
//!
//! Installing goes through `claude`/`codex` rather than through Termo
//! writing their config files: the CLI is what clones the repo, resolves the
//! scope, updates its own lockfiles and enforces the marketplace's
//! command-approval model. Reimplementing that here would mean tracking an
//! undocumented on-disk format that moves between releases, and quietly
//! corrupting it the first time it changed.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::i18n::{tr, Msg};
use crate::pty::bin_search;

use super::model::AgentTarget;

/// The agent's executable, or None when it isn't installed. `override_path`
/// comes from the user's config, for an install this search can't reach.
pub fn locate(target: AgentTarget, override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = override_path.map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    if let Some(path) = bin_search::find_binary(target.binary_stem()) {
        return Some(path);
    }
    // Claude Code's local installer puts it outside every PATH dir.
    if target == AgentTarget::Claude {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let local = home.join(".claude").join("local").join("claude");
            if local.is_file() {
                return Some(local);
            }
        }
    }
    None
}

pub fn require(target: AgentTarget, override_path: Option<&str>) -> Result<PathBuf, String> {
    locate(target, override_path).ok_or_else(|| match target {
        AgentTarget::Claude => tr(Msg::ClaudeCliNotFound),
        AgentTarget::Codex => tr(Msg::CodexCliNotFound),
        AgentTarget::Gemini => tr(Msg::GeminiCliNotFound),
        AgentTarget::Copilot => tr(Msg::CopilotCliNotFound),
    })
}

/// How much of a failing command's stderr is worth showing: enough for the
/// CLI's own explanation, short of pasting a stack trace into the panel.
const MAX_STDERR: usize = 2000;

/// Runs `bin` with `args` and returns its stdout.
///
/// Two deliberate choices: the arguments go as a vector, never through a
/// shell — a plugin name is remote-controlled text and must never be able to
/// become shell syntax — and stdin is closed, so a CLI that decides to ask
/// something gets an immediate EOF and exits, instead of blocking this
/// thread forever behind a prompt nobody can see or answer.
pub fn run(bin: &PathBuf, args: &[&str]) -> Result<String, String> {
    let output = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Colour codes would end up rendered as literal escapes in the panel.
        .env("NO_COLOR", "1")
        .output()
        .map_err(|err| tr(Msg::MarketplaceCommandFailed(&err.to_string())))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    let detail = if detail.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        detail.chars().take(MAX_STDERR).collect()
    };
    Err(tr(Msg::MarketplaceCommandFailed(&detail)))
}

pub struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Like `run`, but hands back a failing command's output instead of turning
/// it into an error: installing has an expected non-success path (the
/// marketplace declares a command that a person has to approve first) whose
/// details live in that output.
pub fn run_capture(bin: &PathBuf, args: &[&str]) -> Result<CommandOutput, String> {
    let output = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .output()
        .map_err(|err| tr(Msg::MarketplaceCommandFailed(&err.to_string())))?;

    Ok(CommandOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(MAX_STDERR)
            .collect(),
    })
}

pub fn run_json<T: serde::de::DeserializeOwned>(bin: &PathBuf, args: &[&str]) -> Result<T, String> {
    let stdout = run(bin, args)?;
    serde_json::from_str(&stdout).map_err(|err| tr(Msg::MarketplaceCommandFailed(&err.to_string())))
}

/// Names from a remote catalog end up as command-line arguments and, inside
/// the CLI, as path segments. Only the shapes the two ecosystems actually
/// use are accepted — scoped npm-style names (`@scope/pkg`), reverse-DNS MCP
/// ids (`ai.smithery/thing`) and plugin ids (`name@marketplace`) — and never
/// anything that could climb out of a directory or start an option.
pub fn is_valid_component_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.starts_with('-')
        && !name.contains("..")
        && !name.contains("//")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@' | '/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_names_the_catalogs_actually_use() {
        assert!(is_valid_component_name("42crunch-api-security-testing"));
        assert!(is_valid_component_name("box@claude-plugins-official"));
        assert!(is_valid_component_name("ai.smithery/Hint-Services-obsidian-github-mcp"));
        assert!(is_valid_component_name("@upstash/context7-mcp"));
    }

    #[test]
    fn rejects_names_that_could_escape_or_become_a_flag() {
        for name in [
            "",
            ".hidden",
            "--version",
            "../../etc/passwd",
            "a//b",
            "name with spaces",
            "name;rm -rf /",
            "$(whoami)",
            "name\nsecond",
        ] {
            assert!(!is_valid_component_name(name), "should reject {name:?}");
        }
    }

    #[test]
    fn a_missing_binary_is_reported_not_panicked() {
        let missing = PathBuf::from("/nonexistent/termo-test-cli");
        let err = run(&missing, &["--version"]).unwrap_err();
        assert!(!err.trim().is_empty());
    }

    #[test]
    fn an_override_that_does_not_exist_is_not_used() {
        assert!(locate(AgentTarget::Claude, Some("/nonexistent/claude")).is_none());
    }
}
