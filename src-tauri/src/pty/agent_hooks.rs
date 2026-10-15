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

use super::agent_events::{parse_line, parse_note, AgentUpdate, NOTE};

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
    Grok,
}

impl AgentTool {
    const ALL: [AgentTool; 5] =
        [AgentTool::Claude, AgentTool::Codex, AgentTool::Gemini, AgentTool::Copilot, AgentTool::Grok];

    fn name(self) -> &'static str {
        match self {
            AgentTool::Claude => "claude",
            AgentTool::Codex => "codex",
            AgentTool::Gemini => "gemini",
            AgentTool::Copilot => "copilot",
            AgentTool::Grok => "grok",
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
            // Grok takes Claude Code's names. It has no PermissionRequest:
            // a tool waiting on the user comes as a Notification.
            AgentTool::Grok => &[
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "Notification",
                "PostToolUse",
                "PostToolUseFailure",
                "Stop",
                "StopFailure",
                "SessionEnd",
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
            // The same for Grok, which merges every file in its hooks folder.
            AgentTool::Grok => home.join(".grok").join("hooks").join("termo.json"),
        })
    }

    /// Whether the hooks file is Termo's own, written whole, rather than the
    /// tool's shared settings that Termo's entries are merged into.
    fn owns_file(self) -> bool {
        matches!(self, AgentTool::Copilot | AgentTool::Grok)
    }

    /// What goes in a hooks file that is Termo's own.
    fn own_root(self, exe: &Path) -> Map<String, Value> {
        match self {
            AgentTool::Copilot => copilot_root(exe),
            // Claude Code's shape, which is Grok's too.
            _ => {
                let mut root = Map::new();
                let _ = add_ours(&mut root, self, exe);
                root
            }
        }
    }

    /// Whether a file by the name of Termo's own is in fact Termo's: one the
    /// user made by that name is left alone.
    fn is_own_file(self, root: &Map<String, Value>) -> bool {
        match self {
            AgentTool::Copilot => is_copilot_file_ours(root),
            _ => is_only_ours(root),
        }
    }
}

pub fn events_dir(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join("agent-events"))
}

fn hook_command(exe: &Path, tool: AgentTool, event: &str) -> String {
    let args = format!("{HOOK_FLAG} {} {event}", tool.name());
    match bare_executable(exe) {
        Some(path) => format!("{path} {args}"),
        None => format!("{}{} {args}", call_prefix(tool), quote_executable(exe)),
    }
}

/// Characters that mean the same, unquoted, to bash, sh and PowerShell.
fn is_shell_neutral(path: &str) -> bool {
    !path.is_empty() && path.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '~'))
}

/// The executable's path as a bare word every shell runs alike, so the
/// hook works whichever shell the agent picks: Claude Code takes Git Bash on
/// Windows but PowerShell when it can't find it, and there a quoted path
/// followed by arguments is a syntax error — the hook fails, and Claude Code
/// took that as refusing every tool. A path with spaces (Program Files)
/// goes by its 8.3 short name. `None` when there's no such word; the quoted
/// path, for the shell each tool usually runs, is left.
#[cfg(windows)]
fn bare_executable(exe: &Path) -> Option<String> {
    let path = exe.to_string_lossy().replace('\\', "/");
    if is_shell_neutral(&path) {
        return Some(path);
    }
    let short = short_path(exe)?.to_string_lossy().replace('\\', "/");
    is_shell_neutral(&short).then_some(short)
}

#[cfg(not(windows))]
fn bare_executable(exe: &Path) -> Option<String> {
    let path = exe.to_string_lossy();
    is_shell_neutral(&path).then(|| path.into_owned())
}

/// The 8.3 name Windows keeps for a path with long names; `None` where the
/// volume keeps none.
#[cfg(windows)]
fn short_path(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: `wide` is NUL-terminated; a null buffer asks for the length.
    let needed = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut buffer = vec![0u16; needed as usize];
    // SAFETY: `buffer` holds `needed` characters, the terminator included.
    let written = unsafe { GetShortPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), needed) };
    if written == 0 || written >= needed {
        return None;
    }
    buffer.truncate(written as usize);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

