//! Translates Claude Code / Codex lifecycle hook events into Modo IA
//! session statuses. Both tools run `termo --agent-hook` (see
//! agent_hooks.rs) on each event, which appends one line per event to
//! `<TERMO_EVENTS_DIR>/<TERMO_SESSION_ID>.jsonl`:
//!
//! `<tool>\t<event>\t<detail>\t<agent session id>\t<cwd>`
//!
//! where `detail` is Claude's `notification_type` for Notification events
//! and empty otherwise, and the last two come from the hook payload's common
//! fields — they're what lets Modo IA list the session later and resume it
//! (see agent_sessions.rs). Everything else in the payload is dropped by the
//! hook, so large payloads (tool results) never touch the file. Lines written
//! by older builds have only the first three fields and still parse.

use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Running,
    Awaiting,
    Error,
    Finished,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AgentUpdate {
    pub tool: String,
    /// `None` means the agent session ended and the tab's status clears.
    pub status: Option<AgentStatus>,
    /// The agent's own session id, when the hook reported one. Named apart
    /// from the PTY id the event payload already carries as `sessionId`.
    #[serde(rename = "agentSessionId")]
    pub agent_session_id: Option<String>,
    #[serde(rename = "agentCwd")]
    pub agent_cwd: Option<String>,
}

/// Notification types that mean the agent is blocked on the user: Claude
/// Code's, and Gemini CLI's one (a tool waiting for approval).
const NEEDS_USER: &[&str] = &[
    "permission_prompt",
    "agent_needs_input",
    "elicitation_dialog",
    "elicitation_url_dialog",
    "ToolPermission",
];

