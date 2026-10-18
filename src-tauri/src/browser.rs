//! Where links open: the browser chosen in Preferences → General, or the
//! system's default one. Every link Termo opens comes through here, the
//! consoles' included (see src/lib/core/links.ts).

use std::path::Path;
use std::process::{Command, Stdio};

use serde::Serialize;

/// A browser installed on this computer, as Preferences lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Browser {
    pub name: String,
    /// What opens it: the executable (Windows), the `.app` (macOS) or the
    /// `.desktop` entry (Linux). This is what `link_browser` holds.
    pub path: String,
}

/// The links that open at all. A console can print any link, `file://` and
/// the like included: those stay unopened.
fn openable(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    ["http://", "https://", "mailto:"].iter().any(|scheme| lower.starts_with(scheme))
}

fn is_web(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Opens `url` with `browser` (a `Browser::path`), or the system's default
/// browser if there's none or it won't start. Mail links go to the mail app.
pub fn open(url: &str, browser: Option<&str>) -> Result<(), String> {
    if !openable(url) {
        return Err("unsupported".to_string());
    }
    if let Some(browser) = browser.filter(|browser| !browser.is_empty() && is_web(url)) {
        if launch(browser, url).is_ok() {
            return Ok(());
        }
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|err| err.to_string())
}

/// Starts it and lets it go: a browser outlives Termo.
fn detach(mut command: Command) -> Result<(), String> {
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|err| err.to_string())?;
    // Reaped once it exits, so it doesn't stay a zombie while Termo runs.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(windows)]
fn launch(browser: &str, url: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    if !Path::new(browser).is_file() {
        return Err("missing".to_string());
    }
    let mut command = Command::new(browser);
    command.arg(url);
    // CREATE_NO_WINDOW: a console browser launcher doesn't flash one.
    command.creation_flags(0x0800_0000);
    detach(command)
}

#[cfg(target_os = "macos")]
fn launch(browser: &str, url: &str) -> Result<(), String> {
    if !Path::new(browser).exists() {
        return Err("missing".to_string());
    }
    let mut command = Command::new("/usr/bin/open");
    command.arg("-a").arg(browser).arg(url);
    detach(command)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn launch(browser: &str, url: &str) -> Result<(), String> {
    let path = Path::new(browser);
    if path.extension().is_some_and(|ext| ext == "desktop") {
        let entry = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        let exec = desktop_field(&entry, "Exec").ok_or("no Exec")?;
        let args = exec_args(&exec, url);
        let (program, rest) = args.split_first().ok_or("empty Exec")?;
        let mut command = Command::new(program);
        command.args(rest);
        return detach(command);
    }
    if !path.is_file() {
        return Err("missing".to_string());
    }
    let mut command = Command::new(path);
    command.arg(url);
    detach(command)
}

/// The browsers installed here, by name.
pub fn installed() -> Vec<Browser> {
    let mut found = find_installed();
    let mut seen = std::collections::HashSet::new();
    found.retain(|browser| seen.insert(browser.path.to_lowercase()));
    found.sort_by_key(|browser| browser.name.to_lowercase());
    found
}

/// Windows lists them under StartMenuInternet: the machine's and this
/// user's, each with its name and the command that opens it.
#[cfg(windows)]
fn find_installed() -> Vec<Browser> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let mut found = Vec::new();
    for (root, base) in [
        (HKEY_CURRENT_USER, r"SOFTWARE\Clients\StartMenuInternet"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Clients\StartMenuInternet"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Clients\StartMenuInternet"),
    ] {
        for key in registry::subkeys(root, base) {
            let at = format!(r"{base}\{key}");
            let Some(command) = registry::default_value(root, &format!(r"{at}\shell\open\command")) else {
                continue;
            };
            let Some(path) = command_program(&command) else {
                continue;
            };
            // Internet Explorer is still registered on Windows 10, but retired:
            // starting it opens Edge.
            if !Path::new(&path).is_file() || path.to_ascii_lowercase().ends_with("\\iexplore.exe") {
                continue;
            }
            let name = registry::default_value(root, &at).filter(|name| !name.trim().is_empty()).unwrap_or(key);
            found.push(Browser { name, path });
        }
    }
    found
}

/// The program a registry command line runs: `"C:\…\chrome.exe" -- "%1"`
/// gives `C:\…\chrome.exe`.
#[cfg(any(windows, test))]
fn command_program(command: &str) -> Option<String> {
    let command = command.trim();
    let program = if let Some(rest) = command.strip_prefix('"') {
        rest.split('"').next()?
    } else {
        // Unquoted: up to the end of the `.exe`, spaces in the path or not.
        let lower = command.to_ascii_lowercase();
        let end = lower.find(".exe").map(|at| at + 4).unwrap_or(command.len());
        &command[..end]
    };
    let program = program.trim();
    (!program.is_empty()).then(|| program.to_string())
}

#[cfg(windows)]
mod registry {
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, FILETIME};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, KEY_READ, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    };

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end])
    }

    /// The names of the keys right under `path`.
    pub fn subkeys(root: HKEY, path: &str) -> Vec<String> {
        let mut key: HKEY = std::ptr::null_mut();
        let path = wide(path);
        // SAFETY: a valid root key, a NUL-terminated path, and an out
        // pointer to a key that's closed below.
        if unsafe { RegOpenKeyExW(root, path.as_ptr(), 0, KEY_READ, &mut key) } != ERROR_SUCCESS {
            return Vec::new();
        }
        let mut names = Vec::new();
        for index in 0.. {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            let mut written = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
            // SAFETY: `name` holds `len` u16s; the optional pointers are null.
            let status = unsafe {
                RegEnumKeyExW(
                    key,
                    index,
                    name.as_mut_ptr(),
                    &mut len,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut written,
                )
            };
            if status != ERROR_SUCCESS {
                break;
            }
            names.push(from_wide(&name[..len as usize]));
        }
        // SAFETY: opened above.
        unsafe { RegCloseKey(key) };
        names
    }

    /// The default value of the key at `path`, as text.
    pub fn default_value(root: HKEY, path: &str) -> Option<String> {
        let path = wide(path);
        let mut buffer = vec![0u16; 1024];
        let mut size = (buffer.len() * 2) as u32;
        // SAFETY: a NUL-terminated path; `buffer` holds `size` bytes.
        let status = unsafe {
            RegGetValueW(
                root,
                path.as_ptr(),
                std::ptr::null(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        buffer.truncate(size as usize / 2);
        Some(from_wide(&buffer))
    }
}

/// macOS: the browsers people use, where apps live.
#[cfg(target_os = "macos")]
fn find_installed() -> Vec<Browser> {
    const KNOWN: &[&str] = &[
        "Safari",
        "Google Chrome",
        "Google Chrome Canary",
        "Firefox",
        "Firefox Developer Edition",
        "Firefox Nightly",
        "Microsoft Edge",
        "Brave Browser",
        "Arc",
        "Opera",
        "Opera GX",
        "Vivaldi",
        "Chromium",
        "Zen",
        "Zen Browser",
        "Orion",
        "DuckDuckGo",
        "LibreWolf",
        "Floorp",
        "Waterfox",
        "Dia",
    ];
    let mut places = vec![std::path::PathBuf::from("/Applications"), std::path::PathBuf::from("/System/Cryptexes/App/System/Applications")];
    if let Some(home) = dirs::home_dir() {
        places.push(home.join("Applications"));
    }
    let mut found = Vec::new();
    for name in KNOWN {
        if let Some(app) = places.iter().map(|dir| dir.join(format!("{name}.app"))).find(|app| app.exists()) {
            found.push(Browser { name: (*name).to_string(), path: app.to_string_lossy().into_owned() });
        }
    }
    found
}

/// Linux: the desktop entries that open web links.
#[cfg(all(unix, not(target_os = "macos")))]
fn find_installed() -> Vec<Browser> {
    let mut places = vec![
        std::path::PathBuf::from("/usr/share/applications"),
        std::path::PathBuf::from("/usr/local/share/applications"),
        std::path::PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        std::path::PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];
    if let Some(data) = dirs::data_dir() {
        places.push(data.join("applications"));
        places.push(data.join("flatpak/exports/share/applications"));
    }
    let mut found = Vec::new();
    for dir in places {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(name) = desktop_browser_name(&text) {
                found.push(Browser { name, path: path.to_string_lossy().into_owned() });
            }
        }
    }
    found
}

/// The name of a desktop entry that opens web links and shows in menus.
#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn desktop_browser_name(entry: &str) -> Option<String> {
    let mime = desktop_field(entry, "MimeType")?;
    if !mime.split(';').any(|kind| kind == "x-scheme-handler/http" || kind == "x-scheme-handler/https") {
        return None;
    }
    let hidden = |field| desktop_field(entry, field).is_some_and(|value| value.eq_ignore_ascii_case("true"));
    if hidden("NoDisplay") || hidden("Hidden") {
        return None;
    }
    desktop_field(entry, "Exec")?;
    desktop_field(entry, "Name")
}