/// Codex, Gemini CLI and Grok run their hooks through PowerShell on Windows, and there a statement
/// that opens with a quoted path is an expression, not a command: the
/// arguments after it are a syntax error ("Unexpected token"), so the hook
/// never ran and Modo IA never heard from Codex. The call operator makes it
/// a command again. Claude Code runs its hooks through Git Bash instead,
/// where a leading `&` would put the command in the background.
#[cfg(windows)]
fn call_prefix(tool: AgentTool) -> &'static str {
    match tool {
        // Grok too: Claude Code's entries, which it also runs (it reads
        // ~/.claude), fail there with that very error.
        AgentTool::Codex | AgentTool::Gemini | AgentTool::Copilot | AgentTool::Grok => "& ",
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

/// Whether every hook in a file is Termo's, and there is at least one.
fn is_only_ours(root: &Map<String, Value>) -> bool {
    let groups: Vec<&Value> = root
        .get("hooks")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|hooks| hooks.values())
        .filter_map(Value::as_array)
        .flatten()
        .collect();
    !groups.is_empty() && groups.iter().all(|group| is_our_group(group))
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
        return write_root(&file, &tool.own_root(&exe));
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
        if tool.is_own_file(&root) {
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
            tool.is_own_file(&root)
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
            let updated = tool.own_root(&exe);
            if tool.is_own_file(&root) && updated != root {
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
    let Some(tool) = reporting_tool(tool, std::env::var_os("GROK_HOOK_EVENT").is_some(), || {
        is_installed(AgentTool::Grok)
    }) else {
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
    // Nor is Grok's status line needed: its hooks point at the session's
    // own notes (see grok_usage).
    if tool == AgentTool::Grok.name() {
        if let Ok(payload) = serde_json::from_str::<Value>(&input) {
            super::grok_usage::record(dir, &session, &payload);
        }
    }
    let _ = append_event(dir, &session, tool, event, &input);
}

/// Which agent an event is really from, or None when it's a repeat to drop.
/// Grok runs Claude Code's hooks as well as its own (it reads ~/.claude for
/// compatibility), so Termo's Claude entries fire inside a Grok session with
/// "claude" on their command line: there the session is Grok's. With Termo's
/// own Grok hooks installed too, those already reported it.
fn reporting_tool<'a>(tool: &'a str, under_grok: bool, grok_hooks_installed: impl FnOnce() -> bool) -> Option<&'a str> {
    if !under_grok || tool == AgentTool::Grok.name() {
        return Some(tool);
    }
    (!grok_hooks_installed()).then_some(AgentTool::Grok.name())
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
        // Grok's payloads are camelCase.
        match string_at("notification_type") {
            kind if kind.is_empty() => string_at("notificationType"),
            kind => kind,
        }
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
    let mut lines = String::new();
    // What the notification says, on a line of its own before the event's
    // (see agent_events.rs). Both in one write: the watcher reads them together.
    if let Some(note) = payload.as_ref().and_then(|payload| note_for(event, payload)) {
        lines.push_str(&format!("{}\t{}\t{}\n", field(tool), NOTE, note));
    }
    lines.push_str(&format!(
        "{}\t{}\t{}\t{}\t{}\n",
        field(tool),
        field(event),
        field(&detail),
        field(&agent_session_id),
        field(&cwd)
    ));
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{session}.jsonl")))?
        .write_all(lines.as_bytes())
}

/// How much of a payload's text is kept for the notification.
const MAX_NOTE: usize = 200;

/// What an event has to tell the user, in a line: the tool a permission is
/// asked for and on what, the agent's own notice, the last thing it said, why
/// it failed. None for the events that only mark the turn going on, and when
/// the agent's payload doesn't say (each names its fields its own way; the
/// ones known are tried in turn).
fn note_for(event: &str, payload: &Value) -> Option<String> {
    let text = |keys: &[&str]| keys.iter().find_map(|key| payload.get(*key).and_then(Value::as_str)).map(str::to_owned);
    let note = match event {
        "PermissionRequest" | "permissionRequest" => {
            let tool = text(&["tool_name", "toolName"])?;
            let input = payload.get("tool_input").or_else(|| payload.get("toolInput"));
            let about = ["command", "file_path", "path", "url", "pattern", "query", "description"]
                .iter()
                .find_map(|key| input?.get(*key)?.as_str());
            match about {
                Some(about) => format!("{tool}: {about}"),
                None => tool,
            }
        }
        "Notification" => text(&["message"])?,
        "Stop" | "AfterAgent" | "agentStop" => text(&["last_assistant_message", "prompt_response"])?,
        "StopFailure" | "errorOccurred" => text(&["error", "error_message", "message", "reason"])
            .or_else(|| payload.get("error")?.get("message")?.as_str().map(str::to_owned))?,
        _ => return None,
    };
    // One line of plain text: the agents answer in Markdown.
    let plain = note.replace("**", "").replace('`', "");
    let line = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = line.chars().take(MAX_NOTE).collect();
    (!cut.is_empty()).then_some(cut)
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
    // Nor a note without the event it's about: it waits for it.
    let last = chunk[..end].rfind('\n').map_or(0, |at| at + 1);
    let consumed = if parse_note(&chunk[last..end]).is_some() { last } else { end + 1 };
    *offset += consumed as u64;
    let mut note = None;
    let mut updates = Vec::new();
    for line in chunk[..consumed].lines() {
        if let Some(text) = parse_note(line) {
            note = Some(text);
            continue;
        }
        let told = note.take();
        if let Some(update) = parse_line(line) {
            updates.push(AgentUpdate { note: told, ..update });
        }
    }
    updates
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
    fn grok_hook_file_is_termos_own_in_claude_codes_shape() {
        let root = AgentTool::Grok.own_root(exe());
        for event in AgentTool::Grok.events() {
            let command = root["hooks"][*event][0]["hooks"][0]["command"].as_str().unwrap();
            assert!(command.ends_with(&format!("--agent-hook grok {event}")), "{command}");
        }
        assert!(AgentTool::Grok.is_own_file(&root));
        assert!(has_ours(&root, AgentTool::Grok));
    }

    #[test]
    fn a_grok_hook_file_with_the_users_hooks_is_not_ours() {
        let mut mixed = AgentTool::Grok.own_root(exe());
        mixed["hooks"]["Stop"].as_array_mut().unwrap().push(json!({ "hooks": [{ "type": "command", "command": "say done" }] }));
        assert!(!AgentTool::Grok.is_own_file(&mixed));
        assert!(!AgentTool::Grok.is_own_file(&Map::new()));
    }

    #[test]
    fn claude_codes_hooks_running_under_grok_report_grok_once() {
        // Not under Grok: whatever the command line says.
        assert_eq!(reporting_tool("claude", false, || panic!("not asked")), Some("claude"));
        // Grok's own hook, under Grok.
        assert_eq!(reporting_tool("grok", true, || panic!("not asked")), Some("grok"));
        // Claude Code's hook run by Grok: the session is Grok's...
        assert_eq!(reporting_tool("claude", true, || false), Some("grok"));
        // ...and already reported when Grok's own hooks are installed.
        assert_eq!(reporting_tool("claude", true, || true), None);
    }

    #[test]
    fn grok_payloads_are_camel_case_too() {
        let dir = temp_dir();
        let payload = r#"{"sessionId":"abc-1","cwd":"E:/proj","notificationType":"permission_prompt"}"#;
        append_event(&dir, "s1", "grok", "Notification", payload).unwrap();
        let mut offset = 0;
        let updates = read_new_lines(&dir.join("s1.jsonl"), &mut offset);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].tool, "grok");
        assert_eq!(updates[0].status, Some(AgentStatus::Awaiting));
        assert_eq!(updates[0].agent_session_id.as_deref(), Some("abc-1"));
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

    #[test]
    fn a_plain_path_is_left_bare_for_any_shell() {
        assert!(is_shell_neutral("C:/Users/me/AppData/Local/Termo/termo.exe"));
        assert!(is_shell_neutral("C:/PROGRA~1/Termo/termo.exe"));
        assert!(!is_shell_neutral("C:/Program Files/Termo/termo.exe"));
        assert!(!is_shell_neutral("C:/me's/termo.exe"));
        assert!(!is_shell_neutral("C:/a$b/termo.exe"));
        assert!(!is_shell_neutral("C:/a(1)/termo.exe"));
    }

    /// The same command for every tool, with neither quotes nor `&`: Claude
    /// Code runs it in Git Bash or, without one, in PowerShell.
    #[cfg(windows)]
    #[test]
    fn a_path_without_spaces_needs_no_quotes_nor_call_operator() {
        let exe = Path::new(r"C:\Users\me\AppData\Local\Termo\termo.exe");
        assert_eq!(
            hook_command(exe, AgentTool::Claude, "Stop"),
            "C:/Users/me/AppData/Local/Termo/termo.exe --agent-hook claude Stop"
        );
        assert_eq!(
            hook_command(exe, AgentTool::Codex, "Stop"),
            "C:/Users/me/AppData/Local/Termo/termo.exe --agent-hook codex Stop"
        );
    }

    /// A path with spaces that exists goes by its short name, where the
    /// volume keeps them.
    #[cfg(windows)]
    #[test]
    fn a_path_with_spaces_goes_by_its_short_name() {
        let base = temp_dir();
        let dir = base.join("Long Folder Name");
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("termo.exe");
        fs::write(&exe, "").unwrap();
        let command = hook_command(&exe, AgentTool::Claude, "Stop");
        let short = short_path(&exe);
        let _ = fs::remove_dir_all(&base);
        if short.is_some_and(|short| is_shell_neutral(&short.to_string_lossy().replace('\\', "/"))) {
            assert!(!command.starts_with('"') && !command.contains("Long Folder Name"), "{command}");
        } else {
            // No 8.3 names on this volume: the quoted path, as before.
            assert!(command.starts_with('"'), "{command}");
        }
        // The system drive usually keeps them.
        let program_files = Path::new(r"C:\Program Files");
        if let Some(short) = program_files.exists().then(|| short_path(program_files)).flatten() {
            let short = short.to_string_lossy().replace('\\', "/");
            assert!(short == "C:/Program Files" || is_shell_neutral(&short), "{short}");
        }
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
    fn an_event_carries_what_it_has_to_tell_the_user() {
        let dir = temp_dir();
        let asks = r#"{"tool_name":"Bash","tool_input":{"command":"npm run build","description":"Build"}}"#;
        append_event(&dir, "s1", "claude", "PermissionRequest", asks).unwrap();
        append_event(&dir, "s1", "claude", "PostToolUse", r#"{"tool_name":"Bash"}"#).unwrap();
        let said = r#"{"last_assistant_message":"**Done**:\n\nthe `build` passes."}"#;
        append_event(&dir, "s1", "claude", "Stop", said).unwrap();
        // A note whose event isn't written yet waits for it.
        fs::OpenOptions::new()
            .append(true)
            .open(dir.join("s1.jsonl"))
            .unwrap()
            .write_all(b"claude\tNote\tlate\n")
            .unwrap();

        let mut offset = 0;
        let updates = read_new_lines(&dir.join("s1.jsonl"), &mut offset);
        assert_eq!(
            updates.iter().map(|u| u.note.as_deref()).collect::<Vec<_>>(),
            [Some("Bash: npm run build"), None, Some("Done: the build passes.")]
        );
        assert!(read_new_lines(&dir.join("s1.jsonl"), &mut offset).is_empty());
        append_event(&dir, "s1", "claude", "StopFailure", "{}").unwrap();
        let updates = read_new_lines(&dir.join("s1.jsonl"), &mut offset);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].note.as_deref(), Some("late"));
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
