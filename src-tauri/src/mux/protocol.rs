//! What the app and the sessions daemon say to each other (see mux/mod.rs).
//!
//! A connection is a stream of frames: a little-endian u32 with the length
//! of what follows, one byte saying what kind of frame it is, and its body.
//! Control messages are JSON; a console's output and input travel as raw
//! bytes, since that is nearly all the traffic.

use std::io::{self, Read, Write};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::commands::folder_commands::{FolderEntry, FolderRoots};
use crate::pty::SpawnSpec;

/// Bumped on any change a daemon of the previous version wouldn't
/// understand. Each version runs its own daemon (see mux::paths), so an app
/// that speaks a newer one never meets an older daemon.
pub const PROTOCOL: u32 = 1;

/// A frame larger than this is a broken peer, not a message.
const MAX_FRAME: usize = 16 * 1024 * 1024;

const KIND_MESSAGE: u8 = 0;
const KIND_OUTPUT: u8 = 1;
const KIND_INPUT: u8 = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum Frame<M> {
    Message(M),
    /// What a console printed, numbered: `seq` grows by one per frame of a
    /// session, so a client can tell whether it has seen it.
    Output { session: Uuid, seq: u64, bytes: Vec<u8> },
    /// What to type into a console.
    Input { session: Uuid, bytes: Vec<u8> },
}

/// From the app to the daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// The first message on a connection. `token` is the one in the
    /// daemon's info file, which only this user can read.
    Hello { protocol: u32, token: String, version: String },
    Request { id: u64, op: Op },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Starts a console and attaches this connection to it. The id is the
    /// app's to choose, so it can be listening before the first output.
    Spawn { session: Uuid, spec: SpawnSpec },
    /// Sends what the console printed after output number `since` (0: all
    /// it still has), then everything new.
    Attach { session: Uuid, since: u64 },
    Detach { session: Uuid },
    Resize { session: Uuid, cols: u16, rows: u16 },
    /// Its size back to this one, if it has another now (someone else
    /// sized it meanwhile): what a tab does when it's taken up again.
    /// Answered with whether it changed.
    Claim { session: Uuid, cols: u16, rows: u16 },
    PromptRedraw { session: Uuid },
    Kill { session: Uuid },
    List,
    /// Ends the daemon and every console in it.
    Shutdown,
    /// Who is connected from the tailnet now (see remote.rs).
    Viewers,
    /// Ends every connection from the tailnet.
    DisconnectRemote,
    /// The remote access settings changed: applied now rather than at the
    /// next look.
    ReloadRemote,
    /// One of the app's own consoles, shown from the tailnet too while
    /// remote access is on (see pty/sharing.rs): the app sends what it
    /// prints as output frames, and gets what's typed into it from there.
    Host { session: Uuid, console: HostedConsole },
    /// It ended, or isn't shared any more.
    Unhost { session: Uuid },
    /// What runs in front in one of the app's own consoles now.
    Foreground { session: Uuid, path: Option<String> },
    /// What the app knows of the agent in a console — its hooks and status
    /// line are the app's —, for whoever shows it from the tailnet.
    Agent { session: Uuid, agent: Option<AgentInfo> },
    /// What can be opened from the tailnet (see Op::Open): none while
    /// Termo isn't open here.
    OpenChoices,
    /// A new console, opened by the app as a tab of its own — the shell or
    /// agent `choice` (one of OpenChoices'), in `cwd` or wherever it opens
    /// by default. Answered with it once the daemon has it to show.
    Open { choice: String, cwd: Option<String> },
    /// From the app: this connection is the one that opens what the
    /// tailnet asks for (see Open), and this is what it offers — and the
    /// folders its agents ran in lately, to offer them first.
    Opener {
        choices: Vec<OpenChoice>,
        #[serde(default)]
        recent: Vec<String>,
    },
    /// From the app: what came of an Open it was asked (see
    /// ServerMessage::Open): the console's id, or why not.
    Opened { request: u64, result: Result<Uuid, String> },
    /// Where a folder browser starts: home and the drives (see
    /// commands/folder_commands.rs), to pick where a new console opens.
    FolderRoots,
    /// The folders inside `path`.
    ListFolders { path: String },
    /// A new folder `name` inside `parent`.
    CreateFolder { parent: String, name: String },
}

/// A shell or an agent the app offers to open from the tailnet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenChoice {
    /// The app's own name for it, sent back to open it.
    pub id: String,
    /// As its new-tab menu shows it: "PowerShell", "Claude Code"…
    pub name: String,
    /// "shell" or "agent".
    pub kind: String,
}

