//! The sessions daemon itself: `termo --mux` (see mux/mod.rs).
//!
//! It owns the consoles. Each keeps what it printed lately, so a client that
//! attaches — the app opening again, another window — gets the screen back,
//! and forwards everything new to the clients attached to it. Threads, like
//! the rest of the console code: one per connection to read from it, one to
//! write to it, and the console's own (see PtySession).

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use super::protocol::{read_frame, write_frame, ClientMessage, Frame, Op, Reply, ServerMessage, SessionInfo, PROTOCOL};
use super::DaemonInfo;
use crate::pty::{contains_cursor_report, ConsoleEvent, EventSink, PtySession, SpawnSpec};

/// How much of a console's output it keeps for a client that attaches later.
const HISTORY_BYTES: usize = 2 * 1024 * 1024;
/// Frames waiting to go out to one client. A client this far behind isn't
/// reading: it is dropped rather than letting the daemon's memory grow.
const CLIENT_QUEUE: usize = 4096;
/// How long the daemon stays up with no consoles and no one connected.
const IDLE_EXIT: Duration = Duration::from_secs(5 * 60);
/// What ConPTY asks before it lets the shell start (see PtySession::write),
/// and the answer the daemon gives when no terminal is attached to.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
const CURSOR_HOME_REPORT: &[u8] = b"\x1b[1;1R";
/// Resets the terminal: what a replay that lost its beginning starts with.
const RESET: &[u8] = b"\x1bc";

/// `termo --mux <build> <folder>`: runs the daemon until it has been idle a
/// while.
pub fn run(build: &str, dir: Option<&str>) {
    let Some(dir) = dir.map(PathBuf::from).or_else(super::dir) else {
        return;
    };
    // Another daemon of this protocol already answering: it keeps the job.
    if super::connect(&dir).is_some() {
        return;
    }
    let Ok(listener) = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))) else {
        return;
    };
    let Ok(port) = listener.local_addr().map(|addr| addr.port()) else {
        return;
    };
    let daemon = Daemon::new(super::new_token());
    let info = DaemonInfo {
        pid: std::process::id(),
        port,
        token: daemon.token.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        build: build.to_string(),
    };
    if super::write_info(&dir, &info).is_err() {
        return;
    }
    daemon.set_dir(dir.clone());
    {
        let daemon = Arc::clone(&daemon);
        std::thread::spawn(move || exit_when_idle(&daemon));
    }
    serve(&daemon, listener);
    super::remove_info(&dir, info.pid);
}

/// Accepts connections until the listener fails.
fn serve(daemon: &Arc<Daemon>, listener: TcpListener) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let daemon = Arc::clone(daemon);
        std::thread::spawn(move || daemon.handle_connection(stream));
    }
}

fn exit_when_idle(daemon: &Arc<Daemon>) {
    loop {
        std::thread::sleep(Duration::from_secs(10));
        if daemon.idle_for() >= IDLE_EXIT {
            daemon.exit();
        }
    }
}

fn now_seconds() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs() as i64)
}

pub struct Daemon {
    token: String,
    dir: OnceLock<PathBuf>,
    sessions: Mutex<HashMap<Uuid, Arc<Live>>>,
    connections: AtomicUsize,
    next_client: AtomicU64,
    /// When the daemon last had a console or a connection.
    busy_at: Mutex<Instant>,
}

/// A console the daemon runs.
struct Live {
    id: Uuid,
    path: String,
    cwd: Option<String>,
    started_at: i64,
    pty: OnceLock<PtySession>,
    state: Mutex<State>,
}

struct State {
    /// The connections showing this console now.
    clients: Vec<Client>,
    history: History,
    /// The number of the last output.
    seq: u64,
    cols: u16,
    rows: u16,
    title: Option<String>,
    exit_code: Option<i32>,
    /// ConPTY asked where the cursor is and no terminal has answered yet.
    /// Whoever is attached answers; if the last one leaves first, the
    /// daemon does (see CURSOR_QUERY).
    cursor_query: bool,
}

