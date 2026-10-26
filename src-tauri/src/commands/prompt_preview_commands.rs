use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::download::{blocking, download_to};
use crate::pty::{oh_my_posh, starship};

/// How long a preview may take: a theme's git or language segments run
/// commands of their own, and a slow one shouldn't leave a hover waiting.
const PREVIEW_TIMEOUT: Duration = Duration::from_secs(3);

/// What a prompt theme looks like, for the status bar's Prompt menu: the
/// tool's primary prompt, as raw ANSI, drawn for `cwd` (the active console's
/// folder, so its repository shows) at `width` columns. Nothing runs in a
/// console. An Oh My Posh theme not downloaded yet is downloaded first, a
/// Starship preset written; `theme` None is Starship with the user's own
/// starship.toml.
#[tauri::command]
pub async fn prompt_preview(
    app: tauri::AppHandle,
    tool: String,
    theme: Option<String>,
    cwd: Option<String>,
    width: u16,
) -> Result<String, String> {
    blocking(move || {
        let cwd = cwd.filter(|dir| Path::new(dir).is_dir());
        let width = width.clamp(20, 400).to_string();
        let mut command = match tool.as_str() {
            "oh_my_posh" => {
                let theme = theme.filter(|name| oh_my_posh::is_valid_theme_name(name)).ok_or_else(|| anyhow::anyhow!("no theme"))?;
                let (binary, _) = oh_my_posh::find(&app).ok_or_else(|| anyhow::anyhow!("oh-my-posh isn't installed"))?;
                let file = oh_my_posh::theme_file(&app, &theme)?;
                if !file.is_file() {
                    download_to(
                        &format!("https://raw.githubusercontent.com/JanDeDobbeleer/oh-my-posh/main/themes/{theme}.omp.json"),
                        &file,
                    )?;
                }
                let mut command = Command::new(binary);
                command
                    .args(["print", "primary", "--escape=false", "--shell"])
                    .arg(if cfg!(windows) { "pwsh" } else { "zsh" })
                    .arg("--config")
                    .arg(&file)
                    .args(["--terminal-width", &width]);
                if let Some(cwd) = &cwd {
                    command.arg("--pwd").arg(cwd);
                }
                command
            }
            "starship" => {
                let (binary, _) = starship::find(&app).ok_or_else(|| anyhow::anyhow!("starship isn't installed"))?;
                let config = match theme {
                    Some(preset) if starship::is_valid_preset_name(&preset) => Some(starship::ensure_preset(&app, &binary, &preset)?),
                    Some(_) => anyhow::bail!("no preset"),
                    None => None,
                };
                let mut command = Command::new(binary);
                command.args(["prompt", "--terminal-width", &width]);
                if let Some(cwd) = &cwd {
                    command.arg("--path").arg(cwd);
                }
                // Raw ANSI, without a shell's wrapping of the escapes.
                command.env_remove("STARSHIP_SHELL");
                if let Some(config) = config {
                    command.env("STARSHIP_CONFIG", config);
                }
                command
            }
            _ => anyhow::bail!("unknown prompt tool"),
        };
        if let Some(cwd) = &cwd {
            command.current_dir(cwd);
        }
        run_with_timeout(command)
    })
    .await
}

/// Its stdout, or an error if it fails or takes longer than PREVIEW_TIMEOUT
/// (then it is killed).
fn run_with_timeout(mut command: Command) -> anyhow::Result<String> {
    crate::git::no_window(&mut command);
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let mut stdout = child.stdout.take().expect("piped");
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        let _ = send.send(bytes);
    });
    let Ok(bytes) = receive.recv_timeout(PREVIEW_TIMEOUT) else {
        let _ = child.kill();
        let _ = child.wait();
        anyhow::bail!("the preview took too long");
    };
    if !child.wait()?.success() {
        anyhow::bail!("the prompt tool failed");
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
