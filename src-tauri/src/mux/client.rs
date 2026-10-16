//! The app's side of the sessions daemon (see mux/mod.rs): one connection,
//! shared by every window, over which its consoles are started, shown and
//! typed into.
//!
//! What can wait doesn't: typing, resizing and detaching go out without
//! waiting for the daemon's answer, since some are sent from the window's
//! main thread. Starting, attaching, listing and killing wait for theirs.

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use dashmap::DashMap;
use uuid::Uuid;

use super::protocol::{read_frame, write_frame, ClientMessage, Frame, Op, Reply, ServerMessage, SessionInfo, PROTOCOL};
use crate::pty::{ConsoleEvent, EventSink, SpawnSpec};

/// How long a request waits for the daemon's answer.
const REPLY_TIMEOUT: Duration = Duration::from_secs(15);

/// Where the daemon is: the folder of its info file (the app), or a port
/// and token given outright (tests).
#[derive(Clone)]
pub enum Endpoint {
    Dir(PathBuf),
    #[cfg_attr(not(test), allow(dead_code))]
    Direct { port: u16, token: String },
}

pub struct MuxClient {
    endpoint: Endpoint,
    /// What runs as the daemon: this executable (another one in tests).
    program: PathBuf,
    conn: Mutex<Option<Arc<Conn>>>,
    /// The consoles this app shows, by id.
    shown: DashMap<Uuid, Shown>,
    /// Lets the reader thread reach back without keeping the client alive.
    me: Weak<MuxClient>,
}

struct Shown {
    sink: EventSink,
    /// The window it's in (see terminal_windows.rs).
    owner: String,
    pid: Option<u32>,
    /// The number of the last output it showed, to pick up from there.
    seq: Arc<AtomicU64>,
}

struct Conn {
    writer: Mutex<TcpStream>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Result<Reply, String>>>>,
    next_id: AtomicU64,
    alive: AtomicBool,
}

impl MuxClient {
    pub fn new(endpoint: Endpoint) -> Arc<Self> {
        Self::running(endpoint, std::env::current_exe().unwrap_or_default())
    }

