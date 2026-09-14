//! Opt-in integration with Claude Code and Codex lifecycle hooks, so Modo IA
//! knows a session's exact state instead of inferring it from the screen.
//!
//! Installing adds one hook entry per lifecycle event to the tool's user
//! hooks file (entries from every settings level merge, so the user's own
//! hooks stay untouched). Each entry runs Termini's hook script, which is a
//! no-op outside Termini: consoles spawned by Termini carry
//! TERMINI_SESSION_ID and TERMINI_EVENTS_DIR, and the script appends the
//! event to that session's file, which a watcher thread turns into
//! `agent-event` app events (see agent_events.rs for the line format).

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tauri::{Emitter, Manager};
use uuid::Uuid;

use super::agent_events::{parse_line, AgentUpdate};

const SCRIPT_NAME: &str = "termini-agent-hook";

const SCRIPT: &str = r#"#!/bin/sh
# Installed by Termini. Reports this agent's lifecycle events to the Termini
# tab it runs in; outside Termini these variables are unset and it does
# nothing. Always exits 0, so it can never block the agent.
if [ -z "$TERMINI_EVENTS_DIR" ] || [ -z "$TERMINI_SESSION_ID" ]; then
  cat >/dev/null
  exit 0
fi
detail=""
if [ "$2" = "Notification" ]; then
  detail=$(tr -d '\n\r' | sed -n 's/.*"notification_type"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
else
  cat >/dev/null
fi
printf '%s\t%s\t%s\n' "$1" "$2" "$detail" >> "$TERMINI_EVENTS_DIR/$TERMINI_SESSION_ID.jsonl" 2>/dev/null
exit 0
"#;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentTool {
    Claude,
    Codex,
}

impl AgentTool {
    fn name(self) -> &'static str {
        match self {
            AgentTool::Claude => "claude",
            AgentTool::Codex => "codex",
        }
    }

    fn events(self) -> &'static [&'static str] {
        match self {
            AgentTool::Claude => &[
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PermissionRequest",
                "Notification",
                "PostToolUse",
                "PostToolUseFailure",
                "Stop",
                "StopFailure",
                "SessionEnd",
            ],
            // Codex has no Notification/StopFailure events; Interrupt is its own.
            AgentTool::Codex => &[
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PermissionRequest",
                "PostToolUse",
                "Stop",
                "Interrupt",
                "SessionEnd",
            ],
        }
    }

    fn hooks_file(self) -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No se encontró el directorio home"))?;
        Ok(match self {
            AgentTool::Claude => home.join(".claude").join("settings.json"),
            AgentTool::Codex => std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".codex"))
                .join("hooks.json"),
        })
    }
}

pub fn events_dir(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("agent-events"))
}

fn script_path(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("hooks").join(SCRIPT_NAME))
}

/// (Re)writes the hook script, so installed hooks always run the script
/// version matching this Termini build.
pub fn write_script(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let path = script_path(app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, SCRIPT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(path)
}

fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn is_our_group(group: &Value) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            !hooks.is_empty()
                && hooks.iter().all(|hook| {
                    hook.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|command| command.contains(SCRIPT_NAME))
                })
        })
}

fn read_root(file: &Path) -> anyhow::Result<Map<String, Value>> {
    if !file.exists() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(file)?;
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&text)? {
        Value::Object(map) => Ok(map),
        _ => anyhow::bail!("{} no contiene un objeto JSON", file.display()),
    }
}

fn write_root(file: &Path, root: &Map<String, Value>) -> anyhow::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("termini-tmp");
    fs::write(&tmp, serde_json::to_string_pretty(root)? + "\n")?;
    fs::rename(&tmp, file)?;
    Ok(())
}

/// Removes Termini's entries, and any event list that ends up empty.
fn strip_ours(hooks: &mut Map<String, Value>) {
    for entries in hooks.values_mut() {
        if let Some(entries) = entries.as_array_mut() {
            entries.retain(|group| !is_our_group(group));
        }
    }
    hooks.retain(|_, entries| !entries.as_array().is_some_and(Vec::is_empty));
}

fn add_ours(root: &mut Map<String, Value>, tool: AgentTool, script: &Path) -> anyhow::Result<()> {
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("\"hooks\" no es un objeto"))?;
    strip_ours(hooks);
    for event in tool.events() {
        let command = format!("{} {} {}", sh_quote(&script.to_string_lossy()), tool.name(), event);
        hooks
            .entry(*event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("\"hooks.{event}\" no es una lista"))?
            .push(json!({ "hooks": [{ "type": "command", "command": command }] }));
    }
    Ok(())
}

fn remove_ours(root: &mut Map<String, Value>) {
    if let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) {
        strip_ours(hooks);
        if hooks.is_empty() {
            root.remove("hooks");
        }
    }
}

fn has_ours(root: &Map<String, Value>, tool: AgentTool) -> bool {
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    tool.events().iter().all(|event| {
        hooks
            .get(*event)
            .and_then(Value::as_array)
            .is_some_and(|entries| entries.iter().any(is_our_group))
    })
}

pub fn install(app: &tauri::AppHandle, tool: AgentTool) -> anyhow::Result<()> {
    let script = write_script(app)?;
    let file = tool.hooks_file()?;
    let mut root = read_root(&file)?;
    // One-time safety copy of the user's file before Termini first edits it.
    let backup = file.with_extension("json.termini-backup");
    if file.exists() && !backup.exists() {
        fs::copy(&file, &backup)?;
    }
    add_ours(&mut root, tool, &script)?;
    write_root(&file, &root)
}