#[derive(Clone)]
struct Client {
    id: u64,
    tx: SyncSender<Frame<ServerMessage>>,
    /// Ends the connection, for a client that stopped reading.
    stream: Arc<TcpStream>,
}

impl Client {
    /// Queues a frame without waiting; a client whose queue is full is cut
    /// off (its connection ends, and it detaches from everything).
    fn offer(&self, frame: Frame<ServerMessage>) -> bool {
        match self.tx.try_send(frame) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                let _ = self.stream.shutdown(std::net::Shutdown::Both);
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }
}

impl Daemon {
    pub fn new(token: String) -> Arc<Self> {
        Arc::new(Self {
            token,
            dir: OnceLock::new(),
            sessions: Mutex::new(HashMap::new()),
            connections: AtomicUsize::new(0),
            next_client: AtomicU64::new(1),
            busy_at: Mutex::new(Instant::now()),
        })
    }

    fn set_dir(&self, dir: PathBuf) {
        let _ = self.dir.set(dir);
    }

    fn touch(&self) {
        *self.busy_at.lock().unwrap() = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        if !self.sessions.lock().unwrap().is_empty() || self.connections.load(Ordering::SeqCst) > 0 {
            self.touch();
        }
        self.busy_at.lock().unwrap().elapsed()
    }

    /// Ends every console and the daemon.
    fn exit(&self) -> ! {
        let sessions: Vec<Arc<Live>> = self.sessions.lock().unwrap().drain().map(|(_, live)| live).collect();
        for live in &sessions {
            if let Some(pty) = live.pty.get() {
                let _ = pty.kill();
            }
        }
        if let Some(dir) = self.dir.get() {
            super::remove_info(dir, std::process::id());
        }
        std::process::exit(0);
    }

    fn session(&self, id: Uuid) -> Result<Arc<Live>, String> {
        self.sessions.lock().unwrap().get(&id).cloned().ok_or_else(|| format!("unknown session: {id}"))
    }

    /// One connection, from its hello to its end.
    pub fn handle_connection(self: Arc<Self>, stream: TcpStream) {
        let _ = stream.set_nodelay(true);
        let mut reader = stream;
        let _ = reader.set_read_timeout(Some(Duration::from_secs(5)));
        let Ok(mut writer) = reader.try_clone() else {
            return;
        };
        match read_frame::<ClientMessage>(&mut reader) {
            Ok(Some(Frame::Message(ClientMessage::Hello { protocol, token, .. }))) => {
                if !crate::agents::webhook::token_matches(&token, &self.token) {
                    let rejected = ServerMessage::Rejected { reason: "bad token".into() };
                    let _ = write_frame(&mut writer, &Frame::Message(rejected));
                    return;
                }
                if protocol != PROTOCOL {
                    let rejected = ServerMessage::Rejected { reason: format!("protocol {protocol}, not {PROTOCOL}") };
                    let _ = write_frame(&mut writer, &Frame::Message(rejected));
                    return;
                }
            }
            _ => return,
        }
        let welcome = ServerMessage::Welcome {
            protocol: PROTOCOL,
            version: env!("CARGO_PKG_VERSION").to_string(),
            pid: std::process::id(),
        };
        if write_frame(&mut writer, &Frame::Message(welcome)).is_err() {
            return;
        }
        let _ = reader.set_read_timeout(None);

        let Ok(shutdown_handle) = reader.try_clone() else {
            return;
        };
        let (tx, rx) = sync_channel::<Frame<ServerMessage>>(CLIENT_QUEUE);
        let client = Client {
            id: self.next_client.fetch_add(1, Ordering::SeqCst),
            tx,
            stream: Arc::new(shutdown_handle),
        };
        std::thread::spawn(move || {
            for frame in rx {
                if write_frame(&mut writer, &frame).is_err() {
                    break;
                }
            }
            let _ = writer.shutdown(std::net::Shutdown::Both);
        });

        self.connections.fetch_add(1, Ordering::SeqCst);
        self.touch();
        let mut attached: HashSet<Uuid> = HashSet::new();
        while let Ok(Some(frame)) = read_frame::<ClientMessage>(&mut reader) {
            match frame {
                Frame::Input { session, bytes } => {
                    if let Ok(live) = self.session(session) {
                        if contains_cursor_report(&bytes) {
                            live.state.lock().unwrap().cursor_query = false;
                        }
                        if let Some(pty) = live.pty.get() {
                            let _ = pty.write(&bytes);
                        }
                    }
                }
                Frame::Message(ClientMessage::Request { id, op }) => {
                    let shutdown = op == Op::Shutdown;
                    let result = self.handle(&client, &mut attached, op);
                    // Blocking here is fine: the writer thread drains it.
                    if client.tx.send(Frame::Message(ServerMessage::Reply { id, result })).is_err() {
                        break;
                    }
                    if shutdown {
                        // Time for the reply to go out.
                        std::thread::sleep(Duration::from_millis(100));
                        self.exit();
                    }
                }
                _ => {}
            }
        }
        for session in attached {
            self.detach(session, client.id);
        }
        self.connections.fetch_sub(1, Ordering::SeqCst);
        self.touch();
    }

