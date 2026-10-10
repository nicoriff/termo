//! Plan usage for Codex consoles, so Modo IA's status bar shows the same
//! reading for both agents.
//!
//! Codex has no status line to chain onto the way Claude Code does (see
//! status_line.rs), but it doesn't need one: every hook payload carries a
//! `transcript_path`, the session's rollout, and Codex appends a
//! `token_count` event to it after each turn with the plan's rate-limit
//! windows, the turn's token usage and the model's context window. So the
//! numbers are already on disk — this reads the last one back on every hook
//! event and records it as that console's snapshot.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::status_line::{write_snapshot, StatusSnapshot, UsageWindow};

/// How much of the end of a rollout to read. A `token_count` event is the
/// last thing a turn writes, but tool results in between can be large, so
/// this is roomy enough to still find one after a noisy turn while never
/// reading a long conversation back in full.
const TAIL_BYTES: u64 = 512 * 1024;

/// Where Codex keeps its own state — `$CODEX_HOME`, or `~/.codex` when
/// unset, exactly how `agent_hooks::AgentTool::Codex` resolves it.
fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
}

/// Refuses a `transcript_path` outside `home`. Codex's own hook mechanism is
/// the only thing that sets this field today, not a plugin or MCP server —
/// but it's the one place this file opens and reads something named
/// entirely by a JSON field, so it costs nothing to keep it inside Codex's
/// own directory rather than trusting it as an arbitrary path to read.
/// `canonicalize` also closes off a `..` component or a symlink pointing
/// back out.
pub(super) fn is_within(path: &Path, home: &Path) -> bool {
    let (Ok(home), Ok(resolved)) = (fs::canonicalize(home), fs::canonicalize(path)) else {
        return false;
    };
    resolved.starts_with(home)
}

/// Records the usage Codex reported for `session`'s console, if its rollout
/// has any yet. Called for every Codex hook event, and quietly does nothing
/// when the payload has no transcript or the file has no usage in it: the
/// hook must never fail (see agent_hooks::run_hook_cli).
pub fn record(dir: &Path, session: &str, payload: &Value) {
    let Some(home) = codex_home() else { return };
    let Some(snapshot) = snapshot_from(payload, &home) else {
        return;
    };
    let _ = write_snapshot(dir, session, &snapshot);
}

