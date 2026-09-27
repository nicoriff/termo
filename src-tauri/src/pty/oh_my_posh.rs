use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tauri::Manager;

pub const DEFAULT_THEME: &str = "jandedobbeleer";

/// oh-my-posh bakes the right prompt's position into the line it renders —
/// `ESC[nX` then `ESC[nC`, measured against the terminal width at the time —
/// so a resize leaves the console reflowing stale cells, and a prompt that
/// filled 120 columns spills onto a second row at 70. Re-running the prompt
/// is the whole fix: oh-my-posh drops the right prompt when it no longer
/// fits and repositions it when it does. The erase-below goes first, to
/// clear whatever the reflow left on the rows underneath.
///
/// The redraw runs under UTF-8 and puts the console's encoding back after.
/// Windows PowerShell leaves the console on the OEM code page (850, 437…),
/// and while oh-my-posh sees to that for the prompts it draws itself, a
/// redraw from here goes out in the OEM page — which has no Nerd Font
/// icons, so every one of them came out as a `?`.
const POWERSHELL_REDRAW_HANDLER: &str = "Set-PSReadLineKeyHandler -Chord 'Ctrl+Alt+F12' -ScriptBlock { \
     $encoding = [Console]::OutputEncoding; [Console]::OutputEncoding = [Text.Encoding]::UTF8; \
     try { [Console]::Write([char]27 + '[J'); [Microsoft.PowerShell.PSConsoleReadLine]::InvokePrompt() } \
     finally { [Console]::OutputEncoding = $encoding } }";

/// What to send a PowerShell console to trigger the handler above. A chord
/// rather than a bare key so it can't collide with anything a user would
/// plausibly press themselves.
pub const POWERSHELL_REDRAW_KEY: &[u8] = b"\x1b[24;7~";

/// zsh's half of the same fix, and it needs nothing from Termo: SIGWINCH
/// already reaches the shell on every resize, so the trap alone is enough.
const ZSH_REDRAW_LINE: &str = "TRAPWINCH() { zle && { zle reset-prompt; zle -R } }";

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "oh-my-posh.exe"
    } else {
        "oh-my-posh"
    }
}

fn root_dir(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("oh-my-posh"))
}

/// Where Termo's own copy of the binary lives. Kept out of the user's
/// PATH dirs on purpose: installing never touches their shell setup.
pub fn managed_binary(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    Ok(root_dir(app)?.join("bin").join(exe_name()))
}

pub fn theme_file(app: &tauri::AppHandle, theme: &str) -> anyhow::Result<PathBuf> {
    Ok(root_dir(app)?.join("themes").join(format!("{theme}.omp.json")))
}

/// Theme names end up in a download URL and a file path, so only plain
/// official-style names (e.g. "agnoster.minimal") are accepted.
pub fn is_valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Same asset names the official install scripts download.
pub fn download_url() -> Option<String> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return None,
    };
    let ext = if os == "windows" { ".exe" } else { "" };
    Some(format!("https://cdn.ohmyposh.dev/releases/latest/posh-{os}-{arch}{ext}"))
}

/// The binary to use, and whether it's Termo's own managed copy. Prefers
/// the managed copy, then any existing install the user already has.
pub fn find(app: &tauri::AppHandle) -> Option<(PathBuf, bool)> {
    if let Ok(managed) = managed_binary(app) {
        if managed.is_file() {
            return Some((managed, true));
        }
    }
    // A GUI app's inherited PATH usually lacks the per-user and package
    // manager dirs, which is exactly where oh-my-posh tends to be installed
    // (see pty/bin_search.rs).
    super::bin_search::find_binary("oh-my-posh").map(|path| (path, false))
}

/// Hooks oh-my-posh's prompt into a new session of `shell_path`, if that
/// shell is supported; unsupported shells are left untouched. Init files are
/// written before `args`/`env` are modified, so a failed write leaves the
/// launch exactly as it was.
///
/// Returns the keys this session has to be sent after a resize to redraw its
/// prompt, for the shells that need telling (see POWERSHELL_REDRAW_HANDLER).
pub fn apply(
    app: &tauri::AppHandle,
    binary: &Path,
    managed: bool,
    theme: &str,
    shell_path: &str,
    args: &mut Vec<String>,
    env: &mut Vec<(&'static str, String)>,
    zsh_lines: &mut Vec<String>,
) -> anyhow::Result<Option<&'static [u8]>> {
    let config = config_arg(app, theme);
    let shell = Path::new(shell_path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut redraw_key = None;

    match shell.as_str() {
        "pwsh" | "powershell" => {
            append_powershell_command(args, &powershell_init(binary, &config));
            redraw_key = Some(POWERSHELL_REDRAW_KEY);
        }
        // Written into the shared zsh startup wrapper by the caller.
        "zsh" => {
            zsh_lines.push(format!(
                "eval \"$({} init zsh --config {})\"",
                sh_quote(&binary.to_string_lossy()),
                sh_quote(&config)
            ));
            zsh_lines.push(ZSH_REDRAW_LINE.to_string());
        }
        // Git Bash on Windows relies on its own /etc startup files, which
        // --rcfile would bypass, so bash is only hooked on macOS/Linux.
        "bash" if !cfg!(windows) && bash_supports_oh_my_posh(shell_path) => {
            let file = root_dir(app)?.join("init").join("bashrc");
            write_file(&file, &bash_init(binary, &config))?;
            args.splice(0..0, ["--rcfile".to_string(), file.to_string_lossy().into_owned()]);
        }
        _ => return Ok(None),
    }

    if managed {
        // So `oh-my-posh` itself is usable inside Termo sessions too.
        if let Some(bin_dir) = binary.parent() {
            let mut paths = vec![bin_dir.to_path_buf()];
            if let Some(existing) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&existing));
            }
            if let Ok(joined) = std::env::join_paths(paths) {
                env.push(("PATH", joined.to_string_lossy().into_owned()));
            }
        }
    }
    Ok(redraw_key)
}