    fn handle(self: &Arc<Self>, client: &Client, attached: &mut HashSet<Uuid>, op: Op) -> Result<Reply, String> {
        match op {
            Op::Spawn { session, spec } => {
                let pid = self.spawn(session, &spec)?;
                self.attach(session, client, 0)?;
                attached.insert(session);
                Ok(Reply::Spawned { pid })
            }
            Op::Attach { session, since } => {
                let info = self.attach(session, client, since)?;
                attached.insert(session);
                Ok(Reply::Attached { info })
            }
            Op::Detach { session } => {
                attached.remove(&session);
                self.detach(session, client.id);
                Ok(Reply::Done)
            }
            Op::Resize { session, cols, rows } => {
                let live = self.session(session)?;
                if let Some(pty) = live.pty.get() {
                    pty.resize(cols, rows).map_err(|err| err.to_string())?;
                }
                let mut state = live.state.lock().unwrap();
                state.cols = cols;
                state.rows = rows;
                Ok(Reply::Done)
            }
            Op::PromptRedraw { session } => {
                // The app has already checked the shell is at its prompt.
                let live = self.session(session)?;
                if let Some(pty) = live.pty.get() {
                    pty.prompt_redraw(|_| true).map_err(|err| err.to_string())?;
                }
                Ok(Reply::Done)
            }
            Op::Kill { session } => {
                attached.remove(&session);
                self.kill(session)?;
                Ok(Reply::Done)
            }
            Op::List => Ok(Reply::Sessions { sessions: self.list() }),
            Op::Shutdown => Ok(Reply::Done),
        }
    }

    fn spawn(self: &Arc<Self>, id: Uuid, spec: &SpawnSpec) -> Result<Option<u32>, String> {
        if self.sessions.lock().unwrap().contains_key(&id) {
            return Err(format!("session {id} already exists"));
        }
        let live = Arc::new(Live {
            id,
            path: spec.path.clone(),
            cwd: spec.cwd.clone(),
            started_at: now_seconds(),
            pty: OnceLock::new(),
            state: Mutex::new(State {
                clients: Vec::new(),
                history: History::default(),
                seq: 0,
                cols: spec.cols,
                rows: spec.rows,
                title: None,
                exit_code: None,
                cursor_query: false,
            }),
        });
        let sink = self.sink_for(&live);
        let pty = PtySession::spawn(id, spec, sink).map_err(|err| err.to_string())?;
        let pid = pty.pid();
        let _ = live.pty.set(pty);
        self.sessions.lock().unwrap().insert(id, live);
        self.touch();
        Ok(pid)
    }

