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
use std::sync::mpsc::{self, sync_channel, SyncSender};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use dashmap::DashMap;
use uuid::Uuid;

use super::protocol::{read_frame, write_frame, ClientMessage, Frame, Op, Reply, ServerMessage, SessionInfo, PROTOCOL};
use crate::pty::{ConsoleEvent, EventSink, SpawnSpec};

/// How long a request waits for the answer of another computer's daemon.
const REPLY_TIMEOUT: Duration = Duration::from_secs(15);
/// The same, for this computer's: it answers in milliseconds, and a new
/// console waiting on it opens in the app instead (see pty/consoles.rs) —
/// the sooner, the better.
const LOCAL_REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// Once it has stopped answering: long enough to notice it's back, without
/// every new console waiting the whole time again.
const NOT_ANSWERING_TIMEOUT: Duration = Duration::from_millis(1500);
/// Frames waiting to go out unawaited (see post).
const OUTBOX: usize = 4096;
/// How often a daemon that stopped answering is asked again.
const HEALTH_RECHECK: Duration = Duration::from_secs(5);

/// Where the daemon is: the folder of its info file (the app), or a port
/// and token given outright (tests).
#[derive(Clone)]
pub enum Endpoint {
    Dir(PathBuf),
    #[cfg_attr(not(test), allow(dead_code))]
    Direct { port: u16, token: String },
    /// Another computer's daemon, over the tailnet: no token, Tailscale
    /// tells it who this is (see mux/remote.rs).
    Remote(SocketAddr),
}

pub struct MuxClient {
    endpoint: Endpoint,
    /// What runs as the daemon: this executable (another one in tests).
    program: PathBuf,
    conn: Mutex<Option<Arc<Conn>>>,
    /// The consoles this app shows, by id.
    shown: DashMap<Uuid, Shown>,
    /// Whether the daemon answered the last time it was asked; what
    /// `on_health` is told when that changes.
    answering: AtomicBool,
    on_health: OnceLock<Box<dyn Fn(bool) + Send + Sync>>,
    /// Where what the tailnet asks of the app's own consoles goes (see
    /// pty/sharing.rs).
    on_host: OnceLock<Box<dyn Fn(HostEvent) + Send + Sync>>,
    /// What goes out without anyone waiting (see post).
    outbox: OnceLock<SyncSender<(u64, Outgoing)>>,
    /// Lets the reader thread reach back without keeping the client alive.
    me: Weak<MuxClient>,
}

/// What goes out unawaited (see MuxClient::post).
pub enum Outgoing {
    Op(Op),
    /// What one of the app's own consoles printed.
    Output { session: Uuid, bytes: Vec<u8> },
}

