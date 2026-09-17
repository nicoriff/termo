//! Makes a console report its working directory, so Termini knows where each
//! shell actually is — not just where it was opened. OSC 7 is the convention
//! every terminal uses for this (Windows Terminal, iTerm, VS Code): the shell
//! prints `ESC ] 7 ; file://<host>/<path> BEL` on each prompt, and the terminal
//! keeps the last one it saw.
//!
//! Applied **after** every other startup command, because PowerShell's prompt
//! is a single function and whoever defines it last wins — Oh My Posh replaces
//! it wholesale, so a wrapper installed before its init would be thrown away.
//! This wrapper instead calls whatever prompt is in place at that moment.

use std::path::Path;

/// Single quotes only, and no interpolation: this travels as one argument of a
/// Windows command line, where embedded double quotes get mangled. The other
/// injected commands here (PSReadLine, Oh My Posh) avoid them for the same
/// reason — `escapes_the_windows_command_line` guards it.
const POWERSHELL_PROMPT: &str = r"if (-not $global:__terminiCwdPrompt) { $global:__terminiCwdPrompt = $function:prompt; function global:prompt { $inner = & $global:__terminiCwdPrompt; if ($PWD.Provider.Name -eq 'FileSystem') { [string][char]27 + ']7;file:///' + ($PWD.ProviderPath -replace '\\','/') + [string][char]7 + $inner } else { $inner } } }";

/// Delivered as an env var for the same reason bash_colors's PS1 is: bash uses
/// an inherited PROMPT_COMMAND until a startup file sets its own, so a user who
/// defines one in ~/.bashrc still wins, with no detection needed.
const BASH_PROMPT_COMMAND: &str = r#"printf '\033]7;file://%s\007' "$PWD""#;

fn stem_is(shell_path: &str, names: &[&str]) -> bool {
    Path::new(shell_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| names.iter().any(|name| stem.eq_ignore_ascii_case(name)))
}

/// Appends whatever this shell needs to report its directory. Unknown shells
/// (and zsh, which configures its prompt through its own startup file) are
/// left untouched — cwd tracking simply stays off for them.
pub fn apply(shell_path: &str, args: &mut Vec<String>, env: &mut Vec<(&'static str, String)>) {
    if stem_is(shell_path, &["powershell", "pwsh"]) {
        super::oh_my_posh::append_powershell_command(args, POWERSHELL_PROMPT);
    } else if stem_is(shell_path, &["bash"]) {
        env.push(("PROMPT_COMMAND", BASH_PROMPT_COMMAND.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_gets_a_prompt_wrapper_and_bash_an_env_var() {
        let mut args = Vec::new();
        let mut env = Vec::new();
        apply(r"C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe", &mut args, &mut env);
        assert!(args.iter().any(|arg| arg.contains("__terminiCwdPrompt")));
        assert!(env.is_empty());

        let mut args = Vec::new();
        let mut env = Vec::new();
        apply("/bin/bash", &mut args, &mut env);
        assert!(args.is_empty());
        assert_eq!(env[0].0, "PROMPT_COMMAND");
    }

    /// The wrapper must call the prompt that exists when it runs, so Oh My
    /// Posh's (installed just before it) survives.
    #[test]
    fn the_wrapper_calls_the_existing_prompt() {
        assert!(POWERSHELL_PROMPT.contains("$function:prompt"));
        assert!(POWERSHELL_PROMPT.contains("& $global:__terminiCwdPrompt"));
    }

    /// A double quote in here comes out mangled on the far side of a Windows
    /// command line, which is how this snippet reaches the shell.
    #[test]
    fn escapes_the_windows_command_line() {
        assert!(!POWERSHELL_PROMPT.contains('"'));
    }

    #[test]
    fn other_shells_are_left_alone() {
        let mut args = Vec::new();
        let mut env = Vec::new();
        apply("/bin/zsh", &mut args, &mut env);
        apply("/usr/bin/fish", &mut args, &mut env);
        assert!(args.is_empty() && env.is_empty());
    }
}
