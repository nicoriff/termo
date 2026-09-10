#[derive(Clone, serde::Serialize)]
pub struct ShellInfo {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub args: Vec<String>,
}

/// The shell used for new sessions when the user hasn't picked one explicitly.
pub fn default_shell() -> ShellInfo {
    let shells = list_shells();
    shells.into_iter().next().unwrap_or_else(|| fallback_shell())
}

#[cfg(target_os = "windows")]
fn fallback_shell() -> ShellInfo {
    ShellInfo {
        id: "cmd".into(),
        display_name: "Command Prompt".into(),
        path: "cmd.exe".into(),
        args: vec![],
    }
}

#[cfg(not(target_os = "windows"))]
fn fallback_shell() -> ShellInfo {
    ShellInfo {
        id: "sh".into(),
        display_name: "sh".into(),
        path: "/bin/sh".into(),
        args: vec![],
    }
}

#[cfg(target_os = "windows")]
pub fn list_shells() -> Vec<ShellInfo> {
    use std::path::Path;

    let mut shells = Vec::new();

    if let Some(pwsh) = which("pwsh.exe") {
        shells.push(ShellInfo {
            id: "pwsh".into(),
            display_name: "PowerShell 7".into(),
            path: pwsh,
            args: vec![],
        });
    }
    if let Some(powershell) = which("powershell.exe") {
        shells.push(ShellInfo {
            id: "powershell".into(),
            display_name: "Windows PowerShell".into(),
            path: powershell,
            args: vec![],
        });
    }
    if Path::new("C:\\Windows\\System32\\cmd.exe").exists() {
        shells.push(ShellInfo {
            id: "cmd".into(),
            display_name: "Command Prompt".into(),
            path: "cmd.exe".into(),
            args: vec![],
        });
    }

    let git_bash = "C:\\Program Files\\Git\\bin\\bash.exe";
    if Path::new(git_bash).exists() {
        shells.push(ShellInfo {
            id: "git-bash".into(),
            display_name: "Git Bash".into(),
            path: git_bash.into(),
            args: vec![],
        });
    }

    shells.extend(list_wsl_distros());

    shells
}

#[cfg(target_os = "windows")]
fn which(exe: &str) -> Option<String> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(exe))
            .find(|full| full.exists())
            .map(|full| full.to_string_lossy().into_owned())
    })
}

#[cfg(target_os = "windows")]
fn list_wsl_distros() -> Vec<ShellInfo> {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: avoid flashing a console window when probing for WSL.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("wsl.exe")
        .args(["-l", "-q"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let Ok(output) = output else {
        return vec![];
    };
    if !output.status.success() {
        return vec![];
    }

    // `wsl -l -q` prints UTF-16LE on stock Windows consoles.
    let text = decode_wsl_output(&output.stdout);

    text.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|distro| ShellInfo {
            id: format!("wsl-{distro}"),
            display_name: format!("WSL: {distro}"),
            path: "wsl.exe".into(),
            args: vec!["-d".into(), distro.to_string()],
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn decode_wsl_output(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0..2] == [0xFF, 0xFE] {
        let utf16: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        String::from_utf16_lossy(&utf16)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(not(target_os = "windows"))]
pub fn list_shells() -> Vec<ShellInfo> {
    use std::collections::HashSet;
    use std::path::Path;

    let mut shells = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(current) = std::env::var("SHELL") {
        if Path::new(&current).exists() && seen.insert(current.clone()) {
            shells.push(shell_info_for_path(&current, true));
        }
    }

    if let Ok(contents) = std::fs::read_to_string("/etc/shells") {
        for line in contents.lines() {
            let path = line.trim();
            if path.is_empty() || path.starts_with('#') {
                continue;
            }
            if Path::new(path).exists() && seen.insert(path.to_string()) {
                shells.push(shell_info_for_path(path, false));
            }
        }
    }

    shells
}

#[cfg(not(target_os = "windows"))]
fn shell_info_for_path(path: &str, is_default: bool) -> ShellInfo {
    let name = path.rsplit('/').next().unwrap_or(path);
    ShellInfo {
        id: name.to_string(),
        display_name: if is_default {
            format!("{name} (default)")
        } else {
            name.to_string()
        },
        path: path.to_string(),
        args: vec![],
    }
}
