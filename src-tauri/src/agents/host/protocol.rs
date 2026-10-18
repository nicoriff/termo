//! What the app and the agents' process say to each other: JSON messages in
//! the sessions daemon's frames (see mux/protocol.rs).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bumped on any change the other side wouldn't understand. An app that
/// meets a process speaking another one asks it to leave once it's idle,
/// and starts its own (see client.rs).
pub const PROTOCOL: u32 = 1;

/// From the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToHost {
    /// The first message on a connection: `token` is the one in the info
    /// file, which only this user can read. An `observer` (the sessions
    /// daemon, for the web client: see mux/agents.rs) isn't an app: with
    /// only observers there, this process still notifies the user itself.
    Hello {
        protocol: u32,
        token: String,
        build: String,
        #[serde(default)]
        observer: bool,
    },
    /// The language the app's windows speak ("es" or "en"), for what this
    /// process says itself (a notification, an error).
    Language { language: String },
    /// One of ops::call's, by name.
    Call { id: u64, name: String, args: Value },
    /// The config changed: read it again now rather than on the next look.
    ConfigChanged,
    /// Leave once nothing is running (an app of another build is here, or
    /// the setting was turned off).
    Retire,
}

/// From the agents' process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FromHost {
    Welcome { protocol: u32, build: String, pid: u32 },
    Reply { id: u64, result: Result<Value, String> },
    /// What the agents said, for the app's windows: the same events they
    /// hear when the agents run in the app.
    Event { name: String, payload: Value },
    /// How it's doing, whenever that changes and on connecting.
    Status(HostStatus),
}

/// What the app shows of it (Preferences, the quit prompt).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    /// Whether the webhook and MCP server took their port.
    pub listening: bool,
    pub running_runs: usize,
    pub running_orchestrations: usize,
    /// Agents with a schedule, a folder, a git hook or a webhook, on.
    pub scheduled: usize,
    pub watching: usize,
    pub on_git: usize,
    pub on_webhook: usize,
    /// It was asked to leave and will once nothing runs.
    pub retiring: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mux::protocol::{read_frame, write_frame, Frame};

    #[test]
    fn messages_go_through_the_daemons_frames() {
        let sent = ToHost::Call { id: 7, name: "agent_stop".into(), args: serde_json::json!({ "agent_id": "a1" }) };
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &Frame::Message(sent.clone())).unwrap();
        let got: Option<Frame<ToHost>> = read_frame(&mut bytes.as_slice()).unwrap();
        assert_eq!(got, Some(Frame::Message(sent)));

        let reply = FromHost::Reply { id: 7, result: Err("busy".into()) };
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &Frame::Message(reply.clone())).unwrap();
        let got: Option<Frame<FromHost>> = read_frame(&mut bytes.as_slice()).unwrap();
        assert_eq!(got, Some(Frame::Message(reply)));
    }
}
