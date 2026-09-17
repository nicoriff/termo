//! Opt-in integration with Claude Code's status line, so Termini can show a
//! console's plan usage (the 5h/7d rate-limit windows), context window and
//! session cost on its own status bar.
//!
//! Claude Code allows a single `statusLine` per user and runs its `command` in
//! a shell, so installing *chains* onto whatever the user already had instead
//! of replacing it: the same JSON is piped to Termini (which only records it)
//! and then to their original command, whose output is what Claude Code keeps
//! displaying. The original command is stored in Termini's config, so
//! uninstalling puts it back.
//!
//! `rate_limits` only exists for claude.ai Pro/Max sessions and only after the
//! first API response, so every field here is optional.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tauri::Emitter;

use super::agent_hooks::{events_dir, quote_executable, read_root, write_root};

/// First argument that switches the executable into status-line mode.
pub const STATUS_LINE_FLAG: &str = "--statusline";

const SNAPSHOT_SUFFIX: &str = ".status.json";

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    /// Claude's own session id, cwd and project root: what Modo IA needs to
    /// list this conversation and resume it later (agent_sessions.rs).
    agent_session_id: Option<String>,
    cwd: Option<String>,
    project_dir: Option<String>,
    model: Option<String>,
    context_used_percentage: Option<f64>,
    cost_usd: Option<f64>,
    five_hour_percentage: Option<f64>,
    five_hour_resets_at: Option<i64>,
    seven_day_percentage: Option<f64>,
    seven_day_resets_at: Option<i64>,
    spend_limit_percentage: Option<f64>,
}

fn snapshot_from(input: &str) -> Option<StatusSnapshot> {
    let payload: Value = serde_json::from_str(input).ok()?;
    let number = |pointer: &str| payload.pointer(pointer).and_then(Value::as_f64);
    let integer = |pointer: &str| payload.pointer(pointer).and_then(Value::as_i64);

    let text = |pointer: &str| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };

    Some(StatusSnapshot {
        agent_session_id: text("/session_id"),
        cwd: text("/cwd").or_else(|| text("/workspace/current_dir")),
        project_dir: text("/workspace/project_dir"),
        model: payload
            .pointer("/model/display_name")
            .and_then(Value::as_str)
            .map(str::to_owned),
        context_used_percentage: number("/context_window/used_percentage"),
        cost_usd: number("/cost/total_cost_usd"),
        five_hour_percentage: number("/rate_limits/five_hour/used_percentage"),
        five_hour_resets_at: integer("/rate_limits/five_hour/resets_at"),
        seven_day_percentage: number("/rate_limits/seven_day/used_percentage"),
        seven_day_resets_at: integer("/rate_limits/seven_day/resets_at"),
        spend_limit_percentage: number("/rate_limits/spend_limit/used_percentage"),
    })
}

/// `termini --statusline`, run by Claude Code on every status-line refresh with
/// the session JSON on stdin. Records the snapshot for this console and returns
/// normally whatever happens: it must never disturb the user's own status line.
pub fn run_status_line_cli() {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let (Some(dir), Ok(session)) = (
        std::env::var_os("TERMINI_EVENTS_DIR"),
        std::env::var("TERMINI_SESSION_ID"),
    ) else {
        return;
    };
    let Some(snapshot) = snapshot_from(&input) else {
        return;
    };
    let _ = write_snapshot(Path::new(&dir), &session, &snapshot);
}

fn write_snapshot(dir: &Path, session: &str, snapshot: &StatusSnapshot) -> std::io::Result<()> {
    // Session ids are UUIDs; anything else must not name a path outside `dir`.
    if session.is_empty() || !session.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Ok(());
    }
    let path = dir.join(format!("{session}{SNAPSHOT_SUFFIX}"));
    // Write then rename, so the watcher never reads a half-written file.
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string(snapshot).unwrap_or_default())?;
    fs::rename(&tmp, &path)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StatusEvent {
    session_id: String,
    #[serde(flatten)]
    snapshot: StatusSnapshot,
}

/// Polls the events dir and emits `agent-status` whenever a console's snapshot
/// changes. Separate from the agent-event watcher: these are whole-file
/// snapshots rather than an append-only log.
pub fn start_watcher(app: tauri::AppHandle) {
    let Ok(dir) = events_dir(&app) else {
        return;
    };

    std::thread::spawn(move || {
        let mut seen: HashMap<PathBuf, SystemTime> = HashMap::new();
        loop {
            std::thread::sleep(Duration::from_millis(500));
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(session_id) = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| name.strip_suffix(SNAPSHOT_SUFFIX))
                    .map(str::to_owned)
                else {
                    continue;
                };
                let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
                    continue;
                };
                if seen.get(&path) == Some(&modified) {
                    continue;
                }
                seen.insert(path.clone(), modified);
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(snapshot) = serde_json::from_str::<StatusSnapshot>(&text) else {
                    continue;
                };
                if let Some(agent_session_id) = snapshot.agent_session_id.clone() {
                    // The status line is Claude-only, so the tool is known.
                    crate::agent_sessions::record(
                        &app,
                        crate::agent_sessions::AgentSession::new("claude", &agent_session_id)
                            .with_cwd(snapshot.cwd.clone())
                            .with_project_dir(snapshot.project_dir.clone())
                            .with_model(snapshot.model.clone()),
                    );
                }
                let _ = app.emit("agent-status", StatusEvent { session_id, snapshot });
            }
            seen.retain(|path, _| path.exists());
        }
    });
}

