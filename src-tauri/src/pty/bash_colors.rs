use std::path::Path;
use std::sync::OnceLock;

// Plain ANSI palette indices rather than fixed RGB, so the prompt follows
// whichever Termini theme is active.
const PS1_BASE: &str = r"\[\e[32m\]\u@\h\[\e[0m\]:\[\e[34m\]\w";
const PS1_GIT_BRANCH: &str = r#"\[\e[35m\]$(b=$(git symbolic-ref --short HEAD 2>/dev/null) && echo " ($b)")"#;
const PS1_END: &str = r"\[\e[0m\]\$ ";

/// Extra environment for a new bash session when the user opted into
/// colored bash. Delivered as env vars rather than an --rcfile: bash only
/// uses an inherited PS1 until a startup file sets its own, so a prompt the
/// user already defines in ~/.bashrc still wins with no detection needed.
/// BASH_SILENCE_DEPRECATION_WARNING has to be in the environment before
/// bash starts — setting it from a startup file is too late on macOS.
pub fn bash_color_env(shell_path: &str, enabled: bool) -> Vec<(&'static str, String)> {
    let is_bash = Path::new(shell_path).file_name().is_some_and(|name| name == "bash");
    if !enabled || !is_bash {
        return vec![];
    }
    let branch = if git_available() { PS1_GIT_BRANCH } else { "" };
    vec![
        ("PS1", format!("{PS1_BASE}{branch}{PS1_END}")),
        ("CLICOLOR", "1".into()),
        ("BASH_SILENCE_DEPRECATION_WARNING", "1".into()),
    ]
}

/// On macOS without the Command Line Tools, /usr/bin/git is a stub that
/// pops an install dialog every time it runs — which, embedded in PS1, would
/// be on every single prompt. `xcode-select -p` answers the same question
/// without triggering that dialog.
fn git_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("/usr/bin/xcode-select")
                .arg("-p")
                .output()
                .is_ok_and(|output| output.status.success())
        }
        #[cfg(not(target_os = "macos"))]
        {
            std::env::var_os("PATH")
                .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join("git").is_file()))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_applies_to_bash_when_enabled() {
        assert!(bash_color_env("/bin/bash", false).is_empty());
        assert!(bash_color_env("/bin/zsh", true).is_empty());
        assert!(bash_color_env(r"C:\Program Files\Git\bin\bash.exe", true).is_empty());
        let keys: Vec<_> = bash_color_env("/usr/local/bin/bash", true)
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, ["PS1", "CLICOLOR", "BASH_SILENCE_DEPRECATION_WARNING"]);
    }

    #[test]
    fn prompt_matches_the_shape_verified_against_real_bash() {
        let env = bash_color_env("/bin/bash", true);
        let ps1 = &env[0].1;
        assert!(ps1.starts_with(r"\[\e[32m\]\u@\h\[\e[0m\]:\[\e[34m\]\w"));
        assert!(ps1.ends_with(r"\[\e[0m\]\$ "));
    }
}
