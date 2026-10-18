//! Starting the agents' process when the user signs in, with no Termo opened
//! (AppConfig.background_agents_at_login): the Run key on Windows, a launch
//! agent on macOS, an autostart entry elsewhere. Brought up to date every
//! time the app starts, as the program it points at can move with an update.

use std::path::{Path, PathBuf};

use crate::config::AppConfig;

/// The name the entry goes by: a dev build's apart, as its folder is.
fn entry_name() -> &'static str {
    if cfg!(debug_assertions) {
        "Termo agents (dev)"
    } else {
        "Termo agents"
    }
}

/// Puts the entry in place or takes it out, as the config says.
pub fn sync(config: &AppConfig) {
    let wanted = config.background_agents && config.background_agents_at_login;
    let result = if wanted { install() } else { remove() };
    if let Err(error) = result {
        eprintln!("agents at sign-in: {error}");
    }
}

/// What runs at sign-in: the process, with its folder.
fn command() -> Result<(PathBuf, Vec<String>), String> {
    let dir = super::dir().ok_or("no data folder")?;
    let program = super::client::program(&dir)?;
    let build = std::env::current_exe().map(|exe| crate::mux::build_stamp(&exe)).unwrap_or_default();
    Ok((program, vec![super::HOST_FLAG.to_string(), build, dir.display().to_string()]))
}

#[cfg(windows)]
mod run_key {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn set(name: &str, value: &str) -> Result<(), String> {
        let (key, name, data) = (wide(RUN), wide(name), wide(value));
        // SAFETY: NUL-terminated strings; `data` is `data.len() * 2` bytes.
        let status = unsafe {
            RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32)
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("registry error {status}"))
        }
    }

    pub fn delete(name: &str) {
        let (key, name) = (wide(RUN), wide(name));
        // SAFETY: NUL-terminated strings. Not there is fine.
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
    }
}

/// A command line Windows runs as it is: each part quoted.
#[cfg(any(windows, test))]
fn command_line(program: &Path, args: &[String]) -> String {
    std::iter::once(program.display().to_string())
        .chain(args.iter().cloned())
        .map(|part| format!("\"{}\"", part.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(windows)]
fn install() -> Result<(), String> {
    let (program, args) = command()?;
    run_key::set(entry_name(), &command_line(&program, &args))
}

#[cfg(windows)]
fn remove() -> Result<(), String> {
    run_key::delete(entry_name());
    Ok(())
}

#[cfg(target_os = "macos")]
fn plist_path() -> Result<PathBuf, String> {
    let label = if cfg!(debug_assertions) { "com.ln.termo.agents-dev" } else { "com.ln.termo.agents" };
    Ok(dirs::home_dir().ok_or("no home folder")?.join("Library/LaunchAgents").join(format!("{label}.plist")))
}

#[cfg(any(target_os = "macos", test))]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(any(target_os = "macos", test))]
fn plist(label: &str, program: &Path, args: &[String]) -> String {
    let arguments: String = std::iter::once(program.display().to_string())
        .chain(args.iter().cloned())
        .map(|part| format!("        <string>{}</string>\n", xml_escape(&part)))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n    <key>Label</key>\n    <string>{}</string>\n    <key>ProgramArguments</key>\n    <array>\n{arguments}    </array>\n    <key>RunAtLoad</key>\n    <true/>\n    <key>ProcessType</key>\n    <string>Background</string>\n</dict>\n</plist>\n",
        xml_escape(label)
    )
}

#[cfg(target_os = "macos")]
fn install() -> Result<(), String> {
    let (program, args) = command()?;
    let path = plist_path()?;
    let label = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("com.ln.termo.agents").to_string();
    std::fs::create_dir_all(path.parent().ok_or("no folder")?).map_err(|err| err.to_string())?;
    std::fs::write(&path, plist(&label, &program, &args)).map_err(|err| err.to_string())
}

#[cfg(target_os = "macos")]
fn remove() -> Result<(), String> {
    let _ = std::fs::remove_file(plist_path()?);
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn desktop_path() -> Result<PathBuf, String> {
    let name = if cfg!(debug_assertions) { "termo-agents-dev.desktop" } else { "termo-agents.desktop" };
    Ok(dirs::config_dir().ok_or("no config folder")?.join("autostart").join(name))
}

/// One argument of a desktop entry's Exec line, quoted as the spec says.
#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn exec_quote(part: &str) -> String {
    let escaped: String = part
        .chars()
        .flat_map(|c| match c {
            '"' | '`' | '$' | '\\' => vec!['\\', c],
            '%' => vec!['%', '%'],
            c => vec![c],
        })
        .collect();
    format!("\"{escaped}\"")
}

#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn desktop_entry(name: &str, program: &Path, args: &[String]) -> String {
    let exec: Vec<String> = std::iter::once(program.display().to_string()).chain(args.iter().cloned()).map(|part| exec_quote(&part)).collect();
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec={}\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n", exec.join(" "))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install() -> Result<(), String> {
    let (program, args) = command()?;
    let path = desktop_path()?;
    std::fs::create_dir_all(path.parent().ok_or("no folder")?).map_err(|err| err.to_string())?;
    std::fs::write(&path, desktop_entry(entry_name(), &program, &args)).map_err(|err| err.to_string())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn remove() -> Result<(), String> {
    let _ = std::fs::remove_file(desktop_path()?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> Vec<String> {
        vec!["--agents".into(), "0.24.0-a-b".into(), "C:\\Users\\Ana López\\AppData\\agents-host".into()]
    }

    #[test]
    fn a_windows_command_line_quotes_every_part() {
        let line = command_line(Path::new(r"C:\Program Files\Termo\termo-agents.exe"), &args());
        assert_eq!(
            line,
            r#""C:\Program Files\Termo\termo-agents.exe" "--agents" "0.24.0-a-b" "C:\Users\Ana López\AppData\agents-host""#
        );
    }

    #[test]
    fn a_launch_agent_lists_the_program_and_its_arguments() {
        let text = plist("com.ln.termo.agents", Path::new("/Applications/Termo.app/Contents/MacOS/termo"), &args());
        assert!(text.contains("<string>/Applications/Termo.app/Contents/MacOS/termo</string>"));
        assert!(text.contains("<string>--agents</string>"));
        assert!(text.contains("<key>RunAtLoad</key>\n    <true/>"));
    }

    #[test]
    fn an_autostart_entry_quotes_what_a_desktop_entry_would_read_otherwise() {
        let text = desktop_entry("Termo agents", Path::new("/opt/Termo $1/termo"), &["--agents".into(), "100%".into()]);
        assert!(text.contains(r#"Exec="/opt/Termo \$1/termo" "--agents" "100%%""#), "{text}");
    }
}