    /// Where a console's events go: into its history, and out to whoever is
    /// attached. Always true: the console keeps reporting with no one
    /// attached, since someone may attach later.
    fn sink_for(self: &Arc<Self>, live: &Arc<Live>) -> EventSink {
        let daemon: Weak<Daemon> = Arc::downgrade(self);
        let live: Weak<Live> = Arc::downgrade(live);
        Arc::new(move |event| {
            let Some(live) = live.upgrade() else {
                return true;
            };
            match event {
                ConsoleEvent::Output(bytes) => live.output(bytes),
                ConsoleEvent::Exit(code) => {
                    let orphan = {
                        let mut state = live.state.lock().unwrap();
                        state.exit_code = Some(code);
                        let exit = ServerMessage::Exit { session: live.id, code };
                        state.clients.retain(|client| client.offer(Frame::Message(exit.clone())));
                        state.clients.is_empty()
                    };
                    // Nobody to show it to: it goes.
                    if orphan {
                        if let Some(daemon) = daemon.upgrade() {
                            daemon.remove(live.id);
                        }
                    }
                }
                ConsoleEvent::Foreground(path) => {
                    let mut state = live.state.lock().unwrap();
                    let message = ServerMessage::Foreground { session: live.id, path };
                    state.clients.retain(|client| client.offer(Frame::Message(message.clone())));
                }
            }
            true
        })
    }

    fn attach(&self, id: Uuid, client: &Client, since: u64) -> Result<SessionInfo, String> {
        let live = self.session(id)?;
        let mut state = live.state.lock().unwrap();
        state.clients.retain(|other| other.id != client.id);
        // Under the same lock as new output: nothing is missed or sent twice.
        for frame in state.history.replay(id, since) {
            if !client.offer(frame) {
                return Err("client is not reading".into());
            }
        }
        state.clients.push(client.clone());
        Ok(live.info(&state))
    }

    fn detach(&self, id: Uuid, client: u64) {
        let Ok(live) = self.session(id) else {
            return;
        };
        let (finished, unanswered) = {
            let mut state = live.state.lock().unwrap();
            state.clients.retain(|other| other.id != client);
            let alone = state.clients.is_empty();
            let unanswered = alone && std::mem::take(&mut state.cursor_query);
            (alone && state.exit_code.is_some(), unanswered)
        };
        if unanswered {
            live.answer_cursor_query();
        }
        // Its process ended while shown; the last one to stop showing it
        // lets it go.
        if finished {
            self.remove(id);
        }
    }

    fn kill(&self, id: Uuid) -> Result<(), String> {
        let live = self.session(id)?;
        {
            let mut state = live.state.lock().unwrap();
            let gone = ServerMessage::Gone { session: id };
            for client in state.clients.drain(..) {
                client.offer(Frame::Message(gone.clone()));
            }
        }
        self.remove(id);
        Ok(())
    }

    /// Forgets a console and ends it, off the caller's thread: on Windows
    /// closing a console waits until its output has been read out.
    fn remove(&self, id: Uuid) {
        let Some(live) = self.sessions.lock().unwrap().remove(&id) else {
            return;
        };
        self.touch();
        std::thread::spawn(move || {
            if let Some(pty) = live.pty.get() {
                let _ = pty.kill();
            }
            drop(live);
        });
    }

    fn list(&self) -> Vec<SessionInfo> {
        let sessions: Vec<Arc<Live>> = self.sessions.lock().unwrap().values().cloned().collect();
        let mut list: Vec<SessionInfo> =
            sessions.iter().map(|live| live.info(&live.state.lock().unwrap())).collect();
        list.sort_by_key(|info| info.started_at);
        list
    }
}

impl Live {
    fn info(&self, state: &State) -> SessionInfo {
        SessionInfo {
            id: self.id,
            pid: self.pty.get().and_then(PtySession::pid),
            path: self.path.clone(),
            cwd: self.cwd.clone(),
            started_at: self.started_at,
            cols: state.cols,
            rows: state.rows,
            title: state.title.clone(),
            attached: state.clients.len() as u32,
            exit_code: state.exit_code,
        }
    }

    fn output(&self, bytes: Vec<u8>) {
        let answer = {
            let mut state = self.state.lock().unwrap();
            state.seq += 1;
            let seq = state.seq;
            if let Some(title) = last_title(&bytes) {
                state.title = Some(title);
            }
            let frame = Frame::Output { session: self.id, seq, bytes: bytes.clone() };
            state.clients.retain(|client| client.offer(frame.clone()));
            let asked = contains(&bytes, CURSOR_QUERY);
            state.history.push(seq, bytes);
            if asked {
                state.cursor_query = !state.clients.is_empty();
            }
            asked && state.clients.is_empty()
        };
        if answer {
            self.answer_cursor_query();
        }
    }

