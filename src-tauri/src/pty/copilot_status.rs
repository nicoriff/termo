//! Opt-in integration with Copilot CLI's status line, so Termo can show a
//! console's context window on its own status bar (its plan usage comes from
//! GitHub instead, see copilot_quota.rs: the status line doesn't carry it).
//!
//! Copilot CLI allows a single `statusLine` in `~/.copilot/settings.json` and
//! runs its `command` through the platform shell (cmd.exe on Windows), where
//! a shell-level chain like Claude's (status_line.rs) can't be written the
//! same way everywhere. So Termo chains in its own executable: the command is
//! `termo --statusline copilot [<hex>]`, which records the JSON and then runs
//! the user's original command — carried hex-encoded in that last argument,
//! so no quoting can break it — with the same input, its output being what
//! Copilot displays. Uninstalling decodes it and puts it back.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Map, Value};

use super::agent_hooks::{read_root, write_root};
use super::status_line::{write_snapshot, StatusSnapshot, STATUS_LINE_FLAG};

/// The argument after `--statusline` that says the JSON is Copilot CLI's.
pub const COPILOT_ARG: &str = "copilot";

fn settings_file() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(crate::i18n::Msg::HomeDirNotFound)))?;
    Ok(crate::agents::spec::copilot_home(&home).join("settings.json"))
}

fn snapshot_from(input: &str) -> Option<StatusSnapshot> {
    let payload: Value = serde_json::from_str(input).ok()?;
    let text = |pointer: &str| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let number = |pointer: &str| payload.pointer(pointer).and_then(Value::as_f64);
    Some(StatusSnapshot {
        tool: Some(COPILOT_ARG.to_string()),
        agent_session_id: text("/session_id"),
        cwd: text("/cwd").or_else(|| text("/workspace/current_dir")),
        project_dir: None,
        model: text("/model/display_name").or_else(|| text("/model/id")),
        // `used_percentage` is null while the model's window is unknown (Auto
        // picks it per turn); the share of the limit it displays is always there.
        context_used_percentage: number("/context_window/used_percentage")
            .or_else(|| number("/context_window/current_context_used_percentage")),
        // A subscription reports AI credits, not dollars.
        cost_usd: None,
        plan_windows: Vec::new(),
        spend_limit_percentage: None,
        context_tokens: number("/context_window/current_context_tokens"),
        context_window_tokens: number("/context_window/displayed_context_limit"),
        ..Default::default()
    })
}

fn hex_encode(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(hex: &str) -> Option<String> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| hex.get(i..i + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
        .collect();
    String::from_utf8(bytes?).ok()
}

/// The executable as the platform shell takes it: cmd.exe on Windows,
/// /bin/sh elsewhere.
fn quote_for_shell(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    if cfg!(windows) {
        format!("\"{path}\"")
    } else {
        format!("'{}'", path.replace('\'', r"'\''"))
    }
}

fn command(exe: &Path, wrapped: Option<&str>) -> String {
    let base = format!("{} {STATUS_LINE_FLAG} {COPILOT_ARG}", quote_for_shell(exe));
    match wrapped.filter(|command| !command.trim().is_empty()) {
        Some(command) => format!("{base} {}", hex_encode(command)),
        None => base,
    }
}

/// When `command` is Termo's: the user's command it wraps, if any.
fn wrapped_of(command: &str) -> Option<Option<String>> {
    let marker = format!("{STATUS_LINE_FLAG} {COPILOT_ARG}");
    let rest = command.split_once(&marker)?.1.trim();
    Some(if rest.is_empty() { None } else { hex_decode(rest) })
}

fn current_command(root: &Map<String, Value>) -> Option<String> {
    root.get("statusLine")?.get("command")?.as_str().map(str::to_owned)
}

pub fn is_installed() -> bool {
    settings_file()
        .and_then(|file| read_root(&file))
        .ok()
        .and_then(|root| current_command(&root))
        .is_some_and(|command| wrapped_of(&command).is_some())
}

pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let file = settings_file()?;
    let mut root = read_root(&file)?;
    // Re-installing (the executable moved) keeps the command already wrapped.
    let wrapped = match current_command(&root) {
        Some(command) => wrapped_of(&command).unwrap_or(Some(command)),
        None => None,
    };
    let backup = file.with_extension("json.termo-backup");
    if file.exists() && !backup.exists() {
        std::fs::copy(&file, &backup)?;
    }
    // Its other settings (padding, refreshInterval) stay as they were.
    let mut line = root.get("statusLine").and_then(Value::as_object).cloned().unwrap_or_default();
    line.insert("type".into(), json!("command"));
    line.insert("command".into(), json!(command(&exe, wrapped.as_deref())));
    root.insert("statusLine".into(), Value::Object(line));
    write_root(&file, &root)
}