    fn running(endpoint: Endpoint, program: PathBuf) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            endpoint,
            program,
            conn: Mutex::new(None),
            shown: DashMap::new(),
            me: me.clone(),
        })
    }

    /// The live connection, connecting — and starting the daemon — first if
    /// there is none.
    fn conn(&self) -> Result<Arc<Conn>, String> {
        let mut slot = self.conn.lock().unwrap();
        if let Some(conn) = slot.as_ref().filter(|conn| conn.alive.load(Ordering::SeqCst)) {
            return Ok(Arc::clone(conn));
        }
        let conn = self.open(true).map_err(|err| err.to_string())?;
        *slot = Some(Arc::clone(&conn));
        Ok(conn)
    }

    fn open(&self, may_launch: bool) -> io::Result<Arc<Conn>> {
        let (stream, token) = match &self.endpoint {
            Endpoint::Dir(dir) => {
                let build = super::build_stamp(&self.program);
                let found = super::connect(dir).filter(|(_, info)| info.build == build || !cfg!(debug_assertions));
                match found {
                    Some((stream, info)) => (stream, info.token),
                    None if may_launch => {
                        retire_stale_dev_daemon(dir);
                        super::launch(dir, &self.program)?;
                        let (stream, info) = super::connect(dir)
                            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "sessions daemon unreachable"))?;
                        (stream, info.token)
                    }
                    None => return Err(io::Error::new(io::ErrorKind::NotConnected, "no sessions daemon")),
                }
            }
            Endpoint::Direct { port, token } => {
                let addr = SocketAddr::from(([127, 0, 0, 1], *port));
                (TcpStream::connect_timeout(&addr, Duration::from_secs(2))?, token.clone())
            }
        };
        handshake(stream, &token).map(|(reader, writer)| {
            let conn = Arc::new(Conn {
                writer: Mutex::new(writer),
                pending: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(1),
                alive: AtomicBool::new(true),
            });
            let me = self.me.clone();
            let for_reader = Arc::clone(&conn);
            std::thread::spawn(move || read_loop(me, for_reader, reader));
            conn
        })
    }

    /// Sends a request and waits for its answer.
    fn call(&self, op: Op) -> Result<Reply, String> {
        let conn = self.conn()?;
        let id = conn.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        conn.pending.lock().unwrap().insert(id, tx);
        if let Err(err) = conn.send(&Frame::Message(ClientMessage::Request { id, op })) {
            conn.pending.lock().unwrap().remove(&id);
            return Err(err);
        }
        match rx.recv_timeout(REPLY_TIMEOUT) {
            Ok(result) => result,
            Err(_) => {
                conn.pending.lock().unwrap().remove(&id);
                Err("the sessions daemon did not answer".into())
            }
        }
    }

    /// Sends a request without waiting: its answer is dropped on arrival.
    fn cast(&self, op: Op) -> Result<(), String> {
        let conn = self.conn()?;
        let id = conn.next_id.fetch_add(1, Ordering::SeqCst);
        conn.send(&Frame::Message(ClientMessage::Request { id, op }))
    }

    pub fn spawn(&self, owner: &str, spec: &SpawnSpec, sink: EventSink) -> Result<Uuid, String> {
        self.spawn_as(owner, Uuid::new_v4(), spec, sink)
    }

    fn spawn_as(&self, owner: &str, id: Uuid, spec: &SpawnSpec, sink: EventSink) -> Result<Uuid, String> {
        // Listening before the daemon starts it: its first output may come
        // before the answer.
        self.shown.insert(id, Shown { sink, owner: owner.to_string(), pid: None, seq: Arc::default() });
        match self.call(Op::Spawn { session: id, spec: spec.clone() }) {
            Ok(Reply::Spawned { pid }) => {
                if let Some(mut shown) = self.shown.get_mut(&id) {
                    shown.pid = pid;
                }
                Ok(id)
            }
            Ok(other) => {
                self.shown.remove(&id);
                Err(format!("unexpected answer: {other:?}"))
            }
            Err(err) => {
                self.shown.remove(&id);
                Err(err)
            }
        }
    }

    /// Shows a console the daemon already runs: what it printed so far,
    /// then everything new.
    pub fn attach(&self, owner: &str, id: Uuid, sink: EventSink) -> Result<SessionInfo, String> {
        let seq = Arc::new(AtomicU64::new(0));
        self.shown.insert(id, Shown { sink, owner: owner.to_string(), pid: None, seq });
        match self.call(Op::Attach { session: id, since: 0 }) {
            Ok(Reply::Attached { info }) => {
                if let Some(mut shown) = self.shown.get_mut(&id) {
                    shown.pid = info.pid;
                }
                Ok(info)
            }
            Ok(other) => {
                self.shown.remove(&id);
                Err(format!("unexpected answer: {other:?}"))
            }
            Err(err) => {
                self.shown.remove(&id);
                Err(err)
            }
        }
    }

    pub fn shows(&self, id: Uuid) -> bool {
        self.shown.contains_key(&id)
    }

    pub fn write(&self, id: Uuid, bytes: &[u8]) -> Result<(), String> {
        self.conn()?.send(&Frame::Input { session: id, bytes: bytes.to_vec() })
    }

    pub fn resize(&self, id: Uuid, cols: u16, rows: u16) -> Result<(), String> {
        self.cast(Op::Resize { session: id, cols, rows })
    }

    pub fn prompt_redraw(&self, id: Uuid) -> Result<(), String> {
        self.cast(Op::PromptRedraw { session: id })
    }

    pub fn kill(&self, id: Uuid) -> Result<(), String> {
        self.shown.remove(&id);
        self.call(Op::Kill { session: id }).map(|_| ())
    }

    /// Stops showing a console; it keeps running in the daemon.
    pub fn detach(&self, id: Uuid) {
        if self.shown.remove(&id).is_some() {
            let _ = self.cast(Op::Detach { session: id });
        }
    }

    /// Stops showing a window's consoles: it closed or reloaded.
    pub fn detach_owned_by(&self, owner: &str) {
        let ids: Vec<Uuid> =
            self.shown.iter().filter(|shown| shown.owner == owner).map(|shown| *shown.key()).collect();
        for id in ids {
            self.detach(id);
        }
    }

    pub fn list(&self) -> Result<Vec<SessionInfo>, String> {
        match self.call(Op::List)? {
            Reply::Sessions { sessions } => Ok(sessions),
            other => Err(format!("unexpected answer: {other:?}")),
        }
    }

    /// Lists the daemon's consoles only if one is already running: asking
    /// must not start one.
    pub fn list_if_running(&self) -> Vec<SessionInfo> {
        let running = self.conn.lock().unwrap().as_ref().is_some_and(|conn| conn.alive.load(Ordering::SeqCst));
        if !running {
            let Ok(conn) = self.open(false) else {
                return Vec::new();
            };
            *self.conn.lock().unwrap() = Some(conn);
        }
        self.list().unwrap_or_default()
    }

    pub fn pid(&self, id: Uuid) -> Option<u32> {
        self.shown.get(&id).and_then(|shown| shown.pid)
    }

    pub fn roots(&self) -> Vec<(Uuid, u32)> {
        self.shown.iter().filter_map(|shown| Some((*shown.key(), shown.pid?))).collect()
    }

    /// The connection dropped. If the daemon is still there (it was the
    /// connection, not the daemon), every console is attached again from
    /// where it was; otherwise they are gone, and their tabs are told.
    fn reconnect(&self) {
        let reattached = (0..3).find_map(|attempt| {
            std::thread::sleep(Duration::from_millis(200 * (attempt + 1)));
            self.open(false).ok()
        });
        let ids: Vec<Uuid> = self.shown.iter().map(|shown| *shown.key()).collect();
        let Some(conn) = reattached else {
            for id in ids {
                if let Some((_, shown)) = self.shown.remove(&id) {
                    (shown.sink)(ConsoleEvent::Exit(-1));
                }
            }
            return;
        };
        *self.conn.lock().unwrap() = Some(conn);
        for id in ids {
            let since = self.shown.get(&id).map_or(0, |shown| shown.seq.load(Ordering::SeqCst));
            let result = self.call(Op::Attach { session: id, since });
            if !matches!(result, Ok(Reply::Attached { .. })) {
                if let Some((_, shown)) = self.shown.remove(&id) {
                    (shown.sink)(ConsoleEvent::Exit(-1));
                }
            }
        }
    }
}

