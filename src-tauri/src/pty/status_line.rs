//! Opt-in integration with Claude Code's status line, so Termo can show a
//! console's plan usage (the 5h/7d rate-limit windows), context window and
//! session cost on its own status bar.
//!
//! Claude Code allows a single `statusLine` per user and runs its `command` in
//! a shell, so installing *chains* onto whatever the user already had instead
//! of replacing it: the same JSON is piped to Termo (which only records it)
//! and then to their original command, whose output is what Claude Code keeps
//! displaying. The original command is stored in Termo's config, so
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

/// One of a plan's rate-limit windows. `window_minutes` is what names it in
/// the UI (300 is the 5-hour window, 10080 the weekly one), since the tools
/// don't agree on which windows a plan even has: Claude reports a 5-hour and
/// a weekly one, Codex a primary and an optional secondary whose length
/// depends on the plan — a free one is a single 30-day window.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub percentage: f64,
    pub window_minutes: Option<i64>,
    pub resets_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    /// Which agent this came from, so the watcher records the conversation
    /// under the right tool. Absent in snapshots written before Codex
    /// reported any, which were all Claude's.
    pub tool: Option<String>,
    /// The agent's own session id, cwd and project root: what Modo IA needs
    /// to list this conversation and resume it later (agent_sessions.rs).
    pub agent_session_id: Option<String>,
    pub cwd: Option<String>,
    pub project_dir: Option<String>,
    pub model: Option<String>,
    pub context_used_percentage: Option<f64>,
    /// Claude only — Codex reports no cost for a subscription.
    pub cost_usd: Option<f64>,
    pub plan_windows: Vec<UsageWindow>,
    pub spend_limit_percentage: Option<f64>,
}

impl StatusSnapshot {
    /// Whether the conversation has anything in it: the status line is drawn
    /// from the moment the agent opens, before a word is said, and nothing
    /// is spent or held in context until one is.
    pub fn has_begun(&self) -> bool {
        self.cost_usd.is_some_and(|cost| cost > 0.0) || self.context_used_percentage.is_some_and(|used| used > 0.0)
    }
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

    // Claude's two windows are fixed, so their length is known here rather
    // than reported: 5 hours and 7 days, in minutes.
    let window = |name: &str, minutes: i64| {
        number(&format!("/rate_limits/{name}/used_percentage")).map(|percentage| UsageWindow {
            percentage,
            window_minutes: Some(minutes),
            resets_at: integer(&format!("/rate_limits/{name}/resets_at")),
        })
    };

    Some(StatusSnapshot {
        tool: Some("claude".to_string()),
        agent_session_id: text("/session_id"),
        cwd: text("/cwd").or_else(|| text("/workspace/current_dir")),
        project_dir: text("/workspace/project_dir"),
        model: payload
            .pointer("/model/display_name")
            .and_then(Value::as_str)
            .map(str::to_owned),
        context_used_percentage: number("/context_window/used_percentage"),
        cost_usd: number("/cost/total_cost_usd"),
        plan_windows: [window("five_hour", 300), window("seven_day", 7 * 24 * 60)]
            .into_iter()
            .flatten()
            .collect(),
        spend_limit_percentage: number("/rate_limits/spend_limit/used_percentage"),
    })
}

/// `termo --statusline`, run by Claude Code on every status-line refresh with
/// the session JSON on stdin. Records the snapshot for this console and returns
/// normally whatever happens: it must never disturb the user's own status line.
pub fn run_status_line_cli() {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let (Some(dir), Ok(session)) = (
        std::env::var_os("TERMO_EVENTS_DIR"),
        std::env::var("TERMO_SESSION_ID"),
    ) else {
        return;
    };
    let Some(snapshot) = snapshot_from(&input) else {
        return;
    };
    let _ = write_snapshot(Path::new(&dir), &session, &snapshot);
}