/// What the tailnet asks of one of the app's own consoles.
pub enum HostEvent {
    Input { session: Uuid, bytes: Vec<u8> },
    Resize { session: Uuid, cols: u16, rows: u16 },
    PromptRedraw { session: Uuid },
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
    /// Which connection this is: frames posted for an earlier one are
    /// dropped (see post).
    generation: u64,
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
            answering: AtomicBool::new(true),
            on_health: OnceLock::new(),
            on_host: OnceLock::new(),
            outbox: OnceLock::new(),
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
        let conn = self.open(true).map_err(|err| {
            self.set_answering(false);
            err.to_string()
        })?;
        *slot = Some(Arc::clone(&conn));
        Ok(conn)
    }

    /// Calls `on_health` from now on whenever the daemon stops answering, or
    /// starts again.
    pub fn on_health(&self, report: impl Fn(bool) + Send + Sync + 'static) {
        let _ = self.on_health.set(Box::new(report));
    }

    /// Whether the daemon answered the last time it was asked.
    pub fn is_answering(&self) -> bool {
        self.answering.load(Ordering::SeqCst)
    }

    fn set_answering(&self, answering: bool) {
        if self.answering.swap(answering, Ordering::SeqCst) == answering {
            return;
        }
        if let Some(report) = self.on_health.get() {
            report(answering);
        }
        if !answering && matches!(self.endpoint, Endpoint::Dir(_)) {
            // Asked again every few seconds until it answers — nothing else
            // may ask, and the app should know it's back.
            let me = self.me.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(HEALTH_RECHECK);
                let Some(client) = me.upgrade() else {
                    return;
                };
                if client.is_answering() || client.list().is_ok() {
                    return;
                }
            });
        }
    }

    /// Replaces a daemon that stopped answering with a new one. The old one
    /// is ended — it can't be asked to —, and its consoles with it: their
    /// tabs are told they're gone.
    pub fn restart(&self) -> Result<(), String> {
        let Endpoint::Dir(dir) = &self.endpoint else {
            return Err("only this computer's sessions daemon can be restarted".into());
        };
        // Nothing shown first, so the connection ending isn't taken for a
        // dropped one to reattach over (see read_loop).
        self.forget_all();
        if let Some(conn) = self.conn.lock().unwrap().take() {
            conn.alive.store(false, Ordering::SeqCst);
            let _ = conn.writer.lock().unwrap().shutdown(std::net::Shutdown::Both);
        }
        super::end_daemon(dir);
        self.conn()?;
        self.set_answering(true);
        Ok(())
    }

    fn reply_timeout(&self) -> Duration {
        match self.endpoint {
            Endpoint::Remote(_) => REPLY_TIMEOUT,
            _ if self.is_answering() => LOCAL_REPLY_TIMEOUT,
            _ => NOT_ANSWERING_TIMEOUT,
        }
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
            Endpoint::Remote(addr) => (TcpStream::connect_timeout(addr, Duration::from_secs(4))?, String::new()),
        };
        handshake(stream, &token).map(|(reader, writer)| {
            static GENERATIONS: AtomicU64 = AtomicU64::new(1);
            let conn = Arc::new(Conn {
                generation: GENERATIONS.fetch_add(1, Ordering::SeqCst),
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
        match rx.recv_timeout(self.reply_timeout()) {
            Ok(result) => result,
            Err(_) => {
                conn.pending.lock().unwrap().remove(&id);
                self.set_answering(false);
                Err("the sessions daemon did not answer".into())
            }
        }
    }

    /// The connection there is now — connecting, and starting the daemon,
    /// first if need be —, by its generation (see post).
    pub fn connected(&self) -> Result<u64, String> {
        Ok(self.conn()?.generation)
    }

    pub fn on_host(&self, handle: impl Fn(HostEvent) + Send + Sync + 'static) {
        let _ = self.on_host.set(Box::new(handle));
    }

    /// Sends a frame over connection `generation` without ever waiting: the
    /// app's own consoles print from their reader threads, which mustn't
    /// stall on a daemon that's slow or gone. A frame for a connection that
    /// has since ended is dropped (whoever posted it starts over on the new
    /// one), and so are frames past what the queue holds.
    pub fn post(&self, generation: u64, frame: Outgoing) {
        let outbox = self.outbox.get_or_init(|| {
            let (tx, rx) = sync_channel::<(u64, Outgoing)>(OUTBOX);
            let me = self.me.clone();
            std::thread::spawn(move || {
                for (generation, frame) in rx {
                    let Some(client) = me.upgrade() else {
                        return;
                    };
                    let conn = client.conn.lock().unwrap().clone();
                    let Some(conn) = conn.filter(|conn| conn.generation == generation) else {
                        continue;
                    };
                    let frame = match frame {
                        Outgoing::Op(op) => {
                            Frame::Message(ClientMessage::Request { id: conn.next_id.fetch_add(1, Ordering::SeqCst), op })
                        }
                        Outgoing::Output { session, bytes } => Frame::Output { session, seq: 0, bytes },
                    };
                    let _ = conn.send(&frame);
                }
            });
            tx
        });
        let _ = outbox.try_send((generation, frame));
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
                // Started after all, once it answers again: no one shows it.
                let _ = self.cast(Op::Kill { session: id });
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

    /// The console back to its tab's size, if someone else (a phone)
    /// sized it meanwhile: whether it changed.
    pub fn claim(&self, id: Uuid, cols: u16, rows: u16) -> Result<bool, String> {
        match self.call(Op::Claim { session: id, cols, rows })? {
            Reply::Claimed { changed } => Ok(changed),
            other => Err(format!("unexpected reply: {other:?}")),
        }
    }

    pub fn prompt_redraw(&self, id: Uuid) -> Result<(), String> {
        self.cast(Op::PromptRedraw { session: id })
    }

    /// Ends a console. Its tab stops showing it first, so the daemon's word
    /// that it's gone doesn't reach it as an exit; if it can't be ended (a
    /// read-only connection), it keeps showing it.
    pub fn kill(&self, id: Uuid) -> Result<(), String> {
        let shown = self.shown.remove(&id);
        let result = self.call(Op::Kill { session: id }).map(|_| ());
        if result.is_err() {
            if let Some((id, shown)) = shown {
                self.shown.insert(id, shown);
            }
        }
        result
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

    /// Who is connected to this daemon from the tailnet.
    pub fn viewers(&self) -> Result<Vec<super::remote::Viewer>, String> {
        match self.call(Op::Viewers)? {
            Reply::Viewers { viewers } => Ok(viewers),
            other => Err(format!("unexpected answer: {other:?}")),
        }
    }

    /// Ends every connection from the tailnet.
    pub fn disconnect_remote(&self) -> Result<(), String> {
        self.call(Op::DisconnectRemote).map(|_| ())
    }

    /// The remote access settings changed (remote.json): applied now.
    pub fn reload_remote(&self) -> Result<(), String> {
        self.call(Op::ReloadRemote).map(|_| ())
    }

    /// Whether there's a daemon to talk to — connected already, or running
    /// and connected to now — without starting one.
    pub fn is_connected_or_running(&self) -> bool {
        if self.conn.lock().unwrap().as_ref().is_some_and(|conn| conn.alive.load(Ordering::SeqCst)) {
            return true;
        }
        let Ok(conn) = self.open(false) else {
            return false;
        };
        *self.conn.lock().unwrap() = Some(conn);
        true
    }

    /// Lists the daemon's consoles only if one is already running: asking
    /// must not start one.
    pub fn list_if_running(&self) -> Vec<SessionInfo> {
        if !self.is_connected_or_running() {
            return Vec::new();
        }
        self.list().unwrap_or_default()
    }

    pub fn pid(&self, id: Uuid) -> Option<u32> {
        self.shown.get(&id).and_then(|shown| shown.pid)
    }

    pub fn roots(&self) -> Vec<(Uuid, u32)> {
        self.shown.iter().filter_map(|shown| Some((*shown.key(), shown.pid?))).collect()
    }

    /// Tells every tab showing a console through this client that it's
    /// gone, and stops showing them.
    fn forget_all(&self) {
        let ids: Vec<Uuid> = self.shown.iter().map(|shown| *shown.key()).collect();
        for id in ids {
            if let Some((_, shown)) = self.shown.remove(&id) {
                (shown.sink)(ConsoleEvent::Exit(-1));
            }
        }
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
            self.forget_all();
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
                client.set_answering(true);
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
            Frame::Input { session, bytes } => {
                if let Some(handle) = client.on_host.get() {
                    handle(HostEvent::Input { session, bytes });
                }
            }
            Frame::Message(ServerMessage::HostResize { session, cols, rows }) => {
                if let Some(handle) = client.on_host.get() {
                    handle(HostEvent::Resize { session, cols, rows });
                }
            }
            Frame::Message(ServerMessage::HostPromptRedraw { session }) => {
                if let Some(handle) = client.on_host.get() {
                    handle(HostEvent::PromptRedraw { session });
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
    // Closed from this side too, so the daemon sees the end at once.
    let _ = conn.writer.lock().unwrap().shutdown(std::net::Shutdown::Both);
    // Whoever was waiting gets an error now rather than at the timeout.
    for (_, waiting) in conn.pending.lock().unwrap().drain() {
        let _ = waiting.send(Err("the sessions daemon disconnected".into()));
    }
    if let Some(client) = client.upgrade() {
        // Another computer that ended the connection meant to (it was
        // turned off, or this one disconnected): no coming back by itself.
        if matches!(client.endpoint, Endpoint::Remote(_)) {
            client.forget_all();
        } else if !client.shown.is_empty() {
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
    use crate::mux::protocol::{AgentInfo, HostedConsole, PlanUse};
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

    /// Remote access, with a stand-in for Tailscale's `whois` that says
    /// whoever the test is pretending to be.
    #[test]
    fn the_tailnet_gets_in_as_far_as_the_list_says() {
        use crate::mux::remote::{self, Access, Allowed, RemoteConfig};
        use crate::mux::tailscale::Identity;

        let dir = std::env::temp_dir().join(format!("termo-remote-{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = RemoteConfig {
            enabled: true,
            port: 0,
            allowed: vec![
                Allowed { login: "me@example.com".into(), node: None, access: Access::Full },
                Allowed { login: "friend@example.com".into(), node: None, access: Access::ReadOnly },
            ],
        };
        remote::write_config(&dir, &config).unwrap();
        let pretending: Arc<Mutex<Option<Identity>>> = Arc::default();
        let whois = {
            let pretending = Arc::clone(&pretending);
            Box::new(move |_| pretending.lock().unwrap().clone())
        };
        let token = "remote-test-token";
        let (port, remote_port) = crate::mux::server::start_remote_for_test(token, &dir, whois).unwrap();
        let remote_addr = SocketAddr::from(([127, 0, 0, 1], remote_port));
        let pretend = |login: &str, node: &str| {
            *pretending.lock().unwrap() = Some(Identity { login: login.into(), node: node.into() });
            MuxClient::new(Endpoint::Remote(remote_addr))
        };

        // A console on this computer, reading what's typed into it.
        let local = client(port, token);
        // `call`: cmd expands %x% when it reads the line, before `set /p`.
        let script = if cfg!(windows) { "echo listo && set /p x= && call echo got-%x% && ping -n 30 127.0.0.1 >nul" } else { "echo listo; read x; echo got-$x; sleep 30" };
        let (id, seen_here) = spawn_until(&local, script, "listo");

        // Someone not on the list: turned away, and left pending.
        assert!(pretend("stranger@example.com", "laptop").list().is_err());
        let pending = remote::read_pending(&dir);
        assert!(pending.iter().any(|entry| entry.login == "stranger@example.com"), "pending: {pending:?}");

        // Read-only: sees the console, can't type into it nor close it.
        let friend = pretend("friend@example.com", "tablet");
        assert!(friend.list().unwrap().iter().any(|session| session.id == id));
        let seen_by_friend = Arc::new(Seen::default());
        friend.attach("main", id, terminal(&friend, &seen_by_friend, &Arc::new(Mutex::new(Some(id))))).unwrap();
        assert!(seen_by_friend.wait_for(|seen| seen.text().contains("listo")), "the friend sees the screen");
        friend.write(id, b"from-friend\r").unwrap();
        assert!(friend.kill(id).is_err(), "read-only can't close it");

        // Full access: typed in, and everyone showing it sees the answer.
        let me = pretend("me@example.com", "phone");
        let seen_by_me = Arc::new(Seen::default());
        me.attach("main", id, terminal(&me, &seen_by_me, &Arc::new(Mutex::new(Some(id))))).unwrap();
        me.write(id, b"from-me\r").unwrap();
        assert!(seen_here.wait_for(|seen| seen.text().contains("got-from-me")), "typed from the phone: {:?}", seen_here.text());
        assert!(seen_by_friend.wait_for(|seen| seen.text().contains("got-from-me")));
        assert!(!seen_here.text().contains("from-friend"), "the read-only typing went nowhere");

        // Sized to the phone's screen while it shows it, and given its own
        // size back when the phone leaves.
        let size = || local.list().unwrap().into_iter().find(|session| session.id == id).map(|s| (s.cols, s.rows)).unwrap();
        let own = size();
        me.call(Op::Resize { session: id, cols: 41, rows: 17 }).unwrap();
        assert_eq!(size(), (41, 17));
        assert!(friend.call(Op::Resize { session: id, cols: 30, rows: 10 }).is_err(), "read-only can't resize it");
        me.detach(id);
        let deadline = Instant::now() + Duration::from_secs(5);
        while size() != own {
            assert!(Instant::now() < deadline, "its own size back: {:?} vs {own:?}", size());
            std::thread::sleep(Duration::from_millis(20));
        }
        // Resized here meanwhile: that's its size now, kept when the phone goes.
        me.attach("main", id, terminal(&me, &seen_by_me, &Arc::new(Mutex::new(Some(id))))).unwrap();
        me.call(Op::Resize { session: id, cols: 41, rows: 17 }).unwrap();
        local.call(Op::Resize { session: id, cols: 90, rows: 25 }).unwrap();
        me.detach(id);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(size(), (90, 25));
        me.attach("main", id, terminal(&me, &seen_by_me, &Arc::new(Mutex::new(Some(id))))).unwrap();
        // Its tab taken up here again, the phone still on it: its size back
        // at once — and asking again changes nothing.
        me.call(Op::Resize { session: id, cols: 41, rows: 17 }).unwrap();
        assert!(local.claim(id, 90, 25).unwrap());
        assert_eq!(size(), (90, 25));
        assert!(!local.claim(id, 90, 25).unwrap());
        // The phone claiming the size it already has changes nothing either.
        me.call(Op::Resize { session: id, cols: 41, rows: 17 }).unwrap();
        assert_eq!(me.call(Op::Claim { session: id, cols: 41, rows: 17 }).unwrap(), Reply::Claimed { changed: false });
        assert!(local.claim(id, 90, 25).unwrap());
        // A size no screen has stops at what the daemon allows.
        me.call(Op::Resize { session: id, cols: u16::MAX, rows: u16::MAX }).unwrap();
        assert_eq!(size(), (1000, 500));
        me.call(Op::Resize { session: id, cols: 0, rows: 0 }).unwrap();
        assert_eq!(size(), (2, 1));
        assert!(local.claim(id, 90, 25).unwrap());

        // This computer sees who's connected, and can cut them off.
        let viewers = local.viewers().unwrap();
        let logins: Vec<&str> = viewers.iter().map(|viewer| viewer.login.as_str()).collect();
        assert!(logins.contains(&"me@example.com") && logins.contains(&"friend@example.com"), "viewers: {logins:?}");
        local.disconnect_remote().unwrap();
        assert!(seen_by_me.wait_for(|seen| seen.exit.lock().unwrap().is_some()), "the phone was told it's gone");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let left = local.viewers().unwrap();
            if left.is_empty() {
                break;
            }
            assert!(Instant::now() < deadline, "nobody connected any more: {left:?}");
            std::thread::sleep(Duration::from_millis(50));
        }

        // Turned off: the tailnet listener goes.
        remote::write_config(&dir, &RemoteConfig { enabled: false, ..config }).unwrap();
        local.reload_remote().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while TcpStream::connect_timeout(&remote_addr, Duration::from_millis(200)).is_ok() {
            assert!(Instant::now() < deadline, "the listener stopped");
            std::thread::sleep(Duration::from_millis(100));
        }

        local.kill(id).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wrong_token_is_turned_away() {
        let port = crate::mux::server::start_for_test("right").unwrap();
        assert!(client(port, "wrong").list().is_err());
    }

    /// One of the app's own consoles, lent to the daemon: someone else sees
    /// what it printed before and after, and what they type and the size
    /// they ask for reach the app; it's gone when the app is.
    #[test]
    fn the_apps_own_console_is_shown_through_the_daemon() {
        let port = crate::mux::server::start_for_test("tok").unwrap();
        let app = client(port, "tok");
        let asked = Arc::new(Mutex::new(Vec::<String>::new()));
        let heard = Arc::clone(&asked);
        app.on_host(move |event| {
            heard.lock().unwrap().push(match event {
                HostEvent::Input { bytes, .. } => format!("input {}", String::from_utf8_lossy(&bytes)),
                HostEvent::Resize { cols, rows, .. } => format!("resize {cols}x{rows}"),
                HostEvent::PromptRedraw { .. } => "redraw".to_string(),
            })
        });
        let generation = app.connected().unwrap();
        let id = Uuid::new_v4();
        let console = HostedConsole { path: "pwsh.exe".into(), cwd: None, started_at: 1, cols: 80, rows: 24, pid: Some(7) };
        app.post(generation, Outgoing::Op(Op::Host { session: id, console }));
        app.post(generation, Outgoing::Output { session: id, bytes: b"before\r\n".to_vec() });

        let viewer = client(port, "tok");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !viewer.list().unwrap().iter().any(|info| info.id == id) {
            assert!(Instant::now() < deadline, "never listed");
            std::thread::sleep(Duration::from_millis(20));
        }
        let listed = viewer.list().unwrap().into_iter().find(|info| info.id == id).unwrap();
        assert!(listed.hosted);
        assert_eq!(listed.pid, Some(7));

        let seen = Arc::new(Seen::default());
        let sink = terminal(&viewer, &seen, &Arc::new(Mutex::new(Some(id))));
        viewer.attach("main", id, sink).unwrap();
        app.post(generation, Outgoing::Output { session: id, bytes: b"after\r\n".to_vec() });
        assert!(seen.wait_for(|seen| seen.text().contains("before") && seen.text().contains("after")), "{:?}", seen.text());

        viewer.write(id, b"ls\r").unwrap();
        viewer.call(Op::Resize { session: id, cols: 50, rows: 40 }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while asked.lock().unwrap().len() < 2 {
            assert!(Instant::now() < deadline, "the app heard {:?}", asked.lock().unwrap());
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(*asked.lock().unwrap(), ["input ls\r", "resize 50x40"]);
        // Closed from the app, not from elsewhere.
        assert!(viewer.call(Op::Kill { session: id }).is_err());

        // What the status bar shows: the folder its shell says, the agent
        // the app says.
        let agent = AgentInfo {
            tool: "Claude Code".into(),
            status: "awaiting".into(),
            plan_windows: vec![PlanUse { window_minutes: Some(300), percent: 42.0 }],
            context_percent: Some(37.0),
            cost_usd: None,
        };
        app.post(generation, Outgoing::Output { session: id, bytes: b"\x1b]7;file:///C:/work/repo\x07> ".to_vec() });
        app.post(generation, Outgoing::Op(Op::Agent { session: id, agent: Some(agent.clone()) }));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let info = viewer.list().unwrap().into_iter().find(|info| info.id == id).unwrap();
            if info.agent.as_ref() == Some(&agent) && info.cwd.as_deref() == Some("C:\\work\\repo") {
                break;
            }
            assert!(Instant::now() < deadline, "never told: {info:?}");
            std::thread::sleep(Duration::from_millis(20));
        }

        drop(app);
        assert!(seen.wait_for(|seen| seen.exit.lock().unwrap().is_some()), "the viewer was never told it's gone");
        assert!(!viewer.list().unwrap().iter().any(|info| info.id == id));
    }

    /// A daemon that lets clients in and then never answers — the way a
    /// hung one looks: the app hears of it, stops waiting long on it, and
    /// hears again once it answers.
    #[test]
    fn a_daemon_that_stops_answering_is_noticed() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let answer = Arc::new(AtomicBool::new(false));
        let answers = Arc::clone(&answer);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let answers = Arc::clone(&answers);
                std::thread::spawn(move || {
                    let mut stream = stream;
                    let welcome = ServerMessage::Welcome { protocol: PROTOCOL, version: "test".into(), pid: 1, access: None };
                    let _ = read_frame::<ClientMessage>(&mut stream);
                    let _ = write_frame(&mut stream, &Frame::Message(welcome));
                    while let Ok(Some(Frame::Message(ClientMessage::Request { id, .. }))) =
                        read_frame::<ClientMessage>(&mut stream)
                    {
                        if answers.load(Ordering::SeqCst) {
                            let reply = ServerMessage::Reply { id, result: Ok(Reply::Sessions { sessions: Vec::new() }) };
                            let _ = write_frame(&mut stream, &Frame::Message(reply));
                        }
                    }
                });
            }
        });
        let client = client(port, "any");
        let told = Arc::new(Mutex::new(Vec::new()));
        let heard = Arc::clone(&told);
        client.on_health(move |answering| heard.lock().unwrap().push(answering));

        let started = Instant::now();
        assert!(client.list().is_err());
        assert!(started.elapsed() < LOCAL_REPLY_TIMEOUT + Duration::from_secs(2));
        assert!(!client.is_answering());
        assert_eq!(*told.lock().unwrap(), [false]);

        // Not the whole wait again.
        let started = Instant::now();
        assert!(client.list().is_err());
        assert!(started.elapsed() < NOT_ANSWERING_TIMEOUT + Duration::from_secs(1));

        answer.store(true, Ordering::SeqCst);
        assert!(client.list().is_ok());
        assert!(client.is_answering());
        assert_eq!(*told.lock().unwrap(), [false, true]);
    }
}