/// The reader thread holds the connection too: closing it is what ends
/// that thread — and, to the daemon, what detaches this app's consoles.
impl Drop for MuxClient {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.get_mut().unwrap().take() {
            conn.alive.store(false, Ordering::SeqCst);
            let _ = conn.writer.lock().unwrap().shutdown(std::net::Shutdown::Both);
        }
    }
}

impl Conn {
    fn send(&self, frame: &Frame<ClientMessage>) -> Result<(), String> {
        if !self.alive.load(Ordering::SeqCst) {
            return Err("the sessions daemon is not connected".into());
        }
        let mut writer = self.writer.lock().unwrap();
        write_frame(&mut *writer, frame).map_err(|err| {
            self.alive.store(false, Ordering::SeqCst);
            err.to_string()
        })
    }
}

fn handshake(mut stream: TcpStream, token: &str) -> io::Result<(TcpStream, TcpStream)> {
    let _ = stream.set_nodelay(true);
    let hello = ClientMessage::Hello {
        protocol: PROTOCOL,
        token: token.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    write_frame(&mut stream, &Frame::Message(hello))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    match read_frame::<ServerMessage>(&mut stream)? {
        Some(Frame::Message(ServerMessage::Welcome { .. })) => {}
        Some(Frame::Message(ServerMessage::Rejected { reason })) => {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, reason));
        }
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "no welcome from the sessions daemon")),
    }
    stream.set_read_timeout(None)?;
    let writer = stream.try_clone()?;
    Ok((stream, writer))
}

