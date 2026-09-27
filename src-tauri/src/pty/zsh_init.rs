use std::fs;
use std::path::Path;

use tauri::Manager;

/// Tab lists the completions, a second Tab opens a menu navigable with the
/// arrow keys (Shift+Tab goes back) — zsh's native equivalent of
/// PSReadLine's MenuComplete, no plugin needed.
pub const MENU_COMPLETE_LINES: &[&str] = &[
    "zmodload zsh/complist",
    // `menu select` needs the completion system; only start it if the
    // user's own startup files haven't already.
    "(( $+functions[compdef] )) || { autoload -Uz compinit && compinit -i }",
    "zstyle ':completion:*' menu select",
    "bindkey -M menuselect '^[[Z' reverse-menu-complete",
];

/// zsh's half of cwd_report (which covers bash and PowerShell): report the
/// directory on every prompt, plus once up front so a console that never
/// changes directory still reports one. macOS only ships this through
/// /etc/zshrc_Apple_Terminal, which is gated on the TERM_PROGRAM that
/// sessions deliberately don't inherit (see session.rs), so without this zsh
/// consoles never report where they are.
pub const CWD_REPORT_LINES: &[&str] = &[
    "autoload -Uz add-zsh-hook",
    r#"_termo_report_cwd() { printf '\033]7;file://%s\007' "$PWD" }"#,
    "add-zsh-hook precmd _termo_report_cwd",
    "_termo_report_cwd",
];

pub fn is_zsh(shell_path: &str) -> bool {
    Path::new(shell_path).file_stem().is_some_and(|stem| stem == "zsh")
}

/// zsh has no --rcfile: point ZDOTDIR at a wrapper dir whose startup files
/// source the user's real ones, restore ZDOTDIR, then run `lines`.
pub fn apply(app: &tauri::AppHandle, lines: &[String], env: &mut Vec<(&'static str, String)>) -> anyhow::Result<()> {
    let dir = app.path().app_data_dir()?.join("zsh-init");
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join(".zshenv"),
        "[ -f \"${TERMO_USER_ZDOTDIR:-$HOME}/.zshenv\" ] && . \"${TERMO_USER_ZDOTDIR:-$HOME}/.zshenv\"\n",
    )?;
    let mut zshrc = String::from(
        "ZDOTDIR=\"${TERMO_USER_ZDOTDIR:-$HOME}\"\n\
         unset TERMO_USER_ZDOTDIR\n\
         [ -f \"$ZDOTDIR/.zshrc\" ] && . \"$ZDOTDIR/.zshrc\"\n",
    );
    for line in lines {
        zshrc.push_str(line);
        zshrc.push('\n');
    }
    fs::write(dir.join(".zshrc"), zshrc)?;

    if let Some(user_zdotdir) = std::env::var_os("ZDOTDIR") {
        env.push(("TERMO_USER_ZDOTDIR", user_zdotdir.to_string_lossy().into_owned()));
    }
    env.push(("ZDOTDIR", dir.to_string_lossy().into_owned()));
    Ok(())
}
