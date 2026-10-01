//! Windows consoles in UTF-8 (Preferences → Platform, on by default). A
//! console starts in the system's legacy code page (850, 437…), so programs
//! that write UTF-8 without asking for it — Python, Node tools, git under
//! PowerShell 5.1 — show "Ã±" for "ñ". ConPTY already speaks UTF-8 to Termo;
//! this makes the console in between agree: `chcp 65001` for CMD, and the
//! console's and PowerShell's own encodings for PowerShell/pwsh. Elsewhere
//! (Git Bash, WSL, macOS, Linux) the locale already is UTF-8.

/// No BOM: `$OutputEncoding` is what PowerShell pipes into native programs,
/// and a BOM there would reach them as text. Single quotes and no double
/// ones, for the same Windows command line reason as cwd_report's prompt
/// wrapper.
const POWERSHELL_UTF8: &str =
    "[Console]::InputEncoding = [Console]::OutputEncoding = $OutputEncoding = [Text.UTF8Encoding]::new($false)";

/// Runs before CMD's prompt; `>nul` keeps "Active code page: 65001" out of
/// the console.
const CMD_UTF8: &str = "chcp 65001>nul";

fn stem(shell_path: &str) -> String {
    std::path::Path::new(shell_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Windows only. CMD keeps arguments it was given to run something else
/// (`/C`, `/K`) to itself: those already decide what it runs.
pub fn apply(enabled: bool, shell_path: &str, args: &mut Vec<String>) {
    if !cfg!(windows) || !enabled {
        return;
    }
    match stem(shell_path).as_str() {
        "powershell" | "pwsh" => super::oh_my_posh::append_powershell_command(args, POWERSHELL_UTF8),
        "cmd" if !args.iter().any(|arg| arg.eq_ignore_ascii_case("/c") || arg.eq_ignore_ascii_case("/k")) => {
            args.extend(["/K".to_string(), CMD_UTF8.to_string()]);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn powershell_and_cmd_start_in_utf8() {
        let mut args = Vec::new();
        apply(true, r"C:\Program Files\PowerShell\7\pwsh.exe", &mut args);
        assert!(args.iter().any(|arg| arg.contains("[Text.UTF8Encoding]::new($false)")));

        let mut args = Vec::new();
        apply(true, r"C:\Windows\System32\cmd.exe", &mut args);
        assert_eq!(args, ["/K", "chcp 65001>nul"]);
    }

    #[test]
    #[cfg(windows)]
    fn a_cmd_already_told_what_to_run_is_left_alone() {
        let mut args = vec!["/C".to_string(), "build.bat".to_string()];
        apply(true, r"C:\Windows\System32\cmd.exe", &mut args);
        assert_eq!(args, ["/C", "build.bat"]);
    }

    #[test]
    fn turned_off_or_other_shells_get_nothing() {
        let mut args = Vec::new();
        apply(false, r"C:\Windows\System32\cmd.exe", &mut args);
        apply(true, "/bin/bash", &mut args);
        apply(true, r"C:\Program Files\Git\bin\bash.exe", &mut args);
        assert!(args.is_empty());
    }

    #[test]
    fn escapes_the_windows_command_line() {
        assert!(!POWERSHELL_UTF8.contains('"') && !CMD_UTF8.contains('"'));
    }
}