/// A field of a desktop entry's main group (not its actions').
#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn desktop_field(entry: &str, field: &str) -> Option<String> {
    let mut in_main = false;
    for line in entry.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim() == field {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

/// A desktop entry's Exec line, split into arguments, with the link where it
/// says (%u, %U, %f, %F) or at the end. Other field codes go.
#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn exec_args(exec: &str, url: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        args.push(current);
    }
    let mut placed = false;
    let mut out = Vec::new();
    for arg in args {
        match arg.as_str() {
            "%u" | "%U" | "%f" | "%F" => {
                out.push(url.to_string());
                placed = true;
            }
            "%i" | "%c" | "%k" => {}
            _ => out.push(arg.replace("%%", "%")),
        }
    }
    if !placed {
        out.push(url.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_and_mail_links_open() {
        assert!(openable("https://example.com"));
        assert!(openable("HTTP://example.com"));
        assert!(openable("mailto:someone@example.com"));
        assert!(!openable("file:///etc/passwd"));
        assert!(!openable("javascript:alert(1)"));
        assert!(!openable("C:\\Windows\\notepad.exe"));
        assert!(!is_web("mailto:someone@example.com"));
    }

    #[test]
    fn finds_the_program_in_a_registry_command() {
        assert_eq!(
            command_program(r#""C:\Program Files\Google\Chrome\Application\chrome.exe" -- "%1""#).as_deref(),
            Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe")
        );
        assert_eq!(
            command_program(r"C:\Program Files\Mozilla Firefox\firefox.exe -osint -url %1").as_deref(),
            Some(r"C:\Program Files\Mozilla Firefox\firefox.exe")
        );
        assert_eq!(command_program("  "), None);
    }

    #[test]
    fn reads_a_browsers_desktop_entry() {
        let entry = "[Desktop Entry]\nName=Firefox\nExec=firefox %u\nMimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;\n\n[Desktop Action new-window]\nName=New Window\nExec=firefox --new-window %u\n";
        assert_eq!(desktop_browser_name(entry).as_deref(), Some("Firefox"));
        assert_eq!(desktop_field(entry, "Exec").as_deref(), Some("firefox %u"));
        let editor = "[Desktop Entry]\nName=Editor\nExec=editor %F\nMimeType=text/plain;\n";
        assert_eq!(desktop_browser_name(editor), None);
        let hidden = "[Desktop Entry]\nName=Helper\nExec=helper %u\nNoDisplay=true\nMimeType=x-scheme-handler/http;\n";
        assert_eq!(desktop_browser_name(hidden), None);
    }

    #[test]
    fn puts_the_link_where_the_exec_line_says() {
        let url = "https://example.com/?a=1&b=2";
        assert_eq!(exec_args("firefox %u", url), ["firefox", url]);
        assert_eq!(
            exec_args(r#"/usr/bin/flatpak run --command=brave "com.brave.Browser" @@u %U @@"#, url),
            ["/usr/bin/flatpak", "run", "--command=brave", "com.brave.Browser", "@@u", url, "@@"]
        );
        assert_eq!(exec_args("chromium --incognito", url), ["chromium", "--incognito", url]);
        assert_eq!(exec_args(r#""/opt/My Browser/browser" %i %u"#, url), ["/opt/My Browser/browser", url]);
    }
}