pub fn uninstall() -> anyhow::Result<()> {
    let file = settings_file()?;
    if !file.exists() {
        return Ok(());
    }
    let mut root = read_root(&file)?;
    let Some(wrapped) = current_command(&root).and_then(|command| wrapped_of(&command)) else {
        return Ok(());
    };
    match wrapped {
        Some(command) => {
            if let Some(line) = root.get_mut("statusLine").and_then(Value::as_object_mut) {
                line.insert("command".into(), json!(command));
            }
        }
        None => {
            root.remove("statusLine");
        }
    }
    write_root(&file, &root)
}

/// Keeps an installed status line pointed at this executable, which moves
/// between dev and installed builds. No-op when it isn't installed.
pub fn refresh_installed() {
    if is_installed() {
        let _ = install();
    }
}

/// `termo --statusline copilot [<hex>]`, run by Copilot CLI on every refresh
/// with the session JSON on stdin. Records it for this console (inside Termo
/// only), then hands the same JSON to the user's own command, whose output is
/// what Copilot shows. Never fails: it must not disturb their status line.
pub fn run_cli(wrapped_hex: Option<&str>) {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    if let (Some(dir), Ok(session)) = (std::env::var_os("TERMO_EVENTS_DIR"), std::env::var("TERMO_SESSION_ID")) {
        if let Some(snapshot) = snapshot_from(&input) {
            let _ = write_snapshot(Path::new(&dir), &session, &snapshot);
        }
    }
    if let Some(wrapped) = wrapped_hex.and_then(hex_decode) {
        run_wrapped(&wrapped, &input);
    }
}

fn run_wrapped(command: &str, input: &str) {
    // Copilot expands a leading `~` itself; the shell below won't on Windows.
    let command = match (command.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', '\\']) => {
            format!("{}{rest}", home.display())
        }
        _ => command.to_string(),
    };
    let mut shell = if cfg!(windows) {
        let mut cmd = Command::new("cmd");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Passed through as written: cmd.exe has its own quoting rules.
            cmd.arg("/C").raw_arg(&command);
        }
        cmd
    } else {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(&command);
        cmd
    };
    let Ok(mut child) = shell.stdin(Stdio::piped()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn() else {
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What Copilot CLI 1.0.90 sends, trimmed.
    const SAMPLE: &str = r#"{
        "cwd": "E:\\Proyectos\\Booky",
        "session_id": "0b739325-ae54-4319-9ffa-8ff83f70114e",
        "model": { "id": "auto", "display_name": "Auto → mai-code-1.1-flash" },
        "workspace": { "current_dir": "E:\\Proyectos\\Booky" },
        "cost": { "total_premium_requests": 1 },
        "context_window": {
            "used_percentage": null,
            "current_context_tokens": 15719,
            "displayed_context_limit": 200000,
            "current_context_used_percentage": 8
        }
    }"#;

    #[test]
    fn reads_the_context_and_the_conversation() {
        let snapshot = snapshot_from(SAMPLE).unwrap();
        assert_eq!(snapshot.tool.as_deref(), Some("copilot"));
        assert_eq!(snapshot.agent_session_id.as_deref(), Some("0b739325-ae54-4319-9ffa-8ff83f70114e"));
        assert_eq!(snapshot.cwd.as_deref(), Some("E:\\Proyectos\\Booky"));
        assert_eq!(snapshot.model.as_deref(), Some("Auto → mai-code-1.1-flash"));
        assert_eq!(snapshot.context_used_percentage, Some(8.0));
        assert_eq!((snapshot.context_tokens, snapshot.context_window_tokens), (Some(15719.0), Some(200000.0)));
        assert!(snapshot.plan_windows.is_empty());
        assert!(snapshot_from("not json").is_none());
    }

    #[test]
    fn carries_the_users_command_whatever_it_has_in_it() {
        let theirs = r#"node "C:\my scripts\line.js" --flag 'x' & echo 100%"#;
        let ours = command(Path::new("/opt/termo"), Some(theirs));
        assert!(ours.contains("--statusline copilot "));
        assert_eq!(wrapped_of(&ours), Some(Some(theirs.to_string())));
        // Without one of the user's, nothing after it.
        let alone = command(Path::new("/opt/termo"), None);
        assert!(alone.ends_with("--statusline copilot"));
        assert_eq!(wrapped_of(&alone), Some(None));
        // Someone else's command isn't Termo's.
        assert_eq!(wrapped_of("bash ~/line.sh"), None);
        assert_eq!(hex_decode("zz"), None);
    }
}