    /// No terminal to answer ConPTY's question: without an answer the
    /// console would never finish starting (see CURSOR_QUERY).
    fn answer_cursor_query(&self) {
        if let Some(pty) = self.pty.get() {
            let _ = pty.write(CURSOR_HOME_REPORT);
        }
    }
}

/// A console's latest output, newest last, up to HISTORY_BYTES.
#[derive(Default)]
struct History {
    chunks: VecDeque<(u64, Vec<u8>)>,
    bytes: usize,
    /// Whether output has been dropped from the front.
    trimmed: bool,
}

impl History {
    fn push(&mut self, seq: u64, bytes: Vec<u8>) {
        self.bytes += bytes.len();
        self.chunks.push_back((seq, bytes));
        while self.bytes > HISTORY_BYTES && self.chunks.len() > 1 {
            if let Some((_, dropped)) = self.chunks.pop_front() {
                self.bytes -= dropped.len();
                self.trimmed = true;
            }
        }
    }

    /// Output after `since`, as frames. When some of it is no longer kept,
    /// what is left starts with a terminal reset, so it isn't drawn on top
    /// of a screen it doesn't belong to.
    fn replay(&self, session: Uuid, since: u64) -> Vec<Frame<ServerMessage>> {
        let first = self.chunks.front().map(|(seq, _)| *seq);
        let gap = match first {
            Some(first) => self.trimmed && since + 1 < first,
            None => false,
        };
        let mut frames = Vec::new();
        if gap {
            let seq = first.unwrap_or(1) - 1;
            frames.push(Frame::Output { session, seq, bytes: RESET.to_vec() });
        }
        for (seq, bytes) in &self.chunks {
            if *seq > since {
                frames.push(Frame::Output { session, seq: *seq, bytes: without_queries(bytes) });
            }
        }
        frames
    }
}

/// `bytes` without the questions a program asks its terminal (where the
/// cursor is, what terminal it is, its colors, its size…). The program
/// asked them once, of a terminal that answered; replayed into another
/// one, it would answer again — into the console, where the answer arrives
/// as if typed at the prompt.
fn without_queries(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != 0x1b || at + 1 >= bytes.len() {
            out.push(bytes[at]);
            at += 1;
            continue;
        }
        let (end, query) = match bytes[at + 1] {
            b'[' => csi(bytes, at + 2),
            b']' => string_sequence(bytes, at + 2, |body| body.ends_with(b"?")),
            b'P' => string_sequence(bytes, at + 2, |body| body.starts_with(b"$q") || body.starts_with(b"+q")),
            _ => (at + 1, false),
        };
        if !query {
            out.extend_from_slice(&bytes[at..end]);
        }
        at = end;
    }
    out
}

/// A CSI sequence starting after `ESC [`: where it ends, and whether it is
/// a query.
fn csi(bytes: &[u8], start: usize) -> (usize, bool) {
    let mut at = start;
    while at < bytes.len() && !(0x40..=0x7e).contains(&bytes[at]) {
        at += 1;
    }
    let Some(&last) = bytes.get(at) else {
        return (bytes.len(), false);
    };
    let middle = &bytes[start..at];
    let query = match last {
        // Status and cursor position reports (DSR).
        b'n' => matches!(middle, b"5" | b"6" | b"?6" | b"?15" | b"?25" | b"?26" | b"?996"),
        // Device attributes: primary, secondary, tertiary.
        b'c' => middle.iter().all(|byte| byte.is_ascii_digit() || matches!(byte, b'>' | b'=')),
        // XTVERSION.
        b'q' => middle.starts_with(b">"),
        // DECRQM (ESC [ ? Ps $ p).
        b'p' => middle.ends_with(b"$"),
        // The kitty keyboard protocol's flags.
        b'u' => middle == b"?",
        // Window and character cell sizes.
        b't' => matches!(middle, b"11" | b"13" | b"14" | b"15" | b"16" | b"18" | b"19" | b"20" | b"21"),
        _ => false,
    };
    (at + 1, query)
}

