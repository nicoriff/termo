use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use tauri::Manager;

use super::oh_my_posh::{
    append_powershell_command, bash_rcfile, is_valid_theme_name, ps_quote, put_binary_on_path, sh_quote, write_file,
    POWERSHELL_REDRAW_HANDLER, POWERSHELL_REDRAW_KEY, ZSH_REDRAW_LINE,
};

pub const DEFAULT_PRESET: &str = "nerd-font-symbols";

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "starship.exe"
    } else {
        "starship"
    }
}

fn root_dir(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("starship"))
}

/// Where Termo's own copy of the binary lives, next to Oh My Posh's and for
/// the same reason: installing never touches the user's shell setup.
pub fn managed_binary(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    Ok(root_dir(app)?.join("bin").join(exe_name()))
}

pub fn preset_file(app: &tauri::AppHandle, preset: &str) -> anyhow::Result<PathBuf> {
    Ok(root_dir(app)?.join("presets").join(format!("{preset}.toml")))
}

/// Preset names end up in a file path, so they follow the same rule as
/// Oh My Posh's theme names.
pub fn is_valid_preset_name(name: &str) -> bool {
    is_valid_theme_name(name)
}

/// The archive the official releases publish for this platform. Linux gets
/// the musl build, which runs on any distribution.
pub fn download_url() -> Option<String> {
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc.zip",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc.zip",
        ("macos", "x86_64") => "x86_64-apple-darwin.tar.gz",
        ("macos", "aarch64") => "aarch64-apple-darwin.tar.gz",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl.tar.gz",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl.tar.gz",
        _ => return None,
    };
    Some(format!("https://github.com/starship/starship/releases/latest/download/starship-{target}"))
}

/// The binary inside a downloaded release archive: a .zip on Windows, a
/// .tar.gz everywhere else, with the binary at the top level of either.
pub fn extract_binary(archive: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut binary = Vec::new();
    if cfg!(windows) {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))?;
        zip.by_name(exe_name())?.read_to_end(&mut binary)?;
        return Ok(binary);
    }
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries()? {
        let mut entry = entry?;
        if entry.path()?.file_name().is_some_and(|name| name == exe_name()) {
            entry.read_to_end(&mut binary)?;
            return Ok(binary);
        }
    }
    anyhow::bail!("{} not found in the archive", exe_name())
}

/// The binary to use, and whether it's Termo's own managed copy. Prefers
/// the managed copy, then any existing install the user already has.
pub fn find(app: &tauri::AppHandle) -> Option<(PathBuf, bool)> {
    if let Ok(managed) = managed_binary(app) {
        if managed.is_file() {
            return Some((managed, true));
        }
    }
    super::bin_search::find_binary("starship").map(|path| (path, false))
}

