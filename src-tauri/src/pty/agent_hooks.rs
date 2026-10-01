//! Opt-in integration with Claude Code, Codex, Gemini CLI and Copilot CLI lifecycle hooks, so Modo IA
//! knows a session's exact state instead of inferring it from the screen.
//!
//! Installing adds one hook entry per lifecycle event to the tool's user
//! hooks file (entries from every settings level merge, so the user's own
//! hooks stay untouched). Each entry runs Termo's own executable as
//! `termo --agent-hook <tool> <event>` (see `run_hook_cli`), so it behaves
//! the same on macOS, Linux and Windows without a shell script. It is a no-op
//! outside Termo: consoles spawned by Termo carry TERMO_SESSION_ID and
//! TERMO_EVENTS_DIR, the hook appends the event to that session's file, and
//! a watcher thread turns new lines into `agent-event` app events (see
//! agent_events.rs for the line format).

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tauri::{Emitter, Manager};
use uuid::Uuid;

use super::agent_events::{parse_line, AgentUpdate};

/// First argument that switches the executable into hook mode.
pub const HOOK_FLAG: &str = "--agent-hook";

/// The POSIX sh script earlier builds installed as the hook. Entries still
/// pointing at it count as Termo's, so they get migrated or removed.
const LEGACY_SCRIPT_NAME: &str = "termini-agent-hook";

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentTool {
    Claude,
    Codex,
    Gemini,
    Copilot,
}

impl AgentTool {
    const ALL: [AgentTool; 4] = [AgentTool::Claude, AgentTool::Codex, AgentTool::Gemini, AgentTool::Copilot];

    fn name(self) -> &'static str {
        match self {
            AgentTool::Claude => "claude",
            AgentTool::Codex => "codex",
            AgentTool::Gemini => "gemini",
            AgentTool::Copilot => "copilot",
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
            // Gemini CLI's own names: a turn runs BeforeAgent…AfterAgent, and
            // a permission prompt is a Notification of type ToolPermission.
            AgentTool::Gemini => &[
                "SessionStart",
                "BeforeAgent",
                "BeforeTool",
                "AfterTool",
                "Notification",
                "AfterAgent",
                "SessionEnd",
            ],
            // Copilot CLI's own names (its camelCase ones are the ones its
            // user-level hooks document).
            AgentTool::Copilot => &[
                "sessionStart",
                "userPromptSubmitted",
                "preToolUse",
                "postToolUse",
                "postToolUseFailure",
                "permissionRequest",
                "agentStop",
                "errorOccurred",
                "sessionEnd",
            ],
        }
    }

    fn hooks_file(self) -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(crate::i18n::Msg::HomeDirNotFound)))?;
        Ok(match self {
            AgentTool::Claude => home.join(".claude").join("settings.json"),
            AgentTool::Codex => std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".codex"))
                .join("hooks.json"),
            // Its user settings, where hooks sit beside everything else.
            AgentTool::Gemini => home.join(".gemini").join("settings.json"),
            // A folder of hook files, one per owner: this one is Termo's alone.
            AgentTool::Copilot => crate::agents::spec::copilot_home(&home).join("hooks").join("termo.json"),
        })
    }

    /// Whether the hooks file is Termo's own, written whole, rather than the
    /// tool's shared settings that Termo's entries are merged into.
    fn owns_file(self) -> bool {
        self == AgentTool::Copilot
    }
}

pub fn events_dir(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("agent-events"))
}

fn hook_command(exe: &Path, tool: AgentTool, event: &str) -> String {
    format!(
        "{}{} {HOOK_FLAG} {} {event}",
        call_prefix(tool),
        quote_executable(exe),
        tool.name()
    )
}

/// Codex and Gemini CLI run their hooks through PowerShell on Windows, and there a statement
/// that opens with a quoted path is an expression, not a command: the
/// arguments after it are a syntax error ("Unexpected token"), so the hook
/// never ran and Modo IA never heard from Codex. The call operator makes it
/// a command again. Claude Code runs its hooks through Git Bash instead,
/// where a leading `&` would put the command in the background.
#[cfg(windows)]
fn call_prefix(tool: AgentTool) -> &'static str {
    match tool {
        AgentTool::Codex | AgentTool::Gemini | AgentTool::Copilot => "& ",
        AgentTool::Claude => "",
    }
}

#[cfg(not(windows))]
fn call_prefix(_tool: AgentTool) -> &'static str {
    ""
}