/// An OSC or DCS sequence whose body starts at `start`, ended by BEL or ST:
/// where it ends, and whether `is_query` says its body asks something.
fn string_sequence(bytes: &[u8], start: usize, is_query: impl Fn(&[u8]) -> bool) -> (usize, bool) {
    let mut at = start;
    while at < bytes.len() {
        match bytes[at] {
            0x07 => return (at + 1, is_query(&bytes[start..at])),
            0x1b if bytes.get(at + 1) == Some(&b'\\') => return (at + 2, is_query(&bytes[start..at])),
            _ => at += 1,
        }
    }
    (bytes.len(), false)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

/// The last window title set in `bytes` (OSC 0 or 2, ended by BEL or ST).
fn last_title(bytes: &[u8]) -> Option<String> {
    let mut found = None;
    let mut at = 0;
    while let Some(offset) = bytes[at..].windows(2).position(|pair| pair == b"\x1b]") {
        let start = at + offset + 2;
        at = start;
        let rest = &bytes[start..];
        let kind_len = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        if kind_len == 0 || rest.get(kind_len) != Some(&b';') {
            continue;
        }
        if !matches!(&rest[..kind_len], b"0" | b"2") {
            continue;
        }
        let text = &rest[kind_len + 1..];
        let end = text.iter().position(|byte| *byte == 0x07 || *byte == 0x1b);
        if let Some(end) = end {
            found = Some(String::from_utf8_lossy(&text[..end]).into_owned());
        }
    }
    found
}

/// For tests: a daemon on an unused port, in this process.
#[cfg(test)]
pub fn start_for_test(token: &str) -> std::io::Result<u16> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
    let port = listener.local_addr()?.port();
    let daemon = Daemon::new(token.to_string());
    std::thread::spawn(move || serve(&daemon, listener));
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_the_newest_and_says_when_it_lost_some() {
        let id = Uuid::new_v4();
        let mut history = History::default();
        let chunk = vec![b'x'; HISTORY_BYTES / 2];
        history.push(1, chunk.clone());
        history.push(2, chunk.clone());
        assert!(!history.trimmed);
        history.push(3, chunk.clone());
        assert!(history.trimmed);

        let all = history.replay(id, 0);
        assert!(matches!(&all[0], Frame::Output { bytes, .. } if bytes == RESET), "starts with a reset");
        assert_eq!(all.len(), 3, "reset + the two kept");

        let newer = history.replay(id, 2);
        assert_eq!(newer.len(), 1);
        assert!(matches!(&newer[0], Frame::Output { seq: 3, .. }));
        assert!(history.replay(id, 3).is_empty());
    }

    #[test]
    fn a_replay_asks_the_terminal_nothing() {
        // ConPTY's startup question, DA1/DA2, colors, XTVERSION, DECRQM,
        // DECRQSS, the cell size.
        let asked = b"\x1b[6n\x1b[c\x1b[>0c\x1b]11;?\x07\x1b]10;?\x1b\\\x1b[>q\x1b[?2004$p\x1bP$qm\x1b\\\x1b[16t";
        assert_eq!(without_queries(asked), b"");
        // Everything else is drawn as it was.
        let drawn = b"\x1b[1;31mred\x1b[0m\x1b]0;title\x07\x1b[2J\x1b[?2004h\x1b[22t\x1b[?25l ok";
        assert_eq!(without_queries(drawn), drawn.to_vec());
        assert_eq!(without_queries(b"before\x1b[6nafter"), b"beforeafter");
    }

    #[test]
    fn finds_the_last_title_set() {
        assert_eq!(last_title(b"\x1b]0;one\x07ls\r\n\x1b]2;two\x1b\\"), Some("two".into()));
        assert_eq!(last_title(b"\x1b]7;file:///tmp\x07"), None, "OSC 7 is a directory, not a title");
        assert_eq!(last_title(b"plain"), None);
    }
}