fn claude_settings_file() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(crate::i18n::Msg::HomeDirNotFound)))?;
    Ok(home.join(".claude").join("settings.json"))
}

fn current_command(root: &Map<String, Value>) -> Option<String> {
    root.get("statusLine")?
        .get("command")?
        .as_str()
        .map(str::to_owned)
}

pub fn is_installed() -> bool {
    claude_settings_file()
        .and_then(|file| read_root(&file))
        .ok()
        .and_then(|root| current_command(&root))
        .is_some_and(|command| command.contains(STATUS_LINE_FLAG))
}

/// One shell line that feeds the same JSON to Termini and then to whatever the
/// user's status line already was, so theirs keeps producing the output Claude
/// Code displays.
fn chained_command(exe: &Path, wrapped: Option<&str>) -> String {
    let ours = format!(
        "printf '%s' \"$input\" | {} {STATUS_LINE_FLAG} >/dev/null 2>&1",
        quote_executable(exe)
    );
    match wrapped {
        Some(command) => format!("input=$(cat); {ours}; printf '%s' \"$input\" | {command}"),
        None => format!("input=$(cat); {ours}"),
    }
}

pub fn install(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let file = claude_settings_file()?;
    let mut root = read_root(&file)?;

    // Re-installing (e.g. after the executable moved) must not wrap our own
    // command a second time — the original one lives in Termini's config.
    let mut config = crate::config::store::load(app);
    let wrapped = match current_command(&root) {
        Some(command) if command.contains(STATUS_LINE_FLAG) => config.status_line_wrapped.clone(),
        other => other,
    };

    // One-time safety copy of the user's file before Termini first edits it.
    let backup = file.with_extension("json.termini-backup");
    if file.exists() && !backup.exists() {
        fs::copy(&file, &backup)?;
    }

    config.status_line_wrapped = wrapped.clone();
    crate::config::store::save(app, &config)?;

    root.insert(
        "statusLine".into(),
        json!({ "type": "command", "command": chained_command(&exe, wrapped.as_deref()) }),
    );
    write_root(&file, &root)
}

pub fn uninstall(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let file = claude_settings_file()?;
    if !file.exists() {
        return Ok(());
    }
    let mut root = read_root(&file)?;
    if !current_command(&root).is_some_and(|command| command.contains(STATUS_LINE_FLAG)) {
        return Ok(());
    }

    let mut config = crate::config::store::load(app);
    match config.status_line_wrapped.take() {
        Some(command) => {
            root.insert("statusLine".into(), json!({ "type": "command", "command": command }));
        }
        None => {
            root.remove("statusLine");
        }
    }
    crate::config::store::save(app, &config)?;
    write_root(&file, &root)
}

/// Keeps an installed status line pointed at this executable, which moves
/// between dev and installed builds. No-op when it isn't installed.
pub fn refresh_installed(app: &tauri::AppHandle) {
    if is_installed() {
        let _ = install(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "model": { "display_name": "Sonnet 5" },
        "context_window": { "used_percentage": 8.4 },
        "cost": { "total_cost_usd": 0.0123 },
        "rate_limits": {
            "five_hour": { "used_percentage": 23.5, "resets_at": 1789500000 },
            "seven_day": { "used_percentage": 41.2, "resets_at": 1790000000 }
        }
    }"#;

    #[test]
    fn reads_the_fields_the_status_bar_shows() {
        let snapshot = snapshot_from(SAMPLE).unwrap();
        assert_eq!(snapshot.model.as_deref(), Some("Sonnet 5"));
        assert_eq!(snapshot.five_hour_percentage, Some(23.5));
        assert_eq!(snapshot.seven_day_resets_at, Some(1790000000));
        assert_eq!(snapshot.context_used_percentage, Some(8.4));
        // Absent for everyone without a spend limit.
        assert_eq!(snapshot.spend_limit_percentage, None);
    }

    #[test]
    fn tolerates_a_payload_without_rate_limits() {
        let snapshot = snapshot_from(r#"{"model":{"display_name":"Opus"}}"#).unwrap();
        assert_eq!(snapshot.model.as_deref(), Some("Opus"));
        assert_eq!(snapshot.five_hour_percentage, None);
        assert!(snapshot_from("not json").is_none());
    }

    #[test]
    fn chains_onto_the_users_own_command() {
        let command = chained_command(Path::new("/opt/termini"), Some("bash ~/.claude/statusline.sh"));
        assert!(command.starts_with("input=$(cat);"));
        assert!(command.contains(STATUS_LINE_FLAG));
        assert!(command.ends_with("| bash ~/.claude/statusline.sh"));
    }

    #[test]
    fn stands_alone_when_there_was_no_status_line() {
        let command = chained_command(Path::new("/opt/termini"), None);
        assert!(command.contains(STATUS_LINE_FLAG));
        assert!(!command.contains("| bash"));
    }

    #[test]
    fn ignores_session_ids_that_could_escape_the_events_dir() {
        let base = std::env::temp_dir().join(format!("termini-status-test-{}", uuid::Uuid::new_v4()));
        let dir = base.join("events");
        fs::create_dir_all(&dir).unwrap();
        write_snapshot(&dir, "../escaped", &StatusSnapshot::default()).unwrap();
        let escaped = base.join("escaped.status.json").exists();
        let _ = fs::remove_dir_all(&base);
        assert!(!escaped);
    }
}