/// A downloaded local theme file when there is one (fast, works offline);
/// otherwise the theme name, which oh-my-posh fetches and caches itself.
fn config_arg(app: &tauri::AppHandle, theme: &str) -> String {
    let theme = if is_valid_theme_name(theme) { theme } else { DEFAULT_THEME };
    match theme_file(app, theme) {
        Ok(file) if file.is_file() => file.to_string_lossy().into_owned(),
        _ => theme.to_string(),
    }
}

fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// `--eval` inlines the init script instead of having PowerShell run a
/// generated .ps1 file, which a Restricted execution policy (the Windows
/// PowerShell 5.1 default) would block.
fn powershell_init(binary: &Path, config: &str) -> String {
    format!(
        "& {} init pwsh --eval --config {} | Invoke-Expression; {POWERSHELL_REDRAW_HANDLER}",
        ps_quote(&binary.to_string_lossy()),
        ps_quote(config)
    )
}

/// PowerShell accepts a single -Command, and the PSReadLine opt-in may
/// already have added one — chain onto it rather than adding a second.
/// Shared with cwd_report, which appends its prompt wrapper after this one.
pub(super) fn append_powershell_command(args: &mut Vec<String>, script: &str) {
    if let Some(index) = args.iter().position(|arg| arg.eq_ignore_ascii_case("-Command")) {
        if let Some(existing) = args.get_mut(index + 1) {
            *existing = format!("{existing}; {script}");
            return;
        }
    }
    args.extend(["-NoExit".to_string(), "-Command".to_string(), script.to_string()]);
}

fn bash_init(binary: &Path, config: &str) -> String {
    format!(
        "[ -f /etc/bash.bashrc ] && . /etc/bash.bashrc\n\
         [ -f \"$HOME/.bashrc\" ] && . \"$HOME/.bashrc\"\n\
         eval \"$({} init bash --config {})\"\n",
        sh_quote(&binary.to_string_lossy()),
        sh_quote(config)
    )
}

fn write_file(path: &Path, contents: &str) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

/// oh-my-posh's bash init needs bash 4.2+ (`[[ -v ]]`); macOS still ships
/// 3.2, where it errors out on every startup. Checked once per bash binary.
fn bash_supports_oh_my_posh(bash: &str) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(supported) = cache.lock().unwrap().get(bash) {
        return *supported;
    }
    let supported = std::process::Command::new(bash)
        .args(["-c", "echo ${BASH_VERSINFO[0]} ${BASH_VERSINFO[1]}"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| {
            let mut parts = text.split_whitespace().map(|part| part.parse::<u32>().ok());
            Some((parts.next()??, parts.next()??))
        })
        .is_some_and(|version| version >= (4, 2));
    cache.lock().unwrap().insert(bash.to_string(), supported);
    supported
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chains_onto_an_existing_powershell_command() {
        let mut args = vec!["-NoExit".to_string(), "-Command".to_string(), "Set-Foo".to_string()];
        append_powershell_command(&mut args, "Init-Posh");
        assert_eq!(args, ["-NoExit", "-Command", "Set-Foo; Init-Posh"]);
    }

    #[test]
    fn adds_a_powershell_command_when_there_is_none() {
        let mut args = Vec::new();
        append_powershell_command(&mut args, "Init-Posh");
        assert_eq!(args, ["-NoExit", "-Command", "Init-Posh"]);
    }

    #[test]
    fn quotes_paths_for_each_shell() {
        assert_eq!(ps_quote("C:\\Users\\O'Neil"), "'C:\\Users\\O''Neil'");
        assert_eq!(sh_quote("/Users/o'neil"), r"'/Users/o'\''neil'");
    }

    #[test]
    fn rejects_theme_names_that_could_escape_the_themes_dir() {
        assert!(is_valid_theme_name("agnoster.minimal"));
        assert!(is_valid_theme_name("powerlevel10k_rainbow"));
        assert!(!is_valid_theme_name("../evil"));
        assert!(!is_valid_theme_name("a/b"));
        assert!(!is_valid_theme_name(""));
    }

    #[test]
    fn detects_the_real_bash_version_here() {
        // /bin/bash is 3.2 on macOS and 4.2+ on any current Linux.
        if cfg!(target_os = "macos") {
            assert!(!bash_supports_oh_my_posh("/bin/bash"));
        }
    }
}
