//! The context window of Grok consoles, for Modo IA's status bar.
//!
//! Grok has a status line Termo could chain onto, as it does Claude Code's
//! and Copilot CLI's, but it's off by default and turning it on puts a row
//! in the user's own screen. It isn't needed: Grok keeps each session's
//! numbers in a folder of its own (`signals.json`: how full the context is,
//! and the model), and every hook payload carries a `transcriptPath` into
//! that folder. So, as with Codex's rollout (codex_usage.rs), the reading is
//! already on disk — this takes it on every hook event and records it as
//! that console's snapshot. Grok reports no plan usage anywhere.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::codex_usage::is_within;
use super::status_line::{write_snapshot, StatusSnapshot};

fn grok_home() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".grok"))
}

/// Records what Grok last noted about `session`'s console, if it has noted
/// anything yet. Called for every Grok hook event, and quietly does nothing
/// otherwise: the hook must never fail (see agent_hooks::run_hook_cli).
pub fn record(dir: &Path, session: &str, payload: &Value) {
    let Some(home) = grok_home() else { return };
    let Some(snapshot) = snapshot_from(payload, &home) else {
        return;
    };
    let _ = write_snapshot(dir, session, &snapshot);
}

fn snapshot_from(payload: &Value, grok_home: &Path) -> Option<StatusSnapshot> {
    let text = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| payload.get(*key).and_then(Value::as_str))
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    // Its payloads name things both ways; the session's folder is where its
    // transcript is.
    let transcript = text(&["transcriptPath", "transcript_path"])?;
    let folder = Path::new(&transcript).parent()?;
    // Named by a JSON field: read only inside Grok's own directory.
    if !is_within(folder, grok_home) {
        return None;
    }
    let signals: Value = serde_json::from_str(&std::fs::read_to_string(folder.join("signals.json")).ok()?).ok()?;
    let number = |key: &str| signals.get(key).and_then(Value::as_f64);
    let context_tokens = number("contextTokensUsed");
    let context_window_tokens = number("contextWindowTokens");
    let context_used_percentage = match (context_tokens, context_window_tokens) {
        (Some(used), Some(window)) if window > 0.0 => Some(used / window * 100.0),
        // The whole number it keeps beside them.
        _ => number("contextWindowUsage"),
    };
    Some(StatusSnapshot {
        tool: Some("grok".to_string()),
        agent_session_id: text(&["sessionId", "session_id"]),
        cwd: text(&["cwd"]),
        project_dir: None,
        model: signals.get("primaryModelId").and_then(Value::as_str).filter(|id| !id.is_empty()).map(str::to_owned),
        context_used_percentage,
        // What it notes as cost is the API's price, whatever the account pays.
        cost_usd: None,
        plan_windows: Vec::new(),
        spend_limit_percentage: None,
        context_tokens,
        context_window_tokens,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_folder(signals: Option<&str>) -> (PathBuf, Value) {
        let folder = std::env::temp_dir().join(format!("termo-grok-session-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("updates.jsonl"), "").unwrap();
        if let Some(signals) = signals {
            std::fs::write(folder.join("signals.json"), signals).unwrap();
        }
        let payload = serde_json::json!({
            "hookEventName": "stop",
            "sessionId": "01a12368-242e-71c0-94be-8016b627e431",
            "cwd": "E:/Proyectos/Termini",
            "transcriptPath": folder.join("updates.jsonl").to_string_lossy(),
        });
        (folder, payload)
    }

    /// What Grok 1.0.50 keeps, trimmed.
    const SIGNALS: &str = r#"{"turnCount":1,"contextWindowUsage":7,"contextTokensUsed":19564,"contextWindowTokens":256000,"primaryModelId":"grok-4.7"}"#;

    #[test]
    fn reads_how_full_the_context_is_from_the_sessions_folder() {
        let (folder, payload) = session_folder(Some(SIGNALS));
        let snapshot = snapshot_from(&payload, &std::env::temp_dir()).unwrap();
        let _ = std::fs::remove_dir_all(&folder);

        assert_eq!(snapshot.tool.as_deref(), Some("grok"));
        assert_eq!(snapshot.agent_session_id.as_deref(), Some("01a12368-242e-71c0-94be-8016b627e431"));
        assert_eq!(snapshot.cwd.as_deref(), Some("E:/Proyectos/Termini"));
        assert_eq!(snapshot.model.as_deref(), Some("grok-4.7"));
        let used = snapshot.context_used_percentage.unwrap();
        assert!((used - 7.642).abs() < 0.01, "{used}");
        assert!(snapshot.plan_windows.is_empty() && snapshot.cost_usd.is_none());
    }

    #[test]
    fn nothing_without_its_notes_or_outside_its_own_directory() {
        // A session that hasn't noted anything yet.
        let (folder, payload) = session_folder(None);
        assert!(snapshot_from(&payload, &std::env::temp_dir()).is_none());
        let _ = std::fs::remove_dir_all(&folder);

        // A transcript somewhere else is not read.
        let (folder, payload) = session_folder(Some(SIGNALS));
        let elsewhere = std::env::temp_dir().join(format!("termo-grok-home-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert!(snapshot_from(&payload, &elsewhere).is_none());
        let _ = std::fs::remove_dir_all(&folder);
        let _ = std::fs::remove_dir_all(&elsewhere);

        // The first event of a session has no transcript yet.
        assert!(snapshot_from(&serde_json::json!({ "sessionId": "s" }), &std::env::temp_dir()).is_none());
    }
}