/// The config file for `preset`, written by starship itself the first time:
/// the presets ship inside the binary, so nothing is downloaded.
pub fn ensure_preset(app: &tauri::AppHandle, binary: &Path, preset: &str) -> anyhow::Result<PathBuf> {
    let preset = if is_valid_preset_name(preset) { preset } else { DEFAULT_PRESET };
    let file = preset_file(app, preset)?;
    if file.is_file() {
        return Ok(file);
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    // Written beside the target and renamed, so a failed run never leaves a
    // half-written file that the next session would take as finished.
    let partial = file.with_extension("partial");
    let mut command = std::process::Command::new(binary);
    command.arg("preset").arg(preset).arg("-o").arg(&partial);
    crate::git::no_window(&mut command);
    let output = command.output()?;
    if !output.status.success() {
        let _ = fs::remove_file(&partial);
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    fs::rename(&partial, &file)?;
    Ok(file)
}

/// Hooks starship's prompt into a new session of `shell_path`, if that shell
/// is supported; unsupported shells are left untouched. With `preset` None
/// the user's own starship.toml applies. Same contract as oh_my_posh::apply,
/// including the keys returned for redrawing the prompt after a resize: a
/// `right_format` is padded to the width at the time, just like Oh My Posh's.
pub fn apply(
    app: &tauri::AppHandle,
    binary: &Path,
    managed: bool,
    preset: Option<&str>,
    shell_path: &str,
    args: &mut Vec<String>,
    env: &mut Vec<(&'static str, String)>,
    zsh_lines: &mut Vec<String>,
) -> anyhow::Result<Option<&'static [u8]>> {
    let shell = Path::new(shell_path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    // Git Bash on Windows is left alone, for the same reason as in oh_my_posh.
    if !matches!(shell.as_str(), "pwsh" | "powershell" | "zsh" | "bash") || (shell == "bash" && cfg!(windows)) {
        return Ok(None);
    }
    // Before anything is changed, so a preset that can't be written leaves
    // the launch exactly as it was.
    let config = preset.map(|preset| ensure_preset(app, binary, preset)).transpose()?;
    let mut redraw_key = None;

    match shell.as_str() {
        "pwsh" | "powershell" => {
            append_powershell_command(args, &powershell_init(binary));
            redraw_key = Some(POWERSHELL_REDRAW_KEY);
        }
        "zsh" => {
            zsh_lines.push(format!("eval \"$({} init zsh)\"", sh_quote(&binary.to_string_lossy())));
            zsh_lines.push(ZSH_REDRAW_LINE.to_string());
        }
        _ => {
            let file = root_dir(app)?.join("init").join("bashrc");
            write_file(&file, &bash_init(binary))?;
            args.splice(0..0, ["--rcfile".to_string(), file.to_string_lossy().into_owned()]);
        }
    }

    if let Some(config) = config {
        env.push(("STARSHIP_CONFIG", config.to_string_lossy().into_owned()));
    }
    if managed {
        // So `starship` itself is usable inside Termo sessions too.
        put_binary_on_path(binary, env);
    }
    Ok(redraw_key)
}

/// `--print-full-init` inlines the init script instead of having PowerShell
/// run a generated .ps1, which a Restricted execution policy would block.
/// The script is read as UTF-8: Windows PowerShell would decode it in the
/// OEM code page and garble its `❯`. Run in a child scope so the variables
/// stay out of the user's session; everything the script sets up lives in
/// its own module or in `global:`.
fn powershell_init(binary: &Path) -> String {
    format!(
        "& {{ $encoding = [Console]::OutputEncoding; [Console]::OutputEncoding = [Text.Encoding]::UTF8; \
         try {{ $init = & {} init powershell --print-full-init | Out-String }} \
         finally {{ [Console]::OutputEncoding = $encoding }}; Invoke-Expression $init }}; \
         {POWERSHELL_REDRAW_HANDLER}",
        ps_quote(&binary.to_string_lossy())
    )
}

fn bash_init(binary: &Path) -> String {
    bash_rcfile(&format!("eval \"$({} init bash)\"", sh_quote(&binary.to_string_lossy())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_init_runs_the_full_script_inline() {
        let init = powershell_init(Path::new("C:\\Users\\O'Neil\\starship.exe"));
        assert!(init.contains("$init = & 'C:\\Users\\O''Neil\\starship.exe' init powershell --print-full-init | Out-String"));
        assert!(init.contains("Invoke-Expression $init }; "));
        assert!(init.ends_with(POWERSHELL_REDRAW_HANDLER));
    }

    #[test]
    fn bash_init_keeps_the_users_startup_files() {
        let init = bash_init(Path::new("/opt/bin/starship"));
        assert!(init.contains(". \"$HOME/.bashrc\""));
        assert!(init.ends_with("eval \"$('/opt/bin/starship' init bash)\"\n"));
    }

    #[test]
    fn rejects_preset_names_that_could_escape_the_presets_dir() {
        assert!(is_valid_preset_name("nerd-font-symbols"));
        assert!(is_valid_preset_name("gruvbox-rainbow"));
        assert!(!is_valid_preset_name("../evil"));
        assert!(!is_valid_preset_name("a\\b"));
    }

    #[test]
    fn has_a_download_for_the_platforms_termo_ships_on() {
        if cfg!(any(windows, target_os = "macos", target_os = "linux")) {
            let url = download_url().expect("a release asset for this platform");
            assert!(url.ends_with(if cfg!(windows) { ".zip" } else { ".tar.gz" }));
        }
    }
}
