//! Makes Ctrl+Backspace kill the previous word in PowerShell, as it does in
//! Windows Terminal. xterm encodes the key as ^H, and ConPTY hands that to
//! the console as Ctrl+H, which PSReadLine binds to deleting one character.
//! Windows Terminal sidesteps it with win32-input-mode, but the ConPTY that
//! ships with Windows 10 drops those sequences, so the fix has to live on
//! the shell's side: Ctrl+H becomes BackwardKillWord. Plain Backspace is
//! unaffected — xterm sends it as DEL, which ConPTY delivers as Backspace.

/// Single quotes only, for the same Windows command line reason as
/// cwd_report's prompt wrapper.
const POWERSHELL_KEYS: &str = "Set-PSReadLineKeyHandler -Chord 'Ctrl+h' -Function BackwardKillWord";

pub(super) fn is_powershell(shell_path: &str) -> bool {
    std::path::Path::new(shell_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case("powershell") || stem.eq_ignore_ascii_case("pwsh"))
}

/// Windows only: elsewhere the PTY passes ^H through untouched and pwsh
/// already reads it as its own Backspace binding.
pub fn apply(shell_path: &str, args: &mut Vec<String>) {
    if cfg!(windows) && is_powershell(shell_path) {
        super::oh_my_posh::append_powershell_command(args, POWERSHELL_KEYS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn powershell_gets_the_key_handler() {
        let mut args = Vec::new();
        apply(r"C:\Program Files\PowerShell\7\pwsh.exe", &mut args);
        assert!(args.iter().any(|arg| arg.contains("BackwardKillWord")));
    }

    #[test]
    fn other_shells_are_left_alone() {
        let mut args = Vec::new();
        apply(r"C:\Windows\System32\cmd.exe", &mut args);
        apply("/bin/bash", &mut args);
        assert!(args.is_empty());
    }

    #[test]
    fn escapes_the_windows_command_line() {
        assert!(!POWERSHELL_KEYS.contains('"'));
    }
}