fn read_loop(client: Weak<MuxClient>, conn: Arc<Conn>, mut reader: TcpStream) {
    while let Ok(Some(frame)) = read_frame::<ServerMessage>(&mut reader) {
        let Some(client) = client.upgrade() else {
            return;
        };
        match frame {
            Frame::Output { session, seq, bytes } => {
                let Some(shown) = client.shown.get(&session) else {
                    continue;
                };
                // Already shown (a replay overlapping what came live).
                if seq != 0 && seq <= shown.seq.load(Ordering::SeqCst) {
                    continue;
                }
                shown.seq.store(seq, Ordering::SeqCst);
                let sink = Arc::clone(&shown.sink);
                drop(shown);
                // The tab is gone without having said so (its window
                // reloaded): the console stays, unshown.
                if !sink(ConsoleEvent::Output(bytes)) {
                    client.detach(session);
                }
            }
            Frame::Message(ServerMessage::Reply { id, result }) => {
                if let Some(waiting) = conn.pending.lock().unwrap().remove(&id) {
                    let _ = waiting.send(result);
                }
            }
            Frame::Message(ServerMessage::Exit { session, code }) => {
                if let Some(sink) = client.shown.get(&session).map(|shown| Arc::clone(&shown.sink)) {
                    sink(ConsoleEvent::Exit(code));
                }
            }
            Frame::Message(ServerMessage::Foreground { session, path }) => {
                if let Some(sink) = client.shown.get(&session).map(|shown| Arc::clone(&shown.sink)) {
                    sink(ConsoleEvent::Foreground(path));
                }
            }
            Frame::Message(ServerMessage::Gone { session }) => {
                if let Some((_, shown)) = client.shown.remove(&session) {
                    (shown.sink)(ConsoleEvent::Exit(-1));
                }
            }
            _ => {}
        }
    }
    conn.alive.store(false, Ordering::SeqCst);
    // Whoever was waiting gets an error now rather than at the timeout.
    for (_, waiting) in conn.pending.lock().unwrap().drain() {
        let _ = waiting.send(Err("the sessions daemon disconnected".into()));
    }
    if let Some(client) = client.upgrade() {
        if !client.shown.is_empty() {
            client.reconnect();
        }
    }
}