/// An agent at work in a console, as the app's status bar shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    /// Its name as shown: "Claude Code", "Codex"…
    pub tool: String,
    /// "running", "awaiting" (the user's answer), "error" or "finished".
    pub status: String,
    #[serde(default)]
    pub plan_windows: Vec<PlanUse>,
    pub context_percent: Option<f64>,
    pub cost_usd: Option<f64>,
}

/// How much of one of the plan's rate-limit windows is used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanUse {
    /// How long the window runs; None when the agent doesn't say.
    pub window_minutes: Option<u32>,
    pub percent: f64,
}

/// The repository a console is in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    /// Its branch, or the commit when none is checked out.
    pub branch: String,
    /// Whether anything is uncommitted; None when that isn't asked (see
    /// git::brings_filters).
    pub dirty: Option<bool>,
}

/// What the daemon is told of a console the app runs itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostedConsole {
    pub path: String,
    pub cwd: Option<String>,
    pub started_at: i64,
    pub cols: u16,
    pub rows: u16,
    pub pid: Option<u32>,
}

/// From the daemon to the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        protocol: u32,
        version: String,
        pid: u32,
        /// For a connection from the tailnet, what it may do.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access: Option<super::remote::Access>,
    },
    Rejected { reason: String },
    Reply { id: u64, result: Result<Reply, String> },
    Exit { session: Uuid, code: i32 },
    Foreground { session: Uuid, path: Option<String> },
    /// The console is gone: killed, or exited with nobody attached.
    Gone { session: Uuid },
    /// To the app, of a console it hosts: someone on the tailnet sized it
    /// to their screen, or gave it its size back.
    HostResize { session: Uuid, cols: u16, rows: u16 },
    /// The same: its prompt to draw again, at the new size.
    HostPromptRedraw { session: Uuid },
    /// Something the status bar shows of it changed: its folder, what runs
    /// in front, its agent.
    Info { info: SessionInfo },
    /// To the app (see Op::Opener): someone on the tailnet asks for a new
    /// console. Answered with Op::Opened and the same `request`.
    Open { request: u64, choice: String, cwd: Option<String> },
    /// To the app (see Op::Opener): someone on the tailnet closed one of
    /// its consoles — its tab closes, as if closed here.
    Close { session: Uuid },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reply {
    Done,
    Spawned { pid: Option<u32> },
    Attached { info: SessionInfo },
    Sessions { sessions: Vec<SessionInfo> },
    Viewers { viewers: Vec<super::remote::Viewer> },
    Claimed { changed: bool },
    OpenChoices {
        choices: Vec<OpenChoice>,
        #[serde(default)]
        recent: Vec<String>,
    },
    FolderRoots { roots: FolderRoots },
    Folders { folders: Vec<FolderEntry> },
    Folder { folder: FolderEntry },
    Opened { session: Uuid },
}

/// A console as the daemon knows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: Uuid,
    pub pid: Option<u32>,
    /// What it runs: the shell or agent CLI.
    pub path: String,
    /// Where it is now, as its shell last said (OSC 7); else where it
    /// started.
    pub cwd: Option<String>,
    /// Unix seconds.
    pub started_at: i64,
    pub cols: u16,
    pub rows: u16,
    /// The last title the console set (OSC 0/2), if any.
    pub title: Option<String>,
    /// How many connections are showing it now.
    pub attached: u32,
    /// Its exit code, once its process has ended.
    pub exit_code: Option<i32>,
    /// One the app runs itself and shares while it's open (see Op::Host).
    #[serde(default)]
    pub hosted: bool,
    /// What runs in front now: the path of its program.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentInfo>,
}