pub fn parse_line(line: &str) -> Option<AgentUpdate> {
    let mut parts = line.trim_end_matches(['\r', '\n']).splitn(5, '\t');
    let tool = parts.next()?;
    let event = parts.next()?;
    let detail = parts.next().unwrap_or("");
    let optional = |value: Option<&str>| value.filter(|text| !text.is_empty()).map(str::to_owned);
    let agent_session_id = optional(parts.next());
    let agent_cwd = optional(parts.next());
    if !matches!(tool, "claude" | "codex" | "gemini" | "copilot" | "grok") {
        return None;
    }
    let status = match event {
        // Gemini CLI: AfterAgent ends a turn as Stop does.
        "SessionStart" | "Interrupt" | "Stop" | "AfterAgent" => Some(AgentStatus::Finished),
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => Some(AgentStatus::Running),
        "BeforeAgent" | "BeforeTool" | "AfterTool" => Some(AgentStatus::Running),
        "PermissionRequest" => Some(AgentStatus::Awaiting),
        "Notification" if NEEDS_USER.contains(&detail) => Some(AgentStatus::Awaiting),
        "StopFailure" => Some(AgentStatus::Error),
        // Copilot CLI's camelCase names.
        "sessionStart" | "agentStop" => Some(AgentStatus::Finished),
        "userPromptSubmitted" | "preToolUse" | "postToolUse" | "postToolUseFailure" => Some(AgentStatus::Running),
        "permissionRequest" => Some(AgentStatus::Awaiting),
        "errorOccurred" => Some(AgentStatus::Error),
        "SessionEnd" | "sessionEnd" => None,
        _ => return None,
    };
    Some(AgentUpdate {
        tool: tool.to_string(),
        status,
        agent_session_id,
        agent_cwd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(line: &str) -> Option<Option<AgentStatus>> {
        parse_line(line).map(|update| update.status)
    }

    #[test]
    fn maps_the_turn_lifecycle() {
        assert_eq!(status("claude\tUserPromptSubmit\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("claude\tPermissionRequest\t"), Some(Some(AgentStatus::Awaiting)));
        assert_eq!(status("claude\tPostToolUse\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("claude\tStop\t"), Some(Some(AgentStatus::Finished)));
        assert_eq!(status("claude\tStopFailure\t"), Some(Some(AgentStatus::Error)));
        assert_eq!(status("codex\tInterrupt\t"), Some(Some(AgentStatus::Finished)));
    }

    #[test]
    fn maps_gemini_clis_turn_lifecycle() {
        assert_eq!(status("gemini\tSessionStart\t"), Some(Some(AgentStatus::Finished)));
        assert_eq!(status("gemini\tBeforeAgent\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("gemini\tBeforeTool\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("gemini\tNotification\tToolPermission"), Some(Some(AgentStatus::Awaiting)));
        assert_eq!(status("gemini\tAfterTool\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("gemini\tAfterAgent\t"), Some(Some(AgentStatus::Finished)));
        assert_eq!(status("gemini\tSessionEnd\t"), Some(None));
        let update = parse_line("gemini\tBeforeAgent\t\tsess-1\tE:/proj").unwrap();
        assert_eq!(update.agent_session_id.as_deref(), Some("sess-1"));
        assert_eq!(update.agent_cwd.as_deref(), Some("E:/proj"));
    }

    #[test]
    fn maps_copilot_clis_turn_lifecycle() {
        assert_eq!(status("copilot\tsessionStart\t"), Some(Some(AgentStatus::Finished)));
        assert_eq!(status("copilot\tuserPromptSubmitted\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("copilot\tpreToolUse\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("copilot\tpermissionRequest\t"), Some(Some(AgentStatus::Awaiting)));
        assert_eq!(status("copilot\tpostToolUseFailure\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("copilot\tagentStop\t"), Some(Some(AgentStatus::Finished)));
        assert_eq!(status("copilot\terrorOccurred\t"), Some(Some(AgentStatus::Error)));
        assert_eq!(status("copilot\tsessionEnd\t"), Some(None));
        assert_eq!(status("copilot\tsomethingElse\t"), None);
    }

    #[test]
    fn only_blocking_notifications_mean_awaiting() {
        assert_eq!(status("claude\tNotification\tpermission_prompt"), Some(Some(AgentStatus::Awaiting)));
        assert_eq!(status("claude\tNotification\tidle_prompt"), None);
        assert_eq!(status("claude\tNotification\t"), None);
    }

    /// Grok takes Claude Code's event names.
    #[test]
    fn maps_groks_turn_lifecycle() {
        assert_eq!(status("grok\tSessionStart\t"), Some(Some(AgentStatus::Finished)));
        assert_eq!(status("grok\tUserPromptSubmit\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("grok\tPreToolUse\t"), Some(Some(AgentStatus::Running)));
        assert_eq!(status("grok\tNotification\tpermission_prompt"), Some(Some(AgentStatus::Awaiting)));
        assert_eq!(status("grok\tStop\t"), Some(Some(AgentStatus::Finished)));
        assert_eq!(status("grok\tStopFailure\t"), Some(Some(AgentStatus::Error)));
        assert_eq!(status("grok\tSessionEnd\t"), Some(None));
    }

    #[test]
    fn reads_the_session_id_and_cwd_when_present() {
        let update = parse_line("claude\tStop\t\tabc-123\tE:/Proyectos/Termini").unwrap();
        assert_eq!(update.agent_session_id.as_deref(), Some("abc-123"));
        assert_eq!(update.agent_cwd.as_deref(), Some("E:/Proyectos/Termini"));
    }

    #[test]
    fn lines_from_older_builds_still_parse() {
        let update = parse_line("claude\tStop\t").unwrap();
        assert_eq!(update.status, Some(AgentStatus::Finished));
        assert_eq!(update.agent_session_id, None);
        assert_eq!(update.agent_cwd, None);
    }

    #[test]
    fn session_end_clears_the_status() {
        let update = parse_line("codex\tSessionEnd\t\n").unwrap();
        assert_eq!(update.tool, "codex");
        assert_eq!(update.status, None);
    }

    #[test]
    fn ignores_unknown_tools_events_and_garbage() {
        assert_eq!(status("vim\tStop\t"), None);
        assert_eq!(status("claude\tPreCompact\t"), None);
        assert_eq!(status("garbage"), None);
        assert_eq!(status(""), None);
    }
}