/// Hook commands run through `sh -c` on macOS/Linux and Git Bash on Windows,
/// so the path is quoted for a POSIX shell.
#[cfg(not(windows))]
pub(super) fn quote_executable(exe: &Path) -> String {
    format!("'{}'", exe.to_string_lossy().replace('\'', r"'\''"))
}

/// Double quotes around a forward-slash path: a backslash is an escape
/// character in bash, while Windows accepts `/` as a separator.
#[cfg(windows)]
pub(super) fn quote_executable(exe: &Path) -> String {
    let mut quoted = String::from("\"");
    for c in exe.to_string_lossy().replace('\\', "/").chars() {
        if matches!(c, '"' | '$' | '`') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

fn group_commands(group: &Value) -> impl Iterator<Item = &str> {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|hook| hook.get("command").and_then(Value::as_str))
}

fn is_our_group(group: &Value) -> bool {
    let mut commands = group_commands(group).peekable();
    commands.peek().is_some()
        && commands.all(|command| command.contains(HOOK_FLAG) || command.contains(LEGACY_SCRIPT_NAME))
}

pub(super) fn read_root(file: &Path) -> anyhow::Result<Map<String, Value>> {
    if !file.exists() {
        return Ok(Map::new());
    }
    let text = fs::read_to_string(file)?;
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(&text)? {
        Value::Object(map) => Ok(map),
        _ => anyhow::bail!(crate::i18n::tr(crate::i18n::Msg::NotAJsonObject(
            &file.display().to_string()
        ))),
    }
}

pub(super) fn write_root(file: &Path, root: &Map<String, Value>) -> anyhow::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("termo-tmp");
    fs::write(&tmp, serde_json::to_string_pretty(root)? + "\n")?;
    fs::rename(&tmp, file)?;
    Ok(())
}

/// Keys of Gemini CLI's `hooks` object that are its own settings, not
/// events: never touched here (`disabled: []` is the user's, empty or not).
const HOOKS_SETTINGS: &[&str] = &["enabled", "disabled", "notifications"];

/// Removes Termo's entries, and any event list that ends up empty.
fn strip_ours(hooks: &mut Map<String, Value>) {
    for (event, entries) in hooks.iter_mut() {
        if HOOKS_SETTINGS.contains(&event.as_str()) {
            continue;
        }
        if let Some(entries) = entries.as_array_mut() {
            entries.retain(|group| !is_our_group(group));
        }
    }
    hooks.retain(|event, entries| {
        HOOKS_SETTINGS.contains(&event.as_str()) || !entries.as_array().is_some_and(Vec::is_empty)
    });
}

fn add_ours(root: &mut Map<String, Value>, tool: AgentTool, exe: &Path) -> anyhow::Result<()> {
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(crate::i18n::Msg::HooksNotAnObject)))?;
    strip_ours(hooks);
    for event in tool.events() {
        hooks
            .entry(*event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(crate::i18n::Msg::HooksEventNotAList(event))))?
            .push(json!({ "hooks": [{ "type": "command", "command": hook_command(exe, tool, event) }] }));
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

fn has_any_ours(root: &Map<String, Value>) -> bool {
    root.get("hooks").and_then(Value::as_object).is_some_and(|hooks| {
        hooks
            .values()
            .filter_map(Value::as_array)
            .any(|entries| entries.iter().any(is_our_group))
    })
}

/// Copilot CLI's hook file, Termo's own (see AgentTool::owns_file): every
/// event runs `termo --agent-hook copilot <event>`, through PowerShell on
/// Windows and bash elsewhere — Copilot runs whichever field fits the
/// platform, so both are always there.
fn copilot_root(exe: &Path) -> Map<String, Value> {
    let posix = format!("'{}'", exe.to_string_lossy().replace('\'', r"'\''"));
    let powershell = format!("& '{}'", exe.to_string_lossy().replace('\'', "''"));
    let hooks: Map<String, Value> = AgentTool::Copilot
        .events()
        .iter()
        .map(|event| {
            let args = format!("{HOOK_FLAG} copilot {event}");
            let entry = json!({
                "type": "command",
                "bash": format!("{posix} {args}"),
                "powershell": format!("{powershell} {args}"),
                "timeoutSec": 10,
            });
            (event.to_string(), json!([entry]))
        })
        .collect();
    let mut root = Map::new();
    root.insert("version".into(), json!(1));
    root.insert("hooks".into(), Value::Object(hooks));
    root
}