pub fn write_frame<M: Serialize>(out: &mut impl Write, frame: &Frame<M>) -> io::Result<()> {
    let mut body = Vec::new();
    match frame {
        Frame::Message(message) => {
            body.push(KIND_MESSAGE);
            serde_json::to_writer(&mut body, message)?;
        }
        Frame::Output { session, seq, bytes } => {
            body.push(KIND_OUTPUT);
            body.extend_from_slice(session.as_bytes());
            body.extend_from_slice(&seq.to_le_bytes());
            body.extend_from_slice(bytes);
        }
        Frame::Input { session, bytes } => {
            body.push(KIND_INPUT);
            body.extend_from_slice(session.as_bytes());
            body.extend_from_slice(bytes);
        }
    }
    let len = u32::try_from(body.len()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too large"))?;
    out.write_all(&len.to_le_bytes())?;
    out.write_all(&body)?;
    out.flush()
}

/// The next frame, or None at a clean end of the stream.
pub fn read_frame<M: for<'de> Deserialize<'de>>(input: &mut impl Read) -> io::Result<Option<Frame<M>>> {
    read_frame_within(input, MAX_FRAME)
}

/// The same, refusing one longer than `max`: what a connection says before
/// it's known who it is (its hello) is small.
pub fn read_frame_within<M: for<'de> Deserialize<'de>>(input: &mut impl Read, max: usize) -> io::Result<Option<Frame<M>>> {
    let mut len = [0u8; 4];
    match input.read_exact(&mut len) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    }
    let len = u32::from_le_bytes(len) as usize;
    if len == 0 || len > max {
        return Err(invalid("bad frame length"));
    }
    // Taken in as it arrives, not set aside up front: a length says nothing
    // of what will follow, and a connection that says a big one and then
    // sends a byte now and then mustn't hold that much memory.
    let mut body = Vec::new();
    Read::take(&mut *input, len as u64).read_to_end(&mut body)?;
    if body.len() < len {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "frame cut short"));
    }
    let rest = &body[1..];
    let frame = match body[0] {
        KIND_MESSAGE => Frame::Message(serde_json::from_slice(rest).map_err(|err| invalid(&err.to_string()))?),
        KIND_OUTPUT => {
            if rest.len() < 24 {
                return Err(invalid("short output frame"));
            }
            let session = Uuid::from_slice(&rest[..16]).map_err(|_| invalid("bad session id"))?;
            let seq = u64::from_le_bytes(rest[16..24].try_into().unwrap());
            Frame::Output { session, seq, bytes: rest[24..].to_vec() }
        }
        KIND_INPUT => {
            if rest.len() < 16 {
                return Err(invalid("short input frame"));
            }
            let session = Uuid::from_slice(&rest[..16]).map_err(|_| invalid("bad session id"))?;
            Frame::Input { session, bytes: rest[16..].to_vec() }
        }
        _ => return Err(invalid("unknown frame kind")),
    };
    Ok(Some(frame))
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<M>(frame: Frame<M>) -> Frame<M>
    where
        M: Serialize + for<'de> Deserialize<'de>,
    {
        let mut buf = Vec::new();
        write_frame(&mut buf, &frame).unwrap();
        let mut reader = buf.as_slice();
        let back = read_frame(&mut reader).unwrap().unwrap();
        assert!(reader.is_empty(), "the whole frame is read, nothing more");
        back
    }

    #[test]
    fn messages_survive_the_trip() {
        let spec = SpawnSpec {
            path: "pwsh.exe".into(),
            args: vec!["-NoLogo".into()],
            env: vec![("TERMO_EVENTS_DIR".into(), "C:\\x".into())],
            cwd: Some("C:\\Users".into()),
            cols: 120,
            rows: 30,
            prompt_redraw: Some(vec![0x1b, b'[']),
            extra_path: vec!["D:\\bin".into()],
        };
        let hello = ClientMessage::Request { id: 7, op: Op::Spawn { session: Uuid::new_v4(), spec } };
        assert_eq!(round_trip(Frame::Message(hello.clone())), Frame::Message(hello));

        let reply = ServerMessage::Reply { id: 7, result: Err("nope".into()) };
        assert_eq!(round_trip(Frame::Message(reply.clone())), Frame::Message(reply));
    }

    #[test]
    fn output_and_input_keep_their_bytes() {
        let session = Uuid::new_v4();
        let output: Frame<ServerMessage> = Frame::Output { session, seq: 42, bytes: b"\x1b[6n\xff\x00".to_vec() };
        assert_eq!(round_trip(output.clone()), output);
        let input: Frame<ClientMessage> = Frame::Input { session, bytes: Vec::new() };
        assert_eq!(round_trip(input.clone()), input);
    }

    #[test]
    fn a_closed_stream_ends_cleanly_and_garbage_is_an_error() {
        let mut empty: &[u8] = &[];
        assert!(read_frame::<ServerMessage>(&mut empty).unwrap().is_none());
        let mut huge: &[u8] = &[0xff, 0xff, 0xff, 0xff, 0];
        assert!(read_frame::<ServerMessage>(&mut huge).is_err());
        let mut unknown: &[u8] = &[1, 0, 0, 0, 9];
        assert!(read_frame::<ServerMessage>(&mut unknown).is_err());
    }

    #[test]
    fn a_frame_is_no_longer_than_allowed_nor_than_what_came() {
        let mut buf = Vec::new();
        write_frame(&mut buf, &Frame::<ClientMessage>::Input { session: Uuid::new_v4(), bytes: vec![b'x'; 100] }).unwrap();
        assert!(read_frame_within::<ClientMessage>(&mut buf.as_slice(), 64).is_err(), "longer than allowed");
        // Says 1 MB, sends three bytes and stops.
        let mut short: &[u8] = &[0, 0, 0x10, 0, 2, 0, 0];
        let err = read_frame::<ClientMessage>(&mut short).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }
}
