//! Starting an agent CLI as a console's own process, instead of typing its
//! command into a shell: the tab shows the agent straight away, with no
//! shell starting first. When it exits the frontend starts the user's shell
//! in the same tab (see TerminalInstance), so leaving the agent still drops
//! you at a prompt in that folder.
//!
//! On Windows npm installs the Node CLIs (codex, gemini, copilot) as `.cmd`
//! shims, which only cmd.exe runs. Elsewhere they're scripts run by `node`
//! through `env`, and a GUI app's PATH often lacks it, so they go through
//! the user's login shell — which only sets PATH up and then gives way.

use serde::Deserialize;

use crate::i18n::{tr, Msg};
use crate::marketplace::agent_cli;
use crate::marketplace::model::AgentTarget;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentLaunch {
    pub tool: String,
    /// What follows the command, e.g. resuming a conversation.
    #[serde(default)]
    pub args: Vec<String>,
}

fn target(tool: &str) -> Option<AgentTarget> {
    Some(match tool {
        "claude" => AgentTarget::Claude,
        "codex" => AgentTarget::Codex,
        "gemini" => AgentTarget::Gemini,
        "copilot" => AgentTarget::Copilot,
        _ => return None,
    })
}

/// The arguments go on a command line some shell reads (cmd.exe, sh), so
/// only plain words: subcommands, flags and session ids.
fn is_plain_word(arg: &str) -> bool {
    !arg.is_empty() && arg.len() <= 200 && arg.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[cfg(not(windows))]
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// The program and arguments that start the agent. `login_shell` is what
/// sets PATH up outside Windows.
pub fn command(launch: &AgentLaunch, login_shell: &str) -> Result<(String, Vec<String>), String> {
    command_with(launch, login_shell, &[])
}

/// The same with `extra` arguments first: Termo's own (see
/// agents/console_bridge.rs), built here rather than typed by anyone, so
/// they skip the plain-word check the frontend's arguments get.
pub fn command_with(launch: &AgentLaunch, login_shell: &str, extra: &[String]) -> Result<(String, Vec<String>), String> {
    let target = target(&launch.tool).ok_or_else(|| tr(Msg::InvalidComponentName(&launch.tool)))?;
    if let Some(bad) = launch.args.iter().find(|arg| !is_plain_word(arg)) {
        return Err(tr(Msg::InvalidComponentName(bad)));
    }
    let path = agent_cli::require(target, None)?;
    #[cfg(windows)]
    if is_script(&path) && path.to_string_lossy().contains(CMD_SYNTAX) {
        return Err(tr(Msg::AgentLaunchUnsafePath(&path.to_string_lossy())));
    }
    let args: Vec<String> = extra.iter().chain(&launch.args).cloned().collect();
    Ok(build(&path, &args, login_shell))
}

/// Characters cmd.exe reads as its own syntax even in a path; an install
/// folder holding one would turn part of the path into another command.
#[cfg(windows)]
const CMD_SYNTAX: &[char] = &['&', '|', '<', '>', '^', '%', '"', '!'];

#[cfg(windows)]
fn build(path: &std::path::Path, args: &[String], _login_shell: &str) -> (String, Vec<String>) {
    let script = is_script(path);
    let path = path.to_string_lossy().into_owned();
    if !script {
        return (path, args.to_vec());
    }
    let comspec = std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into());
    // `/d`: no AutoRun commands from the registry before the agent.
    let mut cmd_args = vec!["/d".to_string(), "/c".to_string(), path];
    cmd_args.extend(args.iter().cloned());
    (comspec, cmd_args)
}

/// An npm shim, which only cmd.exe runs.
#[cfg(windows)]
fn is_script(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
}

#[cfg(not(windows))]
fn build(path: &std::path::Path, args: &[String], login_shell: &str) -> (String, Vec<String>) {
    let mut line = format!("exec {}", sh_quote(&path.to_string_lossy()));
    for arg in args {
        line.push(' ');
        line.push_str(&sh_quote(arg));
    }
    // Interactive too: that's where nvm and friends usually put node on PATH.
    (login_shell.to_string(), vec!["-l".into(), "-i".into(), "-c".into(), line])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_words_follow_the_command() {
        assert!(is_plain_word("--resume"));
        assert!(is_plain_word("0b739325-ae54-4319-9ffa-8ff83f70114e"));
        for bad in ["", "a b", "x&calc", "$(id)", "a;b", "a|b", "'q'", "%PATH%"] {
            assert!(!is_plain_word(bad), "{bad:?}");
        }
        let launch = AgentLaunch { tool: "claude".into(), args: vec!["x&calc".into()] };
        assert!(command(&launch, "/bin/zsh").is_err());
        let unknown = AgentLaunch { tool: "calc".into(), args: Vec::new() };
        assert!(command(&unknown, "/bin/zsh").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn an_npm_shim_runs_through_cmd_and_an_exe_on_its_own() {
        let (program, args) = build(std::path::Path::new(r"D:\nodejs\codex.cmd"), &["resume".into(), "abc".into()], "");
        assert!(program.to_ascii_lowercase().ends_with("cmd.exe"));
        assert_eq!(args, ["/d", "/c", r"D:\nodejs\codex.cmd", "resume", "abc"]);

        let (program, args) = build(std::path::Path::new(r"C:\Users\me\.local\bin\claude.exe"), &[], "");
        assert_eq!(program, r"C:\Users\me\.local\bin\claude.exe");
        assert!(args.is_empty());

        // cmd.exe would read these as its own syntax inside the path.
        assert!(is_script(std::path::Path::new(r"C:\a&calc\codex.cmd")));
        assert!(r"C:\a&calc\codex.cmd".contains(CMD_SYNTAX));
        assert!(!r"D:\nodejs\codex.cmd".contains(CMD_SYNTAX));
    }

    #[cfg(not(windows))]
    #[test]
    fn a_script_runs_through_the_login_shell() {
        let (program, args) = build(std::path::Path::new("/opt/my tools/codex"), &["resume".into()], "/bin/zsh");
        assert_eq!(program, "/bin/zsh");
        assert_eq!(args, ["-l", "-i", "-c", "exec '/opt/my tools/codex' 'resume'"]);
    }
}
