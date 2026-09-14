//! Translates Claude Code / Codex lifecycle hook events into Modo IA
//! session statuses. Both tools run the Termini hook script (see
//! agent_hooks.rs) on each event; the script appends one line per event to
//! `<TERMINI_EVENTS_DIR>/<TERMINI_SESSION_ID>.jsonl`:
//!
//! `<tool>\t<event>\t<detail>`
//!
//! where `detail` is Claude's `notification_type` for Notification events
//! and empty otherwise. Everything else in the hook payload is dropped in
//! the script, so large payloads (tool results) never touch the file.

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
}

/// Notification types that mean the agent is blocked on the user.
const NEEDS_USER: &[&str] = &[
    "permission_prompt",
    "agent_needs_input",
    "elicitation_dialog",
    "elicitation_url_dialog",
];

pub fn parse_line(line: &str) -> Option<AgentUpdate> {
    let mut parts = line.trim_end_matches(['\r', '\n']).splitn(3, '\t');
    let tool = parts.next()?;
    let event = parts.next()?;
    let detail = parts.next().unwrap_or("");
    if tool != "claude" && tool != "codex" {
        return None;
    }
    let status = match event {
        "SessionStart" | "Interrupt" | "Stop" => Some(AgentStatus::Finished),
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => Some(AgentStatus::Running),
        "PermissionRequest" => Some(AgentStatus::Awaiting),
        "Notification" if NEEDS_USER.contains(&detail) => Some(AgentStatus::Awaiting),
        "StopFailure" => Some(AgentStatus::Error),
        "SessionEnd" => None,
        _ => return None,
    };
    Some(AgentUpdate { tool: tool.to_string(), status })
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
    fn only_blocking_notifications_mean_awaiting() {
        assert_eq!(status("claude\tNotification\tpermission_prompt"), Some(Some(AgentStatus::Awaiting)));
        assert_eq!(status("claude\tNotification\tidle_prompt"), None);
        assert_eq!(status("claude\tNotification\t"), None);
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