pub fn uninstall(tool: AgentTool) -> anyhow::Result<()> {
    let file = tool.hooks_file()?;
    if !file.exists() {
        return Ok(());
    }
    let mut root = read_root(&file)?;
    remove_ours(&mut root);
    write_root(&file, &root)
}

pub fn is_installed(tool: AgentTool) -> bool {
    tool.hooks_file()
        .and_then(|file| read_root(&file))
        .is_ok_and(|root| has_ours(&root, tool))
}

pub fn forget_session(app: &tauri::AppHandle, id: Uuid) {
    if let Ok(dir) = events_dir(app) {
        let _ = fs::remove_file(dir.join(format!("{id}.jsonl")));
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AgentEventPayload {
    session_id: String,
    #[serde(flatten)]
    update: AgentUpdate,
}

/// Polls the events dir and emits `agent-event` for every new line.
pub fn start_watcher(app: tauri::AppHandle) {
    let Ok(dir) = events_dir(&app) else {
        return;
    };
    // Files left by a previous run belong to consoles that no longer exist.
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);

    std::thread::spawn(move || {
        let mut offsets: HashMap<PathBuf, u64> = HashMap::new();
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(true, |ext| ext != "jsonl") {
                    continue;
                }
                let Some(session_id) = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()) else {
                    continue;
                };
                let offset = offsets.entry(path.clone()).or_insert(0);
                for update in read_new_lines(&path, offset) {
                    let _ = app.emit(
                        "agent-event",
                        AgentEventPayload {
                            session_id: session_id.clone(),
                            update,
                        },
                    );
                }
            }
            offsets.retain(|path, _| path.exists());
        }
    });
}

fn read_new_lines(path: &Path, offset: &mut u64) -> Vec<AgentUpdate> {
    let Ok(mut file) = fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(len) = file.metadata().map(|meta| meta.len()) else {
        return Vec::new();
    };
    if len <= *offset || file.seek(SeekFrom::Start(*offset)).is_err() {
        return Vec::new();
    }
    let mut chunk = String::new();
    if file.take(len - *offset).read_to_string(&mut chunk).is_err() {
        return Vec::new();
    }
    // Consume only complete lines; a half-written one is re-read next tick.
    let Some(end) = chunk.rfind('\n') else {
        return Vec::new();
    };
    *offset += (end + 1) as u64;
    chunk[..=end].lines().filter_map(parse_line).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT_PATH: &str = "/Users/me/Library/Application Support/com.termini.app/hooks/termini-agent-hook";

    #[test]
    fn install_keeps_user_hooks_and_is_idempotent() {
        let mut root: Map<String, Value> = serde_json::from_value(json!({
            "theme": "dark",
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }] }
        }))
        .unwrap();
        let script = Path::new(SCRIPT_PATH);

        add_ours(&mut root, AgentTool::Claude, script).unwrap();
        add_ours(&mut root, AgentTool::Claude, script).unwrap();

        assert!(has_ours(&root, AgentTool::Claude));
        let stop = root["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "user hook plus exactly one Termini hook");
        assert_eq!(stop[0]["hooks"][0]["command"], "say done");
        assert_eq!(
            stop[1]["hooks"][0]["command"],
            format!("'{SCRIPT_PATH}' claude Stop")
        );
        assert_eq!(root["theme"], "dark");
    }

    #[test]
    fn uninstall_removes_only_termini_entries() {
        let mut root = Map::new();
        add_ours(&mut root, AgentTool::Codex, Path::new(SCRIPT_PATH)).unwrap();
        assert!(has_ours(&root, AgentTool::Codex));
        remove_ours(&mut root);
        assert!(!root.contains_key("hooks"));

        let mut with_user: Map<String, Value> = serde_json::from_value(json!({
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }] }
        }))
        .unwrap();
        add_ours(&mut with_user, AgentTool::Codex, Path::new(SCRIPT_PATH)).unwrap();
        remove_ours(&mut with_user);
        assert_eq!(with_user["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert!(with_user["hooks"].get("SessionStart").is_none());
    }

    #[test]
    fn script_reports_events_for_termini_consoles_only() {
        let dir = std::env::temp_dir().join(format!("termini-hook-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let script = dir.join(SCRIPT_NAME);
        fs::write(&script, SCRIPT).unwrap();

        let run = |event: &str, stdin: &str, with_env: bool| {
            use std::io::Write;
            let mut command = std::process::Command::new("sh");
            command.arg(&script).args(["claude", event]).stdin(std::process::Stdio::piped());
            command.env_remove("TERMINI_EVENTS_DIR").env_remove("TERMINI_SESSION_ID");
            if with_env {
                command.env("TERMINI_EVENTS_DIR", &dir).env("TERMINI_SESSION_ID", "s1");
            }
            let mut child = command.spawn().unwrap();
            child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
            assert!(child.wait().unwrap().success());
        };

        run("Stop", r#"{"hook_event_name":"Stop"}"#, false);
        assert!(!dir.join("s1.jsonl").exists(), "no-op outside Termini");

        run("Notification", "{\n \"notification_type\": \"permission_prompt\",\n \"message\": \"x\"\n}", true);
        run("Stop", r#"{"last_assistant_message":"done"}"#, true);

        let mut offset = 0;
        let updates = read_new_lines(&dir.join("s1.jsonl"), &mut offset);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            updates.iter().map(|u| u.status).collect::<Vec<_>>(),
            [
                Some(super::super::agent_events::AgentStatus::Awaiting),
                Some(super::super::agent_events::AgentStatus::Finished)
            ]
        );
    }
}