/// Whether a Copilot hook file is Termo's: every command in it runs Termo.
fn is_copilot_file_ours(root: &Map<String, Value>) -> bool {
    let commands: Vec<&str> = root
        .get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|hooks| hooks.values())
        .filter_map(Value::as_array)
        .flatten()
        .flat_map(|entry| ["bash", "powershell"].map(|key| entry.get(key).and_then(Value::as_str)))
        .flatten()
        .collect();
    !commands.is_empty() && commands.iter().all(|command| command.contains(HOOK_FLAG))
}

pub fn install(tool: AgentTool) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let file = tool.hooks_file()?;
    if tool.owns_file() {
        return write_root(&file, &copilot_root(&exe));
    }
    let mut root = read_root(&file)?;
    // One-time safety copy of the user's file before Termo first edits it.
    let backup = file.with_extension("json.termo-backup");
    if file.exists() && !backup.exists() {
        fs::copy(&file, &backup)?;
    }
    add_ours(&mut root, tool, &exe)?;
    write_root(&file, &root)
}

pub fn uninstall(tool: AgentTool) -> anyhow::Result<()> {
    let file = tool.hooks_file()?;
    if !file.exists() {
        return Ok(());
    }
    let mut root = read_root(&file)?;
    if tool.owns_file() {
        // Only a file that is Termo's: one by that name the user made stays.
        if is_copilot_file_ours(&root) {
            fs::remove_file(&file)?;
        }
        return Ok(());
    }
    remove_ours(&mut root);
    write_root(&file, &root)
}

pub fn is_installed(tool: AgentTool) -> bool {
    tool.hooks_file().and_then(|file| read_root(&file)).is_ok_and(|root| {
        if tool.owns_file() {
            is_copilot_file_ours(&root)
        } else {
            has_ours(&root, tool)
        }
    })
}

/// Points already-installed hooks at this executable (it moves between dev
/// and installed builds, or on update) and migrates entries left by the old
/// shell-script hook. Files without Termo hooks are never touched, and a
/// file is only rewritten when something actually changes.
pub fn refresh_installed(app: &tauri::AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let legacy = dir.join("hooks");
        let _ = fs::remove_file(legacy.join(LEGACY_SCRIPT_NAME));
        let _ = fs::remove_dir(legacy);
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    for tool in AgentTool::ALL {
        let Ok(file) = tool.hooks_file() else {
            continue;
        };
        let Ok(root) = read_root(&file) else {
            continue;
        };
        if tool.owns_file() {
            let updated = copilot_root(&exe);
            if is_copilot_file_ours(&root) && updated != root {
                let _ = write_root(&file, &updated);
            }
            continue;
        }
        if !has_any_ours(&root) {
            continue;
        }
        let mut updated = root.clone();
        if add_ours(&mut updated, tool, &exe).is_ok() && updated != root {
            let _ = write_root(&file, &updated);
        }
    }
}

/// `termo --agent-hook <tool> <event>`, run by the agent for each lifecycle
/// event with the event's JSON on stdin. Never fails: whatever happens it
/// returns normally (exit code 0), so it can never block the agent.
pub fn run_hook_cli(tool: &str, event: &str) {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let (Some(dir), Ok(session)) = (std::env::var_os("TERMO_EVENTS_DIR"), std::env::var("TERMO_SESSION_ID"))
    else {
        return;
    };
    let dir = Path::new(&dir);
    // Codex has no status line to chain onto, so its plan usage is read from
    // the rollout each event points at (see codex_usage).
    if tool == AgentTool::Codex.name() {
        if let Ok(payload) = serde_json::from_str::<Value>(&input) {
            super::codex_usage::record(dir, &session, &payload);
        }
    }
    let _ = append_event(dir, &session, tool, event, &input);
}