pub(super) fn write_snapshot(dir: &Path, session: &str, snapshot: &StatusSnapshot) -> std::io::Result<()> {
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
                    // Only the status line wrote these before Codex reported
                    // its own usage, and that one is Claude's alone.
                    let tool = snapshot.tool.clone().unwrap_or_else(|| "claude".to_string());
                    crate::agent_sessions::record(
                        &app,
                        crate::agent_sessions::AgentSession::new(&tool, &agent_session_id)
                            .with_cwd(snapshot.cwd.clone())
                            .with_project_dir(snapshot.project_dir.clone())
                            .with_model(snapshot.model.clone()),
                        snapshot.has_begun(),
                    );
                }
                let _ = app.emit("agent-status", StatusEvent { session_id, snapshot });
            }
            seen.retain(|path, _| path.exists());
        }
    });
}

/// Sends a console's last snapshot again: the app attached to a console the
/// sessions daemon kept, and the snapshot it wrote meanwhile was sent to no
/// one (see agent_hooks::restore).
pub fn restore(app: &tauri::AppHandle, dir: &std::path::Path, id: &str) {
    let path = dir.join(format!("{id}{SNAPSHOT_SUFFIX}"));
    let Some(snapshot) = fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<StatusSnapshot>(&text).ok())
    else {
        return;
    };
    let _ = app.emit("agent-status", StatusEvent { session_id: id.to_string(), snapshot });
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

/// One shell line that feeds the same JSON to Termo and then to whatever the
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
    // command a second time — the original one lives in Termo's config.
    let mut config = crate::config::store::load(app);
    let wrapped = match current_command(&root) {
        Some(command) if command.contains(STATUS_LINE_FLAG) => config.status_line_wrapped.clone(),
        other => other,
    };

    // One-time safety copy of the user's file before Termo first edits it.
    let backup = file.with_extension("json.termo-backup");
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
        assert_eq!(snapshot.context_used_percentage, Some(8.4));
        // Claude's windows are fixed, so their length is filled in here for
        // the status bar to name them the same way it names Codex's.
        let windows: Vec<_> = snapshot
            .plan_windows
            .iter()
            .map(|window| (window.percentage, window.window_minutes, window.resets_at))
            .collect();
        assert_eq!(
            windows,
            [(23.5, Some(300), Some(1789500000)), (41.2, Some(10080), Some(1790000000))]
        );
        // Absent for everyone without a spend limit.
        assert_eq!(snapshot.spend_limit_percentage, None);
    }

    #[test]
    fn tolerates_a_payload_without_rate_limits() {
        let snapshot = snapshot_from(r#"{"model":{"display_name":"Opus"}}"#).unwrap();
        assert_eq!(snapshot.model.as_deref(), Some("Opus"));
        assert!(snapshot.plan_windows.is_empty());
        assert!(snapshot_from("not json").is_none());
    }

    #[test]
    fn chains_onto_the_users_own_command() {
        let command = chained_command(Path::new("/opt/termo"), Some("bash ~/.claude/statusline.sh"));
        assert!(command.starts_with("input=$(cat);"));
        assert!(command.contains(STATUS_LINE_FLAG));
        assert!(command.ends_with("| bash ~/.claude/statusline.sh"));
    }

    #[test]
    fn stands_alone_when_there_was_no_status_line() {
        let command = chained_command(Path::new("/opt/termo"), None);
        assert!(command.contains(STATUS_LINE_FLAG));
        assert!(!command.contains("| bash"));
    }

    #[test]
    fn ignores_session_ids_that_could_escape_the_events_dir() {
        let base = std::env::temp_dir().join(format!("termo-status-test-{}", uuid::Uuid::new_v4()));
        let dir = base.join("events");
        fs::create_dir_all(&dir).unwrap();
        write_snapshot(&dir, "../escaped", &StatusSnapshot::default()).unwrap();
        let escaped = base.join("escaped.status.json").exists();
        let _ = fs::remove_dir_all(&base);
        assert!(!escaped);
    }
}
