//! Zellij, when the user has it: its sessions, to open one in a tab (the
//! tab runs `zellij attach <name>`, see TileChrome's new-tab menu). Another
//! way to have consoles that outlive Termo — Zellij's own server keeps
//! them —, on any of the three systems.

use serde::Serialize;

use crate::pty::bin_search;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZellijSessions {
    /// Where its executable is; None when it isn't installed.
    pub path: Option<String>,
    pub sessions: Vec<ZellijSession>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZellijSession {
    pub name: String,
    /// Ended, but Zellij can bring it back (resurrect) on attach.
    pub exited: bool,
}

#[tauri::command]
pub async fn zellij_sessions() -> ZellijSessions {
    tauri::async_runtime::spawn_blocking(|| {
        let Some(path) = bin_search::find_binary("zellij") else {
            return ZellijSessions { path: None, sessions: Vec::new() };
        };
        let mut command = std::process::Command::new(&path);
        command.args(["list-sessions", "--no-formatting"]).stdin(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        // "No active sessions" is an error exit: no sessions, all the same.
        let sessions = command.output().map(|output| parse(&String::from_utf8_lossy(&output.stdout))).unwrap_or_default();
        ZellijSessions { path: Some(path.to_string_lossy().into_owned()), sessions }
    })
    .await
    .unwrap_or(ZellijSessions { path: None, sessions: Vec::new() })
}

/// `zellij list-sessions --no-formatting`: one session per line, its name
/// first — `work [Created 2h ago]`, `old [Created 1day ago] (EXITED - attach
/// to resurrect)`.
fn parse(listing: &str) -> Vec<ZellijSession> {
    listing
        .lines()
        .filter_map(|line| {
            let name = line.split_whitespace().next()?;
            Some(ZellijSession { name: name.to_string(), exited: line.contains("EXITED") })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_sessions_and_which_ended() {
        let listing = "work [Created 2h 3m ago] (current)\nold-one [Created 1day ago] (EXITED - attach to resurrect)\n";
        assert_eq!(
            parse(listing),
            [
                ZellijSession { name: "work".into(), exited: false },
                ZellijSession { name: "old-one".into(), exited: true },
            ]
        );
        assert!(parse("").is_empty());
    }
}