fn snapshot_from(payload: &Value, codex_home: &Path) -> Option<StatusSnapshot> {
    let text = |key: &str| {
        payload
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let transcript = text("transcript_path")?;
    if !is_within(Path::new(&transcript), codex_home) {
        return None;
    }
    let usage = last_token_count(Path::new(&transcript))?;

    let number = |pointer: &str| usage.pointer(pointer).and_then(Value::as_f64);
    let integer = |pointer: &str| usage.pointer(pointer).and_then(Value::as_i64);
    let window = |name: &str| {
        number(&format!("/rate_limits/{name}/used_percent")).map(|percentage| UsageWindow {
            percentage,
            window_minutes: integer(&format!("/rate_limits/{name}/window_minutes")),
            resets_at: integer(&format!("/rate_limits/{name}/resets_at")),
        })
    };

    // What the model is carrying right now, which is the turn's own usage —
    // the running total counts every turn of the conversation, cached tokens
    // included, and would read far past the window it's measured against.
    let context_used_percentage = match (
        number("/info/last_token_usage/total_tokens"),
        number("/info/model_context_window"),
    ) {
        (Some(used), Some(window)) if window > 0.0 => Some(used / window * 100.0),
        _ => None,
    };

    Some(StatusSnapshot {
        tool: Some("codex".to_string()),
        agent_session_id: text("session_id"),
        cwd: text("cwd"),
        // Codex reports no project root of its own; the console's directory
        // is what Modo IA lists the conversation under.
        project_dir: None,
        model: text("model"),
        context_used_percentage,
        // Codex reports no cost for a subscription.
        cost_usd: None,
        plan_windows: [window("primary"), window("secondary")].into_iter().flatten().collect(),
        spend_limit_percentage: None,
    })
}

/// The `payload` of the last `token_count` event in a rollout. Rollouts are
/// one JSON object per line, so the tail is scanned backwards for the most
/// recent one; a first line cut in half by the seek simply fails to parse.
fn last_token_count(transcript: &Path) -> Option<Value> {
    let mut file = File::open(transcript).ok()?;
    let length = file.metadata().ok()?.len();
    if length > TAIL_BYTES {
        file.seek(SeekFrom::Start(length - TAIL_BYTES)).ok()?;
    }
    let mut tail = Vec::new();
    file.read_to_end(&mut tail).ok()?;

    String::from_utf8_lossy(&tail).lines().rev().find_map(|line| {
        let event: Value = serde_json::from_str(line).ok()?;
        let payload = event.get("payload")?;
        (payload.get("type").and_then(Value::as_str) == Some("token_count")).then(|| payload.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn rollout(lines: &[&str]) -> (std::path::PathBuf, Value) {
        let path = std::env::temp_dir().join(format!("termo-rollout-{}.jsonl", uuid::Uuid::new_v4()));
        let mut file = File::create(&path).unwrap();
        for line in lines {
            writeln!(file, "{line}").unwrap();
        }
        let payload = serde_json::json!({
            "session_id": "01a0c173-80ea-7911-967d-693454337a90",
            "cwd": "E:\\Proyectos\\Termini",
            "model": "gpt-6-astra",
            "transcript_path": path.to_string_lossy(),
        });
        (path, payload)
    }

    const TOKEN_COUNT: &str = r#"{"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"total_tokens":40559},"total_token_usage":{"total_tokens":189673},"model_context_window":258400},"rate_limits":{"primary":{"used_percent":18.0,"window_minutes":43200,"resets_at":1792540193},"secondary":null,"plan_type":"free"}}}"#;

    #[test]
    fn reads_the_plan_window_codex_last_reported() {
        let (path, payload) = rollout(&[r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#, TOKEN_COUNT]);
        let snapshot = snapshot_from(&payload, &std::env::temp_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(snapshot.tool.as_deref(), Some("codex"));
        assert_eq!(snapshot.model.as_deref(), Some("gpt-6-astra"));
        assert_eq!(snapshot.plan_windows.len(), 1, "a free plan has no secondary window");
        assert_eq!(snapshot.plan_windows[0].percentage, 18.0);
        assert_eq!(snapshot.plan_windows[0].window_minutes, Some(43200));
        // 40559 of a 258400-token context.
        let context = snapshot.context_used_percentage.unwrap();
        assert!((context - 15.7).abs() < 0.1, "context was {context}");
    }

    /// Each turn appends another one, and only the last is current.
    #[test]
    fn the_newest_reading_wins() {
        let newer = TOKEN_COUNT.replace("18.0", "42.0");
        let (path, payload) = rollout(&[TOKEN_COUNT, &newer]);
        let snapshot = snapshot_from(&payload, &std::env::temp_dir()).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(snapshot.plan_windows[0].percentage, 42.0);
    }

    /// Before the first response there is no reading, and a console with no
    /// usage yet must not be recorded as having 0%.
    #[test]
    fn a_rollout_without_usage_reports_nothing() {
        let (path, payload) = rollout(&[r#"{"type":"event_msg","payload":{"type":"agent_message"}}"#]);
        let snapshot = snapshot_from(&payload, &std::env::temp_dir());
        let _ = std::fs::remove_file(&path);
        assert!(snapshot.is_none());
    }

    #[test]
    fn a_payload_without_a_transcript_reports_nothing() {
        assert!(snapshot_from(&serde_json::json!({ "session_id": "x" }), &std::env::temp_dir()).is_none());
    }

    /// The hook payload names its own transcript; Codex's own hook mechanism
    /// is the only thing that sets this field today, but this is the one
    /// place this file opens and reads something a JSON field named. A
    /// rollout genuinely outside Codex's own directory is refused, however
    /// well-formed its content otherwise is.
    #[test]
    fn refuses_a_transcript_path_outside_codex_home() {
        let home = std::env::temp_dir().join(format!("termo-codexhome-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&home).unwrap();

        // rollout() writes under the bare system temp dir, not under `home`.
        let (outside_path, payload) = rollout(&[TOKEN_COUNT]);
        assert!(snapshot_from(&payload, &home).is_none(), "a transcript outside `home` must be refused");

        // The identical content, now genuinely under `home`, is accepted.
        let inside_path = home.join("session.jsonl");
        fs::copy(&outside_path, &inside_path).unwrap();
        let mut inside_payload = payload;
        inside_payload["transcript_path"] = serde_json::json!(inside_path.to_string_lossy());
        assert!(snapshot_from(&inside_payload, &home).is_some());

        let _ = fs::remove_file(&outside_path);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_transcript_path_that_does_not_exist_cannot_be_canonicalized_and_is_refused() {
        let home = std::env::temp_dir();
        let payload = serde_json::json!({ "transcript_path": home.join("no-such-file.jsonl").to_string_lossy() });
        assert!(snapshot_from(&payload, &home).is_none());
    }
}