/// In a dev build, a daemon started by an earlier build is shut down, its
/// consoles with it, so that changes to the daemon's own code take effect
/// on the next run. Restarting the same build keeps them. Release builds
/// never do this: an update keeps the old daemon (see protocol::PROTOCOL).
fn retire_stale_dev_daemon(dir: &std::path::Path) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Some((stream, info)) = super::connect(dir) else {
        return;
    };
    let Ok((mut reader, mut writer)) = handshake(stream, &info.token) else {
        return;
    };
    let shutdown = ClientMessage::Request { id: 1, op: Op::Shutdown };
    if write_frame(&mut writer, &Frame::Message(shutdown)).is_ok() {
        let _ = reader.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = read_frame::<ServerMessage>(&mut reader);
    }
    // Until the old one has let go of the info file.
    for _ in 0..40 {
        if super::read_info(dir).is_none_or(|current| current.pid != info.pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// What a console printed and whether it exited, as a tab would see it.
    #[derive(Default)]
    struct Seen {
        output: Mutex<Vec<u8>>,
        exit: Mutex<Option<i32>>,
    }

    impl Seen {
        fn text(&self) -> String {
            String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned()
        }

        fn wait_for(&self, what: impl Fn(&Seen) -> bool) -> bool {
            let deadline = Instant::now() + Duration::from_secs(20);
            while Instant::now() < deadline {
                if what(self) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            false
        }
    }

    /// A sink that records, and answers ConPTY's cursor question the way
    /// xterm does — through the client, into the console.
    fn terminal(client: &Arc<MuxClient>, seen: &Arc<Seen>, id: &Arc<Mutex<Option<Uuid>>>) -> EventSink {
        let client = Arc::downgrade(client);
        let seen = Arc::clone(seen);
        let id = Arc::clone(id);
        Arc::new(move |event| {
            match event {
                ConsoleEvent::Output(bytes) => {
                    if bytes.windows(4).any(|window| window == b"\x1b[6n") {
                        if let (Some(client), Some(id)) = (client.upgrade(), *id.lock().unwrap()) {
                            let _ = client.write(id, b"\x1b[1;1R");
                        }
                    }
                    seen.output.lock().unwrap().extend(bytes);
                }
                ConsoleEvent::Exit(code) => *seen.exit.lock().unwrap() = Some(code),
                ConsoleEvent::Foreground(_) => {}
            }
            true
        })
    }

    fn spec(script: &str) -> SpawnSpec {
        let (path, args) = if cfg!(windows) {
            ("cmd.exe".to_string(), vec!["/d".to_string(), "/c".to_string(), script.to_string()])
        } else {
            ("/bin/sh".to_string(), vec!["-c".to_string(), script.to_string()])
        };
        SpawnSpec { path, args, env: Vec::new(), cwd: None, cols: 80, rows: 24, prompt_redraw: None, extra_path: Vec::new() }
    }

    fn client(port: u16, token: &str) -> Arc<MuxClient> {
        MuxClient::new(Endpoint::Direct { port, token: token.to_string() })
    }

    /// Spawns `script`, answering ConPTY on the way, and waits for `marker`.
    fn spawn_until(client: &Arc<MuxClient>, script: &str, marker: &str) -> (Uuid, Arc<Seen>) {
        let seen = Arc::new(Seen::default());
        // Known to the sink before the console starts: ConPTY's question
        // comes before spawn() returns.
        let id = Uuid::new_v4();
        let sink = terminal(client, &seen, &Arc::new(Mutex::new(Some(id))));
        client.spawn_as("main", id, &spec(script), sink).unwrap();
        assert!(seen.wait_for(|seen| seen.text().contains(marker)), "never printed {marker:?}: {:?}", seen.text());
        (id, seen)
    }

    #[test]
    fn a_console_survives_its_client_and_shows_again_elsewhere() {
        let token = "test-token";
        let port = crate::mux::server::start_for_test(token).unwrap();
        let first = client(port, token);
        let script = if cfg!(windows) { "echo uno && ping -n 30 127.0.0.1 >nul" } else { "echo uno; sleep 30" };
        let (id, _) = spawn_until(&first, script, "uno");

        // The app quits: its connection goes, the console stays.
        drop(first);
        let second = client(port, token);
        let listed = second.list().unwrap();
        assert!(listed.iter().any(|info| info.id == id && info.exit_code.is_none()), "still running: {listed:?}");

        // Another app attaches and gets the screen back.
        let seen = Arc::new(Seen::default());
        let id_cell = Arc::new(Mutex::new(Some(id)));
        second.attach("main", id, terminal(&second, &seen, &id_cell)).unwrap();
        assert!(seen.wait_for(|seen| seen.text().contains("uno")), "replay: {:?}", seen.text());

        second.kill(id).unwrap();
        assert!(second.list().unwrap().iter().all(|info| info.id != id), "killed and gone");
    }

    #[test]
    fn an_exited_console_reports_and_goes_once_unshown() {
        let token = "test-token-2";
        let port = crate::mux::server::start_for_test(token).unwrap();
        let client = client(port, token);
        let (id, seen) = spawn_until(&client, "echo hola", "hola");
        assert!(seen.wait_for(|seen| seen.exit.lock().unwrap().is_some()), "exit reported");

        client.detach(id);
        let deadline = Instant::now() + Duration::from_secs(10);
        while client.list().unwrap().iter().any(|info| info.id == id) {
            assert!(Instant::now() < deadline, "an exited console nobody shows is forgotten");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The real thing: `termo --mux`, started detached by the client the way
    /// the app does, in a folder of its own. Needs the app built first
    /// (`cargo build`), hence ignored by default:
    /// `cargo test --lib launches_the_real_daemon -- --ignored`.
    #[test]
    #[ignore]
    fn launches_the_real_daemon() {
        let exe_name = if cfg!(windows) { "termo.exe" } else { "termo" };
        let exe = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("debug").join(exe_name);
        assert!(exe.exists(), "build the app first: {}", exe.display());
        let dir = std::env::temp_dir().join(format!("termo-mux-test-{}", Uuid::new_v4().simple()));

        let client = MuxClient::running(Endpoint::Dir(dir.clone()), exe.clone());
        let script = if cfg!(windows) { "echo desde-el-daemon && ping -n 30 127.0.0.1 >nul" } else { "echo desde-el-daemon; sleep 30" };
        let (id, _) = spawn_until(&client, script, "desde-el-daemon");
        let info = super::super::read_info(&dir).expect("the daemon wrote its info file");
        assert_ne!(info.pid, std::process::id(), "a process of its own");

        // Another app finds the same daemon through the folder, and the
        // console in it.
        let other = MuxClient::running(Endpoint::Dir(dir.clone()), exe);
        let listed = other.list_if_running();
        assert!(listed.iter().any(|session| session.id == id), "listed: {listed:?}");

        client.kill(id).unwrap();
        assert!(matches!(client.call(Op::Shutdown), Ok(Reply::Done)));
        let deadline = Instant::now() + Duration::from_secs(10);
        while super::super::read_info(&dir).is_some() {
            assert!(Instant::now() < deadline, "the daemon removed its info file on the way out");
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wrong_token_is_turned_away() {
        let port = crate::mux::server::start_for_test("right").unwrap();
        assert!(client(port, "wrong").list().is_err());
    }
}