fn append_event(dir: &Path, session: &str, tool: &str, event: &str, input: &str) -> std::io::Result<()> {
    // Session ids are UUIDs; anything else must not name a path outside `dir`.
    if session.is_empty() || !session.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Ok(());
    }
    let payload = serde_json::from_str::<Value>(input).ok();
    let string_at = |key: &str| {
        payload
            .as_ref()
            .and_then(|value| value.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    // Only Notification needs a field from the payload body; large payloads
    // (tool results) never touch the file.
    let detail = if event == "Notification" {
        string_at("notification_type")
    } else {
        String::new()
    };
    // Common hook fields: they identify the agent's own conversation, so Modo
    // IA can list it and resume it later (agent_sessions.rs).
    // Copilot CLI's payloads are camelCase.
    let agent_session_id = match string_at("session_id") {
        id if id.is_empty() => string_at("sessionId"),
        id => id,
    };
    let cwd = string_at("cwd");

    let field = |value: &str| value.replace(['\t', '\r', '\n'], " ");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{session}.jsonl")))?;
    writeln!(
        file,
        "{}\t{}\t{}\t{}\t{}",
        field(tool),
        field(event),
        field(&detail),
        field(&agent_session_id),
        field(&cwd)
    )
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
                    if let Some(agent_session_id) = update.agent_session_id.clone() {
                        crate::agent_sessions::record(
                            &app,
                            crate::agent_sessions::AgentSession::new(&update.tool, &agent_session_id)
                                .with_cwd(update.agent_cwd.clone()),
                        );
                    }
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
    use super::super::agent_events::AgentStatus;
    use super::*;

    const EXE: &str = "/Applications/Termo.app/Contents/MacOS/termo";

    fn exe() -> &'static Path {
        Path::new(EXE)
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-hook-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn install_keeps_user_hooks_and_is_idempotent() {
        let mut root: Map<String, Value> = serde_json::from_value(json!({
            "theme": "dark",
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }] }
        }))
        .unwrap();

        add_ours(&mut root, AgentTool::Claude, exe()).unwrap();
        add_ours(&mut root, AgentTool::Claude, exe()).unwrap();

        assert!(has_ours(&root, AgentTool::Claude));
        let stop = root["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "user hook plus exactly one Termo hook");
        assert_eq!(stop[0]["hooks"][0]["command"], "say done");
        assert_eq!(stop[1]["hooks"][0]["command"], hook_command(exe(), AgentTool::Claude, "Stop"));
        assert_eq!(root["theme"], "dark");
    }

    #[test]
    fn uninstall_removes_only_termo_entries() {
        let mut root = Map::new();
        add_ours(&mut root, AgentTool::Codex, exe()).unwrap();
        assert!(has_ours(&root, AgentTool::Codex));
        remove_ours(&mut root);
        assert!(!root.contains_key("hooks"));

        let mut with_user: Map<String, Value> = serde_json::from_value(json!({
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }] }
        }))
        .unwrap();
        add_ours(&mut with_user, AgentTool::Codex, exe()).unwrap();
        remove_ours(&mut with_user);
        assert_eq!(with_user["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert!(with_user["hooks"].get("SessionStart").is_none());
    }

    #[test]
    fn migrates_entries_from_the_legacy_script() {
        let mut root: Map<String, Value> = serde_json::from_value(json!({
            "hooks": { "Stop": [{ "hooks": [{
                "type": "command",
                "command": "'/Users/me/Library/Application Support/com.ln.termo/hooks/termini-agent-hook' claude Stop"
            }] }] }
        }))
        .unwrap();
        assert!(has_any_ours(&root));

        add_ours(&mut root, AgentTool::Claude, exe()).unwrap();

        let stop = root["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1, "the legacy entry is replaced, not kept alongside");
        assert_eq!(stop[0]["hooks"][0]["command"], hook_command(exe(), AgentTool::Claude, "Stop"));
    }

    /// Gemini CLI keeps its hooks in its whole settings file, beside its own
    /// `hooks` settings: Termo adds its entries and takes them away without
    /// touching either.
    #[test]
    fn gemini_settings_keep_everything_but_termos_entries() {
        let original: Map<String, Value> = serde_json::from_value(json!({
            "security": { "auth": { "selectedType": "gemini-api-key" } },
            "hooks": {
                "enabled": true,
                "disabled": [],
                "AfterAgent": [{ "hooks": [{ "type": "command", "command": "say done" }] }]
            }
        }))
        .unwrap();
        let mut root = original.clone();

        add_ours(&mut root, AgentTool::Gemini, exe()).unwrap();
        assert!(has_ours(&root, AgentTool::Gemini));
        for event in ["SessionStart", "BeforeAgent", "BeforeTool", "AfterTool", "Notification", "AfterAgent", "SessionEnd"] {
            assert!(root["hooks"][event].as_array().is_some(), "{event}");
        }
        assert_eq!(root["hooks"]["AfterAgent"].as_array().unwrap().len(), 2);
        assert_eq!(
            root["hooks"]["BeforeAgent"][0]["hooks"][0]["command"],
            hook_command(exe(), AgentTool::Gemini, "BeforeAgent")
        );

        remove_ours(&mut root);
        assert_eq!(root, original);
    }

    /// Copilot CLI gets a hook file of its own, with a bash and a PowerShell
    /// command per event since it runs whichever fits the platform.
    #[test]
    fn copilot_hook_file_runs_termo_for_every_event() {
        let exe = Path::new("/opt/Ter'mo/termo");
        let root = copilot_root(exe);
        assert_eq!(root["version"], 1);
        for event in AgentTool::Copilot.events() {
            let entry = &root["hooks"][*event][0];
            assert_eq!(entry["type"], "command");
            assert_eq!(entry["bash"], format!(r"'/opt/Ter'\''mo/termo' --agent-hook copilot {event}"));
            assert_eq!(entry["powershell"], format!("& '/opt/Ter''mo/termo' --agent-hook copilot {event}"));
        }
        assert!(is_copilot_file_ours(&root));
    }

    #[test]
    fn a_copilot_hook_file_of_the_users_is_not_ours() {
        let users: Map<String, Value> = serde_json::from_value(json!({
            "version": 1,
            "hooks": { "agentStop": [{ "type": "command", "bash": "say done" }] }
        }))
        .unwrap();
        assert!(!is_copilot_file_ours(&users));
        assert!(!is_copilot_file_ours(&Map::new()));
    }

    #[test]
    fn copilot_payloads_name_the_session_in_camel_case() {
        let dir = temp_dir();
        append_event(&dir, "s1", "copilot", "agentStop", r#"{"sessionId":"abc-1","cwd":"E:/proj"}"#).unwrap();
        let mut offset = 0;
        let updates = read_new_lines(&dir.join("s1.jsonl"), &mut offset);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].status, Some(AgentStatus::Finished));
        assert_eq!(updates[0].agent_session_id.as_deref(), Some("abc-1"));
        assert_eq!(updates[0].agent_cwd.as_deref(), Some("E:/proj"));
    }

    #[test]
    fn files_without_termo_hooks_are_not_ours() {
        let root: Map<String, Value> = serde_json::from_value(json!({
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }] }
        }))
        .unwrap();
        assert!(!has_any_ours(&root));
    }

    #[cfg(windows)]
    #[test]
    fn command_quotes_a_windows_path_for_bash() {
        assert_eq!(
            hook_command(Path::new(r"C:\Program Files\Termo\termo.exe"), AgentTool::Claude, "Stop"),
            "\"C:/Program Files/Termo/termo.exe\" --agent-hook claude Stop"
        );
    }

    /// Codex's and Gemini CLI's PowerShell need the call operator, and
    /// Claude's Git Bash must not get it — without this Codex silently ran no
    /// hooks at all.
    #[cfg(windows)]
    #[test]
    fn only_the_powershell_tools_get_the_call_operator() {
        let exe = Path::new(r"C:\Program Files\Termo\termo.exe");
        assert_eq!(
            hook_command(exe, AgentTool::Codex, "Stop"),
            "& \"C:/Program Files/Termo/termo.exe\" --agent-hook codex Stop"
        );
        assert_eq!(
            hook_command(exe, AgentTool::Gemini, "AfterAgent"),
            "& \"C:/Program Files/Termo/termo.exe\" --agent-hook gemini AfterAgent"
        );
        assert!(!hook_command(exe, AgentTool::Claude, "Stop").starts_with('&'));
    }

    #[cfg(not(windows))]
    #[test]
    fn command_quotes_a_unix_path_for_sh() {
        assert_eq!(
            hook_command(Path::new("/opt/Ter'mini/termo"), AgentTool::Codex, "Stop"),
            r"'/opt/Ter'\''mini/termo' --agent-hook codex Stop"
        );
    }

    #[test]
    fn hook_events_become_status_updates() {
        let dir = temp_dir();
        append_event(
            &dir,
            "s1",
            "claude",
            "Notification",
            "{\n \"notification_type\": \"permission_prompt\",\n \"message\": \"x\"\n}",
        )
        .unwrap();
        append_event(&dir, "s1", "claude", "Stop", r#"{"last_assistant_message":"done"}"#).unwrap();

        let mut offset = 0;
        let updates = read_new_lines(&dir.join("s1.jsonl"), &mut offset);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            updates.iter().map(|u| u.status).collect::<Vec<_>>(),
            [Some(AgentStatus::Awaiting), Some(AgentStatus::Finished)]
        );
    }

    #[test]
    fn ignores_session_ids_that_could_escape_the_events_dir() {
        let base = temp_dir();
        let dir = base.join("events");
        fs::create_dir_all(&dir).unwrap();
        append_event(&dir, "../escaped", "claude", "Stop", "{}").unwrap();
        let escaped = base.join("escaped.jsonl").exists();
        let _ = fs::remove_dir_all(&base);
        assert!(!escaped);
    }
}
