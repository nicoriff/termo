//! The sessions daemon itself: `termo --mux` (see mux/mod.rs).
//!
//! It owns the consoles. Each keeps what it printed lately, so a client that
//! attaches — the app opening again, another window — gets the screen back,
//! and forwards everything new to the clients attached to it. Threads, like
//! the rest of the console code: one per connection to read from it, one to
//! write to it, and the console's own (see PtySession).

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use uuid::Uuid;

use super::agents::{self as remote_agents, HostLink, Need};
use super::protocol::{
    read_frame, read_frame_within, write_frame, AgentInfo, ClientMessage, Frame, Op, OpenChoice, Reply, ServerMessage, SessionInfo,
    PROTOCOL,
};
use super::remote::{self, Access, RemoteConfig, Viewer};
use super::screen::Screen;
use super::status;
use super::tailscale::{self, Identity};
use super::{web, DaemonInfo};
use crate::commands::folder_commands;
use crate::pty::{contains_cursor_report, ConsoleEvent, EventSink, PtySession, SpawnSpec};

/// How much of a console's latest output it keeps as is, for a client that
/// lost its connection to pick up where it was. One that attaches anew gets
/// the screen instead (see screen.rs).
const HISTORY_BYTES: usize = 1024 * 1024;
/// Frames waiting to go out to one client. A client this far behind isn't
/// reading: it is dropped rather than letting the daemon's memory grow.
const CLIENT_QUEUE: usize = 4096;
/// How long the daemon stays up with no consoles and no one connected.
const IDLE_EXIT: Duration = Duration::from_secs(5 * 60);
/// What ConPTY asks before it lets the shell start (see PtySession::write);
/// the daemon answers it when no terminal is attached to.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";

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
    log_panics(&dir);
    let Ok(listener) = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))) else {
        return;
    };
    let Ok(port) = listener.local_addr().map(|addr| addr.port()) else {
        return;
    };
    let daemon = Daemon::new(
        super::new_token(),
        Box::new(tailscale::whois),
        Box::new(|| tailscale::status().filter(tailscale::Status::running).and_then(|status| status.me.ip).map(IpAddr::V4)),
    );
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
    {
        let daemon = Arc::clone(&daemon);
        std::thread::spawn(move || remote_supervisor(&daemon));
    }
    serve(&daemon, listener);
    super::remove_info(&dir, info.pid);
}

/// Accepts this computer's own connections (the app's) until the listener
/// fails.
fn serve(daemon: &Arc<Daemon>, listener: TcpListener) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let daemon = Arc::clone(daemon);
        std::thread::spawn(move || daemon.handle_connection(stream, Role::Local));
    }
}

/// Keeps the tailnet listener as the remote access settings say: up on this
/// computer's Tailscale address when they're on, down otherwise; and ends
/// the connections of whoever is no longer allowed. Looks again every few
/// seconds — Tailscale can come up, or its address change, at any time —
/// and at once when the app says the settings changed.
fn remote_supervisor(daemon: &Arc<Daemon>) {
    loop {
        daemon.apply_remote_config();
        let (flag, wake) = &daemon.remote.wake;
        let mut woken = flag.locked();
        if !*woken {
            woken = wake.wait_timeout(woken, Duration::from_secs(5)).unwrap_or_else(std::sync::PoisonError::into_inner).0;
        }
        *woken = false;
    }
}

/// Accepts connections from the tailnet until told to stop.
fn accept_remote(daemon: &Arc<Daemon>, listener: &TcpListener, stop: &AtomicBool) {
    let _ = listener.set_nonblocking(true);
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                let _ = stream.set_nonblocking(false);
                let daemon = Arc::clone(daemon);
                std::thread::spawn(move || daemon.admit(stream, peer));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
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

/// The daemon has no console to print to: a panic (what went wrong, and
/// where) goes to `daemon.log` in its folder, the last few kept.
fn log_panics(dir: &std::path::Path) {
    const MAX_LOG_BYTES: u64 = 256 * 1024;
    let path = dir.join("daemon.log");
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES) {
            let _ = std::fs::remove_file(&path);
        }
        let thread = std::thread::current();
        let line = format!("{}\t{}\t{info}\n", now_seconds(), thread.name().unwrap_or("?"));
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = file.write_all(line.as_bytes());
        }
        previous(info);
    }));
}

/// A lock taken even after a thread panicked holding it. What it guards is
/// still usable (a console's screen copy starts over if it was mid-update,
/// see screen.rs); refusing it instead would take down every connection
/// that touches it, one after another, and leave the app waiting.
trait Locked<T> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Whether the shell `pid` sits at its prompt: nothing running under it.
fn shell_is_idle(pid: u32) -> bool {
    static PROCESSES: std::sync::LazyLock<crate::pty::memory::MemorySampler> =
        std::sync::LazyLock::new(Default::default);
    PROCESSES.busy_roots(&[pid]).is_empty()
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
    remote: RemoteAccess,
    /// The app's connection, which opens what the tailnet asks for (see
    /// Op::Opener), while it's connected.
    opener: Mutex<Option<Opener>>,
    /// The new consoles asked of it and not answered yet: whose connection
    /// was asked, and where its answer goes.
    opening: Mutex<HashMap<u64, (u64, SyncSender<Result<Uuid, String>>)>>,
    next_open: AtomicU64,
    /// The web clients watching the agents (see agents.rs), by client id.
    agent_watchers: Mutex<HashMap<u64, Client>>,
    /// The agents' calls asked of the app and not answered yet, as
    /// `opening` is for consoles.
    agents_asking: Mutex<HashMap<u64, (u64, SyncSender<Result<Value, String>>)>>,
    next_agents: AtomicU64,
    /// The agents' own process, reached while the app isn't connected.
    host_link: OnceLock<Arc<HostLink>>,
}

struct Opener {
    client: Client,
    choices: Vec<OpenChoice>,
    recent: Vec<String>,
    /// It answers for the agents too (see Op::Opener).
    agents: bool,
}

/// How long a call about the agents waits on the app.
const AGENTS_WAIT: Duration = Duration::from_secs(30);
/// The agents' calls asked of the app at once, at most.
const MAX_AGENTS_ASKING: usize = 16;

/// How long a new console asked from the tailnet may take to show up: the
/// app opening its tab, and lending it to the daemon (see pty/sharing.rs).
const OPEN_WAIT: Duration = Duration::from_secs(12);
/// New consoles asked for at once, at most.
const MAX_OPENING: usize = 4;
/// What the app may offer to open, at most.
const MAX_CHOICES: usize = 64;
/// The folders of one folder sent to a folder browser, at most.
const MAX_FOLDERS: usize = 5000;

/// Whether a console may be opened in `cwd`: a folder of this computer's
/// that is there. Not a network share: just looking would have Windows
/// reach that server with the user's credentials.
fn usable_folder(cwd: &str) -> bool {
    let path = std::path::Path::new(cwd);
    cwd.len() <= 4096 && !status::is_network_path(cwd) && path.is_absolute() && path.is_dir()
}

pub type Whois = Box<dyn Fn(SocketAddr) -> Option<Identity> + Send + Sync>;
pub type Address = Box<dyn Fn() -> Option<IpAddr> + Send + Sync>;

/// Remote access's moving parts (see remote.rs).
struct RemoteAccess {
    /// Who is behind a connection from the tailnet: Tailscale's answer.
    whois: Whois,
    /// This computer's address on the tailnet, if it has one now.
    address: Address,
    config: Mutex<RemoteConfig>,
    /// The listener: the address it was asked for, the one it got, and what
    /// stops it.
    listener: Mutex<Option<(SocketAddr, SocketAddr, Arc<AtomicBool>)>>,
    /// The connections from the tailnet, by client id.
    viewers: Mutex<HashMap<u64, RemoteViewer>>,
    wake: (Mutex<bool>, Condvar),
    /// What Tailscale said of each address lately (see identify).
    known: Mutex<HashMap<IpAddr, (Instant, Option<Identity>)>>,
    /// Connections being identified right now.
    admitting: AtomicUsize,
}

/// How long Tailscale's word on an address is taken as still good. Not at
/// all in tests, which pretend to be several people from one address.
const WHOIS_FOR: Duration = if cfg!(test) { Duration::ZERO } else { Duration::from_secs(30) };
/// Connections from the tailnet being identified at once, at most; more are
/// dropped.
const MAX_ADMITTING: usize = 16;

struct RemoteViewer {
    identity: Identity,
    access: Access,
    since: i64,
    kick: Arc<Kick>,
    sessions: HashSet<Uuid>,
}

/// Who a connection is, and so what it may do.
#[derive(Clone)]
pub enum Role {
    /// The app on this computer, with the token from the info file.
    Local,
    /// Someone on the tailnet, as Tailscale vouched for them.
    Remote { identity: Identity, access: Access },
}

impl Role {
    fn may(&self, op: &Op) -> bool {
        match self {
            Role::Local => true,
            Role::Remote { access, .. } => match op {
                Op::List | Op::Attach { .. } | Op::Detach { .. } => true,
                Op::Resize { .. }
                | Op::Claim { .. }
                | Op::PromptRedraw { .. }
                | Op::Kill { .. }
                | Op::OpenChoices
                | Op::Open { .. }
                | Op::FolderRoots
                | Op::ListFolders { .. }
                | Op::CreateFolder { .. } => *access == Access::Full,
                // What each call takes is the call's (see Daemon::agents).
                Op::Agents { .. } => true,
                // Starting consoles other than through the app (see
                // Op::Open), and everything about the daemon itself, is
                // this computer's alone.
                _ => false,
            },
        }
    }

    fn may_type(&self) -> bool {
        match self {
            Role::Local => true,
            Role::Remote { access, .. } => *access == Access::Full,
        }
    }
}

/// The longest hello a connection may send (see read_frame_within).
const HELLO_MAX: usize = 64 * 1024;

/// The biggest a console is made by whoever sizes it: its copy of the
/// screen holds every cell, so a size from a phone (or a client gone
/// wrong) of 65535×65535 would take the daemon down with every console.
const MAX_COLS: u16 = 1000;
const MAX_ROWS: u16 = 500;

fn bounded(cols: u16, rows: u16) -> (u16, u16) {
    (cols.clamp(2, MAX_COLS), rows.clamp(1, MAX_ROWS))
}

/// A console the daemon runs.
struct Live {
    id: Uuid,
    path: String,
    cwd: Option<String>,
    started_at: i64,
    pty: OnceLock<PtySession>,
    /// For one of the app's own consoles (see Op::Host), instead of a pty:
    /// the app's connection, where what's typed and resizes go, and the
    /// process id it gave.
    host: Option<(Client, Option<u32>)>,
    state: Mutex<State>,
}

struct State {
    /// The connections showing this console now.
    clients: Vec<Client>,
    history: History,
    screen: Screen,
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
    /// The client that typed last. With more than one terminal attached,
    /// each would answer what the program asks its terminal; only this
    /// one's answers go through (see accepts_input).
    typist: Option<u64>,
    /// Someone on the tailnet resized it to their screen (a phone): the
    /// size it had before, given back when they leave — unless this
    /// computer sized it meanwhile, which makes that its size.
    borrowed: Option<Borrowed>,
    /// Where its shell last said it is (see status::last_cwd).
    cwd_now: Option<String>,
    /// What runs in front now.
    foreground: Option<String>,
    /// What the app says of the agent in it (see Op::Agent).
    agent: Option<AgentInfo>,
}

struct Borrowed {
    by: u64,
    cols: u16,
    rows: u16,
}

impl State {
    /// Whether `client`'s input reaches the console: everything typed does,
    /// and makes it the typist; a terminal's answer only from the typist
    /// (or from anyone while no one has typed).
    fn accepts_input(&mut self, client: u64, bytes: &[u8]) -> bool {
        if is_terminal_reply(bytes) {
            return self.typist.is_none_or(|typist| typist == client);
        }
        self.typist = Some(client);
        true
    }
}

#[derive(Clone)]
struct Client {
    id: u64,
    /// This computer's own Termo, not someone on the tailnet.
    local: bool,
    tx: SyncSender<Frame<ServerMessage>>,
    /// Ends the connection: a client that stopped reading, one no longer
    /// allowed in.
    kick: Arc<Kick>,
}

impl Client {
    /// Queues a frame without waiting; a client whose queue is full is cut
    /// off (its connection ends, and it detaches from everything).
    fn offer(&self, frame: Frame<ServerMessage>) -> bool {
        match self.tx.try_send(frame) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.kick.kick();
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }
}

/// What ends a connection from this side, whatever the other side does:
/// shutting the socket down isn't enough on Windows, where it doesn't wake
/// a read already waiting — the reader (see KickableReader) checks this.
struct Kick {
    stream: TcpStream,
    kicked: AtomicBool,
}

impl Kick {
    fn kick(&self) {
        self.kicked.store(true, Ordering::SeqCst);
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

/// Reads a connection a little at a time, so a kick is noticed even while
/// nothing arrives: a read that times out with nothing read is retried —
/// no data is lost — unless the connection was kicked.
struct KickableReader {
    stream: TcpStream,
    kick: Arc<Kick>,
}

impl std::io::Read for KickableReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.kick.kicked.load(Ordering::SeqCst) {
                return Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "kicked"));
            }
            match self.stream.read(buf) {
                Err(err) if matches!(err.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                other => return other,
            }
        }
    }
}

impl Daemon {
    /// Its folder (info file, remote access settings), once it runs.
    pub(super) fn folder(&self) -> Option<&std::path::Path> {
        self.dir.get().map(PathBuf::as_path)
    }

    pub fn new(token: String, whois: Whois, address: Address) -> Arc<Self> {
        Arc::new(Self {
            token,
            dir: OnceLock::new(),
            sessions: Mutex::new(HashMap::new()),
            connections: AtomicUsize::new(0),
            next_client: AtomicU64::new(1),
            busy_at: Mutex::new(Instant::now()),
            remote: RemoteAccess {
                whois,
                address,
                config: Mutex::new(RemoteConfig::default()),
                listener: Mutex::new(None),
                viewers: Mutex::new(HashMap::new()),
                wake: (Mutex::new(false), Condvar::new()),
                known: Mutex::new(HashMap::new()),
                admitting: AtomicUsize::new(0),
            },
            opener: Mutex::new(None),
            opening: Mutex::new(HashMap::new()),
            next_open: AtomicU64::new(1),
            agent_watchers: Mutex::new(HashMap::new()),
            agents_asking: Mutex::new(HashMap::new()),
            next_agents: AtomicU64::new(1),
            host_link: OnceLock::new(),
        })
    }

    /// Something about the user's agents, asked from the web client (see
    /// agents.rs). Errors are words it translates, or the agents' own.
    fn agents(self: &Arc<Self>, client: &Client, role: &Role, call: &str, args: Value) -> Result<Reply, String> {
        use crate::agents::store;
        use remote_agents::{AgentRef, Answer, EnabledArgs, EngineArgs, IdArgs, RunRef, SaveArgs, StartArgs};

        let need = remote_agents::need(call).ok_or("unknown-call")?;
        let edit_allowed = self.remote.config.locked().agents_edit;
        let full = match role {
            Role::Local => true,
            Role::Remote { access, .. } => *access == Access::Full,
        };
        match need {
            Need::Look => {}
            Need::Run | Need::Edit if !full => return Err("not-allowed".into()),
            Need::Edit if !edit_allowed && !matches!(role, Role::Local) => return Err("edit-off".into()),
            _ => {}
        }
        // What the daemon says itself (a save that failed) in the language
        // the app's windows speak (see window_commands::set_ui_language).
        if let Some(language) = self.folder().and_then(|dir| std::fs::read_to_string(dir.join(super::LANGUAGE_FILE)).ok()) {
            crate::i18n::set(if language.trim() == "en" { crate::i18n::Lang::En } else { crate::i18n::Lang::Es });
        }
        let root = crate::agents::host::agents_root().ok_or("no-agents")?;
        let spec_of = |id: &str| store::get_in(&root, id).ok_or_else(|| "not-found".to_string());
        let value = match call {
            "overview" => self.agents_overview(&root, full, edit_allowed),
            "watch" => {
                self.watch_agents(client);
                Value::Null
            }
            "agent" => {
                let IdArgs { id } = remote_agents::args(args)?;
                let spec = spec_of(&id)?;
                json!({ "form": remote_agents::form_of(&spec), "advanced": remote_agents::advanced(&spec) })
            }
            // Only of an agent that's there: reading a history makes its
            // folder, and an id from the tailnet mustn't make folders.
            "runs" => {
                let AgentRef { agent_id } = remote_agents::args(args)?;
                spec_of(&agent_id)?;
                let mut runs = store::list_runs_in(&root, &agent_id);
                runs.truncate(remote_agents::MAX_RUNS);
                json!(runs)
            }
            "run_log" => {
                let RunRef { agent_id, run_id } = remote_agents::args(args)?;
                spec_of(&agent_id)?;
                let events = store::read_log_in(&root, &agent_id, &run_id);
                let skip = events.len().saturating_sub(remote_agents::MAX_LOG_EVENTS);
                json!({ "events": &events[skip..], "skipped": skip })
            }
            "models" => {
                let EngineArgs { engine } = remote_agents::args(args)?;
                json!(crate::agents::models::available(engine))
            }
            "start" => {
                let StartArgs { agent_id, context } = remote_agents::args(args)?;
                let spec = spec_of(&agent_id)?;
                self.agents_runtime("agent_start", remote_agents::start_args(&spec, context))?
            }
            "stop" => {
                let AgentRef { agent_id } = remote_agents::args(args)?;
                self.agents_runtime("agent_stop", json!({ "agent_id": agent_id }))?
            }
            "answer" => {
                let Answer { question_id, answer } = remote_agents::args(args)?;
                self.agents_runtime("agent_answer_question", json!({ "question_id": question_id, "answer": answer }))?
            }
            "save" => {
                let SaveArgs { form } = remote_agents::args(args)?;
                let base = if form.id.is_empty() { None } else { Some(spec_of(&form.id)?) };
                let had_git = base.as_ref().is_some_and(|spec| spec.triggers.git.is_some());
                let spec = remote_agents::apply(form, base, usable_folder)?;
                let id = spec.id.clone();
                self.save_agent(&root, had_git, spec)?;
                self.agents_changed();
                json!({ "id": id })
            }
            "set_enabled" => {
                let EnabledArgs { id, enabled } = remote_agents::args(args)?;
                let mut spec = spec_of(&id)?;
                spec.enabled = enabled;
                let had_git = spec.triggers.git.is_some();
                self.save_agent(&root, had_git, spec)?;
                self.agents_changed();
                Value::Null
            }
            "delete" => {
                let IdArgs { id } = remote_agents::args(args)?;
                spec_of(&id)?;
                // Where its runs are, so one going is stopped. Deleted here
                // only when nobody runs the agents for sure, asked before:
                // a call that fails on the way is an error, not "nobody".
                if self.agents_runtime_kind().is_some() {
                    self.agents_runtime("agents_delete", json!({ "id": id }))?;
                } else {
                    let idle = crate::agents::runner::Runner::default();
                    crate::commands::agent_commands::delete_agent(&root, &idle, &id)?;
                }
                self.agents_changed();
                Value::Null
            }
            _ => return Err("unknown-call".into()),
        };
        Ok(Reply::Agents { value })
    }

    /// The agents for the web client's list, with what only whoever runs
    /// them knows (what runs now, what waits on an answer) when someone does.
    fn agents_overview(self: &Arc<Self>, root: &std::path::Path, full: bool, edit_allowed: bool) -> Value {
        let config = crate::agents::host::config_path().map(|path| crate::config::store::load_from(&path)).unwrap_or_default();
        let runtime = if !config.ai_mode_enabled { None } else { self.agents_runtime_kind() };
        let (running, questions) = match runtime {
            Some(_) => (
                self.agents_runtime("agents_running", json!({})).unwrap_or_else(|_| json!([])),
                self.agents_runtime("agent_questions", json!({})).unwrap_or_else(|_| json!([])),
            ),
            None => (json!([]), json!([])),
        };
        // The agent CLIs installed here, as the app's new-tab menu offers
        // them; unknown while it isn't connected.
        let engines: Option<Vec<String>> = self.opener.locked().as_ref().map(|opener| {
            opener
                .choices
                .iter()
                .filter(|choice| choice.kind == "agent")
                .filter_map(|choice| choice.id.strip_prefix("agent:").map(str::to_string))
                .collect()
        });
        json!({
            "agents": remote_agents::summaries(root),
            "running": running,
            "questions": questions,
            "runtime": runtime,
            "aiMode": config.ai_mode_enabled,
            "canRun": full && runtime.is_some(),
            "canEdit": full && edit_allowed,
            "editOff": full && !edit_allowed,
            "engines": engines,
        })
    }

    /// Who runs the agents now, as far as the tailnet can reach them: the
    /// app ("app"), their own process ("host"), or nobody.
    fn agents_runtime_kind(self: &Arc<Self>) -> Option<&'static str> {
        if self.opener.locked().as_ref().is_some_and(|opener| opener.agents) {
            return Some("app");
        }
        self.host_link().reach().then_some("host")
    }

    fn host_link(self: &Arc<Self>) -> &Arc<HostLink> {
        self.host_link.get_or_init(|| {
            let daemon = Arc::downgrade(self);
            HostLink::new(move |name, payload| {
                // While the app is connected, it's the one that passes on
                // what the agents say (Op::AgentsEvent): not twice.
                if let Some(daemon) = daemon.upgrade() {
                    if !daemon.opener.locked().as_ref().is_some_and(|opener| opener.agents) {
                        daemon.pass_on_agent_event(name, payload);
                    }
                }
            })
        })
    }

    /// Saves an agent changed from the tailnet. One with a git trigger, now
    /// or before, is saved by the app: saving rewrites its git hook with the
    /// running program's path, which here would be the daemon's own copy —
    /// one an update may later remove, and the hook then fails silently.
    fn save_agent(self: &Arc<Self>, root: &std::path::Path, had_git: bool, spec: crate::agents::spec::AgentSpec) -> Result<(), String> {
        if had_git || spec.triggers.git.is_some() {
            let app = self.app_for_agents().ok_or("needs-app")?;
            self.ask_app(&app, "agent_save", json!({ "spec": spec }))?;
            return Ok(());
        }
        crate::commands::agent_commands::save_agent(root, spec).map(|_| ())
    }

    /// The app's connection, when it answers for the agents.
    fn app_for_agents(&self) -> Option<Client> {
        self.opener.locked().as_ref().filter(|opener| opener.agents).map(|opener| opener.client.clone())
    }

    /// One of ops::call's, run by whoever runs the agents: the app while
    /// it's connected, else their own process.
    fn agents_runtime(self: &Arc<Self>, name: &str, args: Value) -> Result<Value, String> {
        if !remote_agents::RUNTIME_CALLS.contains(&name) {
            return Err("unknown-call".into());
        }
        match self.app_for_agents() {
            Some(app) => self.ask_app(&app, name, args),
            None => self.host_link().call(name, args),
        }
    }

    /// Asks the app one of the calls it answers for the agents (see
    /// agents::RUNTIME_CALLS and APP_CALLS), and waits for the answer.
    fn ask_app(&self, app: &Client, name: &str, args: Value) -> Result<Value, String> {
        let request = self.next_agents.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = sync_channel(1);
        {
            let mut asking = self.agents_asking.locked();
            if asking.len() >= MAX_AGENTS_ASKING {
                return Err("busy".into());
            }
            asking.insert(request, (app.id, tx));
        }
        let asked = app.offer(Frame::Message(ServerMessage::AgentsCall { request, name: name.to_string(), args }));
        let answer = if asked { rx.recv_timeout(AGENTS_WAIT) } else { Err(RecvTimeoutError::Disconnected) };
        self.agents_asking.locked().remove(&request);
        match answer {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err("timeout".into()),
            Err(RecvTimeoutError::Disconnected) => Err("no-runtime".into()),
        }
    }

    /// From now on `client` hears what the agents say. The app is asked to
    /// pass it on once someone watches; their own process, reached now,
    /// tells it anyway.
    fn watch_agents(self: &Arc<Self>, client: &Client) {
        let first = {
            let mut watchers = self.agent_watchers.locked();
            let first = watchers.is_empty();
            watchers.insert(client.id, client.clone());
            first
        };
        if first {
            self.tell_app_watching(true);
        }
        if !self.opener.locked().as_ref().is_some_and(|opener| opener.agents) {
            let _ = self.host_link().reach();
        }
    }

    fn unwatch_agents(&self, client: u64) {
        let last = {
            let mut watchers = self.agent_watchers.locked();
            watchers.remove(&client).is_some() && watchers.is_empty()
        };
        if last {
            self.tell_app_watching(false);
        }
    }

    fn tell_app_watching(&self, on: bool) {
        let app = self.opener.locked().as_ref().filter(|opener| opener.agents).map(|opener| opener.client.clone());
        if let Some(app) = app {
            app.offer(Frame::Message(ServerMessage::AgentsWatch { on }));
        }
    }

    fn pass_on_agent_event(&self, name: String, payload: Value) {
        if !remote_agents::worth_passing_on(&name) {
            return;
        }
        let message = ServerMessage::AgentEvent { name, payload };
        let emptied = {
            let mut watchers = self.agent_watchers.locked();
            let before = watchers.len();
            watchers.retain(|_, watcher| watcher.offer(Frame::Message(message.clone())));
            before > 0 && watchers.is_empty()
        };
        // The last one went (it stopped reading): the app stops too.
        if emptied {
            self.tell_app_watching(false);
        }
    }

    /// The app's agents lists are read again: one changed from here.
    fn agents_changed(&self) {
        let app = self.opener.locked().as_ref().filter(|opener| opener.agents).map(|opener| opener.client.clone());
        if let Some(app) = app {
            app.offer(Frame::Message(ServerMessage::AgentsChanged));
        }
    }

    /// A new console from the tailnet: asked of the app (see Op::Opener),
    /// which opens it as one of its tabs; answered once the daemon has it to
    /// show. Errors are words the web client translates.
    fn open(&self, choice: &str, cwd: Option<String>) -> Result<Reply, String> {
        let cwd = cwd.filter(|cwd| !cwd.trim().is_empty());
        if cwd.as_deref().is_some_and(|cwd| !usable_folder(cwd)) {
            return Err("bad-folder".into());
        }
        let opener = {
            let opener = self.opener.locked();
            let Some(opener) = opener.as_ref() else {
                return Err("no-app".into());
            };
            if !opener.choices.iter().any(|offered| offered.id == choice) {
                return Err("unknown-choice".into());
            }
            opener.client.clone()
        };
        let request = self.next_open.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = sync_channel(1);
        {
            let mut opening = self.opening.locked();
            if opening.len() >= MAX_OPENING {
                return Err("busy".into());
            }
            opening.insert(request, (opener.id, tx));
        }
        let deadline = Instant::now() + OPEN_WAIT;
        let asked = opener.offer(Frame::Message(ServerMessage::Open { request, choice: choice.to_string(), cwd }));
        let answer = if asked { rx.recv_timeout(OPEN_WAIT) } else { Err(RecvTimeoutError::Disconnected) };
        self.opening.locked().remove(&request);
        let session = match answer {
            Ok(result) => result?,
            Err(RecvTimeoutError::Timeout) => return Err("timeout".into()),
            // The app went meanwhile.
            Err(RecvTimeoutError::Disconnected) => return Err("no-app".into()),
        };
        // One the app runs itself is lent to the daemon a moment later.
        while !self.sessions.locked().contains_key(&session) {
            if Instant::now() >= deadline {
                return Err("timeout".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(Reply::Opened { session })
    }

    /// Applies the remote access settings (see remote_supervisor).
    fn apply_remote_config(self: &Arc<Self>) {
        let Some(dir) = self.dir.get() else {
            return;
        };
        let config = remote::read_config(dir);
        // Whoever is no longer allowed — or now allowed less — is cut off.
        for viewer in self.remote.viewers.locked().values() {
            if !config.enabled || config.access_for(&viewer.identity) != Some(viewer.access) {
                viewer.kick.kick();
            }
        }
        *self.remote.config.locked() = config.clone();
        let wanted = config
            .enabled
            .then(|| (self.remote.address)())
            .flatten()
            .map(|ip| SocketAddr::new(ip, config.port));
        let mut listener = self.remote.listener.locked();
        if listener.as_ref().map(|(asked, _, _)| *asked) == wanted {
            return;
        }
        if let Some((_, _, stop)) = listener.take() {
            stop.store(true, Ordering::SeqCst);
            self.disconnect_remote();
        }
        let Some(asked) = wanted else {
            return;
        };
        let Ok(tcp) = TcpListener::bind(asked) else {
            return;
        };
        let bound = tcp.local_addr().unwrap_or(asked);
        let stop = Arc::new(AtomicBool::new(false));
        let daemon = Arc::clone(self);
        let stop_for_thread = Arc::clone(&stop);
        std::thread::spawn(move || accept_remote(&daemon, &tcp, &stop_for_thread));
        *listener = Some((asked, bound, stop));
    }

    /// A connection from the tailnet: in, if Tailscale says who it is and
    /// they're on the list; turned away and left pending otherwise.
    fn admit(self: Arc<Self>, stream: TcpStream, peer: SocketAddr) {
        // Each new connection asks Tailscale who it is: someone connecting
        // over and over mustn't pile those up.
        if self.remote.admitting.fetch_add(1, Ordering::SeqCst) >= MAX_ADMITTING {
            self.remote.admitting.fetch_sub(1, Ordering::SeqCst);
            return;
        }
        let identity = self.identify(peer);
        self.remote.admitting.fetch_sub(1, Ordering::SeqCst);
        let dir = self.dir.get().cloned();
        let now = now_seconds();
        // A browser (the web client, see web.rs) or the daemon's own protocol.
        let browser = web::is_http(&stream);
        let access = identity.as_ref().and_then(|who| self.remote.config.locked().access_for(who));
        if let Some(dir) = &dir {
            let outcome = match (&identity, access) {
                (None, _) => "unknown",
                (Some(_), None) => "pending",
                (Some(_), Some(Access::Full)) => "full",
                (Some(_), Some(Access::ReadOnly)) => "read-only",
            };
            remote::log(dir, now, peer, identity.as_ref(), outcome);
        }
        match (identity, access) {
            (Some(identity), Some(access)) if browser => {
                let own = web::own_names((self.remote.address)());
                web::serve(self, stream, &own, Role::Remote { identity, access });
            }
            (Some(identity), Some(access)) => self.handle_connection(stream, Role::Remote { identity, access }),
            (identity, _) => {
                if let (Some(identity), Some(dir)) = (&identity, &dir) {
                    remote::add_pending(dir, identity, now);
                }
                if browser {
                    web::refuse(stream, identity.is_some());
                    return;
                }
                let reason = if identity.is_some() { "waiting for approval" } else { "not on the tailnet" };
                let mut writer = stream;
                let _ = writer.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = read_frame_within::<ClientMessage>(&mut writer, HELLO_MAX);
                let _ = write_frame(&mut writer, &Frame::Message(ServerMessage::Rejected { reason: reason.into() }));
            }
        }
    }

    /// Who `peer` is, as Tailscale said a moment ago or says now. An
    /// address stays the same computer, and the same someone, for a while.
    fn identify(&self, peer: SocketAddr) -> Option<Identity> {
        let ip = peer.ip();
        {
            let mut known = self.remote.known.locked();
            known.retain(|_, (at, _)| at.elapsed() < WHOIS_FOR);
            if let Some((_, who)) = known.get(&ip) {
                return who.clone();
            }
        }
        let who = (self.remote.whois)(peer);
        self.remote.known.locked().insert(ip, (Instant::now(), who.clone()));
        who
    }

    /// Ends every connection from the tailnet.
    fn disconnect_remote(&self) {
        for viewer in self.remote.viewers.locked().values() {
            viewer.kick.kick();
        }
    }

    fn viewers(&self) -> Vec<Viewer> {
        let mut viewers: Vec<Viewer> = self
            .remote
            .viewers
            .locked()
            .values()
            .map(|viewer| Viewer {
                login: viewer.identity.login.clone(),
                node: viewer.identity.node.clone(),
                access: viewer.access,
                sessions: viewer.sessions.iter().copied().collect(),
                since: viewer.since,
            })
            .collect();
        viewers.sort_by_key(|viewer| viewer.since);
        viewers
    }

    fn note_viewing(&self, client: u64, session: Uuid, viewing: bool) {
        if let Some(viewer) = self.remote.viewers.locked().get_mut(&client) {
            if viewing {
                viewer.sessions.insert(session);
            } else {
                viewer.sessions.remove(&session);
            }
        }
    }

    fn set_dir(&self, dir: PathBuf) {
        let _ = self.dir.set(dir);
    }

    fn touch(&self) {
        *self.busy_at.locked() = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        if !self.sessions.locked().is_empty() || self.connections.load(Ordering::SeqCst) > 0 {
            self.touch();
        }
        self.busy_at.locked().elapsed()
    }

    /// Ends every console and the daemon.
    fn exit(&self) -> ! {
        let sessions: Vec<Arc<Live>> = self.sessions.locked().drain().map(|(_, live)| live).collect();
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

    /// Sizes a console as `client` asked: someone on the tailnet borrows
    /// the size (see Borrowed), this computer makes it its own.
    fn resize_for(&self, client: &Client, role: &Role, session: Uuid, cols: u16, rows: u16) -> Result<(), String> {
        let (cols, rows) = bounded(cols, rows);
        let live = self.session(session)?;
        {
            let mut state = live.state.locked();
            if matches!(role, Role::Remote { .. }) {
                let before = (state.cols, state.rows);
                let borrowed = state.borrowed.get_or_insert(Borrowed { by: client.id, cols: before.0, rows: before.1 });
                borrowed.by = client.id;
            } else {
                state.borrowed = None;
            }
        }
        if live.hosted_by(client.id) {
            // The app resized its own console already.
            live.resized(cols, rows);
        } else {
            live.resize(cols, rows)?;
        }
        Ok(())
    }

    fn session(&self, id: Uuid) -> Result<Arc<Live>, String> {
        self.sessions.locked().get(&id).cloned().ok_or_else(|| format!("unknown session: {id}"))
    }

    /// One connection, from its hello to its end. A local one has to bring
    /// the token; a remote one was already vouched for (see admit).
    pub fn handle_connection(self: Arc<Self>, stream: TcpStream, role: Role) {
        let _ = stream.set_nodelay(true);
        let mut reader = stream;
        let _ = reader.set_read_timeout(Some(Duration::from_secs(5)));
        let Ok(mut writer) = reader.try_clone() else {
            return;
        };
        match read_frame_within::<ClientMessage>(&mut reader, HELLO_MAX) {
            Ok(Some(Frame::Message(ClientMessage::Hello { protocol, token, .. }))) => {
                let local = matches!(role, Role::Local);
                if local && !crate::agents::webhook::token_matches(&token, &self.token) {
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
            access: match &role {
                Role::Local => None,
                Role::Remote { access, .. } => Some(*access),
            },
        };
        if write_frame(&mut writer, &Frame::Message(welcome)).is_err() {
            return;
        }
        // Short reads, so a kick is noticed (see KickableReader).
        let _ = reader.set_read_timeout(Some(Duration::from_millis(500)));
        let Ok(kick_handle) = reader.try_clone() else {
            return;
        };
        let kick = Arc::new(Kick { stream: kick_handle, kicked: AtomicBool::new(false) });
        let mut reader = KickableReader { stream: reader, kick: Arc::clone(&kick) };
        let (tx, rx) = sync_channel::<Frame<ServerMessage>>(CLIENT_QUEUE);
        let client = Client {
            id: self.next_client.fetch_add(1, Ordering::SeqCst),
            local: matches!(role, Role::Local),
            tx,
            kick,
        };
        std::thread::spawn(move || {
            for frame in rx {
                if write_frame(&mut writer, &frame).is_err() {
                    break;
                }
            }
            let _ = writer.shutdown(std::net::Shutdown::Both);
        });

        if let Role::Remote { identity, access } = &role {
            self.remote.viewers.locked().insert(
                client.id,
                RemoteViewer {
                    identity: identity.clone(),
                    access: *access,
                    since: now_seconds(),
                    kick: Arc::clone(&client.kick),
                    sessions: HashSet::new(),
                },
            );
        }
        self.connections.fetch_add(1, Ordering::SeqCst);
        self.touch();
        let mut attached: HashSet<Uuid> = HashSet::new();
        while let Ok(Some(frame)) = read_frame::<ClientMessage>(&mut reader) {
            match frame {
                Frame::Input { session, bytes } => {
                    // Read-only: shown, never typed into.
                    if !role.may_type() || !attached.contains(&session) {
                        continue;
                    }
                    if let Ok(live) = self.session(session) {
                        // The app's own console has the app's terminal
                        // answering what the program asks it; another
                        // one's answer would be a second.
                        if live.host.is_some() && is_terminal_reply(&bytes) {
                            continue;
                        }
                        {
                            let mut state = live.state.locked();
                            if !state.accepts_input(client.id, &bytes) {
                                continue;
                            }
                            if contains_cursor_report(&bytes) {
                                state.cursor_query = false;
                            }
                        }
                        live.write(&bytes);
                    }
                }
                // What one of the app's own consoles printed (see Op::Host).
                Frame::Output { session, bytes, .. } => {
                    if let Ok(live) = self.session(session) {
                        if live.hosted_by(client.id) {
                            live.output(bytes);
                        }
                    }
                }
                // It waits on the app: the connection goes on meanwhile.
                Frame::Message(ClientMessage::Request { id, op: Op::Open { choice, cwd } })
                    if role.may(&Op::Open { choice: String::new(), cwd: None }) =>
                {
                    let daemon = Arc::clone(&self);
                    let tx = client.tx.clone();
                    std::thread::spawn(move || {
                        let result = daemon.open(&choice, cwd);
                        let _ = tx.send(Frame::Message(ServerMessage::Reply { id, result }));
                    });
                }
                // Some wait on the app or the agents' process: the same.
                Frame::Message(ClientMessage::Request { id, op: Op::Agents { call, args } }) => {
                    let (daemon, client, role) = (Arc::clone(&self), client.clone(), role.clone());
                    std::thread::spawn(move || {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| daemon.agents(&client, &role, &call, args)))
                            .unwrap_or_else(|_| Err("the sessions daemon failed at this".to_string()));
                        let _ = client.tx.send(Frame::Message(ServerMessage::Reply { id, result }));
                    });
                }
                Frame::Message(ClientMessage::Request { id, op }) => {
                    let shutdown = op == Op::Shutdown;
                    let result = if role.may(&op) {
                        // A bug in handling one request is that request's
                        // error (and a line in daemon.log), not a client
                        // left waiting for an answer that never comes.
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            self.handle(&client, &role, &mut attached, op)
                        }))
                        .unwrap_or_else(|_| Err("the sessions daemon failed at this".to_string()))
                    } else {
                        Err("not allowed".to_string())
                    };
                    // Blocking here is fine: the writer thread drains it.
                    let allowed = result.is_ok();
                    if client.tx.send(Frame::Message(ServerMessage::Reply { id, result })).is_err() {
                        break;
                    }
                    if shutdown && allowed {
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
        // The app's own consoles go with it.
        let hosted: Vec<Uuid> =
            self.sessions.locked().values().filter(|live| live.hosted_by(client.id)).map(|live| live.id).collect();
        for session in hosted {
            let _ = self.kill(session);
        }
        // Nobody opens consoles for the tailnet any more; whoever was
        // waiting on it is told now.
        {
            let mut opener = self.opener.locked();
            if opener.as_ref().is_some_and(|opener| opener.client.id == client.id) {
                *opener = None;
            }
        }
        self.opening.locked().retain(|_, (asked, _)| *asked != client.id);
        self.agents_asking.locked().retain(|_, (asked, _)| *asked != client.id);
        self.unwatch_agents(client.id);
        self.remote.viewers.locked().remove(&client.id);
        self.connections.fetch_sub(1, Ordering::SeqCst);
        self.touch();
    }

    fn handle(self: &Arc<Self>, client: &Client, role: &Role, attached: &mut HashSet<Uuid>, op: Op) -> Result<Reply, String> {
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
                self.note_viewing(client.id, session, true);
                Ok(Reply::Attached { info })
            }
            Op::Detach { session } => {
                attached.remove(&session);
                self.detach(session, client.id);
                self.note_viewing(client.id, session, false);
                Ok(Reply::Done)
            }
            Op::Viewers => Ok(Reply::Viewers { viewers: self.viewers() }),
            Op::DisconnectRemote => {
                self.disconnect_remote();
                Ok(Reply::Done)
            }
            Op::ReloadRemote => {
                let (flag, wake) = &self.remote.wake;
                *flag.locked() = true;
                wake.notify_all();
                Ok(Reply::Done)
            }
            Op::Resize { session, cols, rows } => {
                self.resize_for(client, role, session, cols, rows)?;
                Ok(Reply::Done)
            }
            Op::Claim { session, cols, rows } => {
                let live = self.session(session)?;
                let now = {
                    let state = live.state.locked();
                    (state.cols, state.rows)
                };
                let changed = now != bounded(cols, rows);
                if changed || !matches!(role, Role::Remote { .. }) {
                    // The same size still makes it this computer's.
                    self.resize_for(client, role, session, cols, rows)?;
                }
                Ok(Reply::Claimed { changed })
            }
            Op::PromptRedraw { session } => {
                // The app has already checked the shell is at its prompt;
                // for the web client (resized to a phone) the daemon does:
                // nothing running under it.
                let live = self.session(session)?;
                let remote = matches!(role, Role::Remote { .. });
                live.prompt_redraw(|pid| !remote || shell_is_idle(pid))?;
                Ok(Reply::Done)
            }
            Op::Kill { session } => {
                let live = self.session(session)?;
                // From the tailnet, a console the app shows closes as its
                // tab would: the app is asked (see Op::Opener), and the
                // console goes once it has closed it.
                if matches!(role, Role::Remote { .. }) {
                    let opener = self.opener.locked().as_ref().map(|opener| opener.client.clone());
                    if let Some(opener) = opener {
                        if !opener.offer(Frame::Message(ServerMessage::Close { session })) {
                            return Err("no-app".into());
                        }
                        return Ok(Reply::Done);
                    }
                }
                // The app's own consoles are closed there, not from here.
                if live.host.is_some() {
                    return Err("this console belongs to the app".into());
                }
                attached.remove(&session);
                self.kill(session)?;
                Ok(Reply::Done)
            }
            Op::Host { session, console } => {
                let existing = self.sessions.locked().get(&session).map(|live| live.host.is_some());
                match existing {
                    // Shared again over a new connection before the daemon
                    // saw the old one end: the new one takes over.
                    Some(true) => self.kill(session)?,
                    Some(false) => return Err(format!("session {session} already exists")),
                    None => {}
                }
                let live = Live::new(
                    session,
                    console.path,
                    console.cwd,
                    console.started_at,
                    (console.cols, console.rows),
                    Some((client.clone(), console.pid)),
                );
                self.sessions.locked().insert(session, live);
                self.touch();
                Ok(Reply::Done)
            }
            Op::Foreground { session, path } => {
                let live = self.session(session)?;
                if live.hosted_by(client.id) {
                    let mut state = live.state.locked();
                    state.foreground = path;
                    live.report(&mut state);
                }
                Ok(Reply::Done)
            }
            Op::Agent { session, agent } => {
                let live = self.session(session)?;
                let mut state = live.state.locked();
                if state.agent != agent {
                    state.agent = agent;
                    live.report(&mut state);
                }
                Ok(Reply::Done)
            }
            Op::Unhost { session } => {
                if self.session(session)?.hosted_by(client.id) {
                    self.kill(session)?;
                }
                Ok(Reply::Done)
            }
            Op::List => Ok(Reply::Sessions { sessions: self.list() }),
            Op::OpenChoices => {
                let opener = self.opener.locked();
                let (choices, recent) =
                    opener.as_ref().map(|opener| (opener.choices.clone(), opener.recent.clone())).unwrap_or_default();
                Ok(Reply::OpenChoices { choices, recent })
            }
            // Where a new console opens, picked by walking folders (the web
            // client's browser). Errors are words it translates.
            Op::FolderRoots => Ok(Reply::FolderRoots { roots: folder_commands::folder_roots() }),
            Op::ListFolders { path } => {
                if !usable_folder(&path) {
                    return Err("unreadable".into());
                }
                let mut folders = folder_commands::subfolders(std::path::Path::new(&path)).map_err(|_| "unreadable".to_string())?;
                folders.truncate(MAX_FOLDERS);
                Ok(Reply::Folders { folders })
            }
            Op::CreateFolder { parent, name } => {
                if !usable_folder(&parent) {
                    return Err("unreadable".into());
                }
                let name = name.trim();
                if !folder_commands::is_valid_folder_name(name) {
                    return Err("bad-name".into());
                }
                let parent = std::path::Path::new(&parent);
                if parent.join(name).exists() {
                    return Err("exists".into());
                }
                let folder = folder_commands::new_folder(parent, name).map_err(|_| "failed".to_string())?;
                Ok(Reply::Folder { folder })
            }
            // Asked on a thread of its own (see handle_connection).
            Op::Open { choice, cwd } => self.open(&choice, cwd),
            Op::Opener { mut choices, mut recent, agents } => {
                choices.truncate(MAX_CHOICES);
                recent.truncate(MAX_CHOICES);
                *self.opener.locked() = Some(Opener { client: client.clone(), choices, recent, agents });
                // Someone was watching the agents already.
                if agents && !self.agent_watchers.locked().is_empty() {
                    client.offer(Frame::Message(ServerMessage::AgentsWatch { on: true }));
                }
                Ok(Reply::Done)
            }
            Op::AgentsAnswer { request, result } => {
                if let Some((asked, answer)) = self.agents_asking.locked().get(&request) {
                    if *asked == client.id {
                        let _ = answer.try_send(result);
                    }
                }
                Ok(Reply::Done)
            }
            Op::AgentsEvent { name, payload } => {
                let from_app = self.opener.locked().as_ref().is_some_and(|opener| opener.client.id == client.id);
                if from_app {
                    self.pass_on_agent_event(name, payload);
                }
                Ok(Reply::Done)
            }
            // Asked on a thread of its own (see handle_connection).
            Op::Agents { call, args } => self.agents(client, role, &call, args),
            Op::Opened { request, result } => {
                if let Some((asked, answer)) = self.opening.locked().get(&request) {
                    if *asked == client.id {
                        let _ = answer.try_send(result);
                    }
                }
                Ok(Reply::Done)
            }
            Op::Shutdown => Ok(Reply::Done),
        }
    }

    fn spawn(self: &Arc<Self>, id: Uuid, spec: &SpawnSpec) -> Result<Option<u32>, String> {
        if self.sessions.locked().contains_key(&id) {
            return Err(format!("session {id} already exists"));
        }
        let live = Live::new(id, spec.path.clone(), spec.cwd.clone(), now_seconds(), (spec.cols, spec.rows), None);
        let sink = self.sink_for(&live);
        let pty = PtySession::spawn(id, spec, sink).map_err(|err| err.to_string())?;
        let pid = pty.pid();
        let _ = live.pty.set(pty);
        self.sessions.locked().insert(id, live);
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
                        let mut state = live.state.locked();
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
                    let mut state = live.state.locked();
                    state.foreground = path.clone();
                    let message = ServerMessage::Foreground { session: live.id, path };
                    state.clients.retain(|client| client.offer(Frame::Message(message.clone())));
                    live.report(&mut state);
                }
            }
            true
        })
    }

    fn attach(&self, id: Uuid, client: &Client, since: u64) -> Result<SessionInfo, String> {
        let live = self.session(id)?;
        let mut state = live.state.locked();
        state.clients.retain(|other| other.id != client.id);
        // Under the same lock as new output: nothing is missed or sent twice.
        // What came after `since`, if it's all still kept; else (and to
        // a client attaching anew) the screen as it is.
        let replay = (since > 0).then(|| state.history.since(id, since)).flatten();
        let frames = replay.unwrap_or_else(|| {
            let title = state.title.clone();
            let bytes = state.screen.snapshot(title.as_deref());
            vec![Frame::Output { session: id, seq: state.seq, bytes }]
        });
        for frame in frames {
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
        let (finished, unanswered, give_back) = {
            let mut state = live.state.locked();
            state.clients.retain(|other| other.id != client);
            if state.typist == Some(client) {
                state.typist = None;
            }
            // Leaving with the console sized to their screen: it gets its
            // own size back.
            let give_back = match &state.borrowed {
                Some(borrowed) if borrowed.by == client => state.borrowed.take().map(|size| (size.cols, size.rows)),
                _ => None,
            };
            let alone = state.clients.is_empty();
            let unanswered = (alone && std::mem::take(&mut state.cursor_query)).then(|| state.screen.cursor_report());
            (alone && state.exit_code.is_some(), unanswered, give_back)
        };
        if let Some((cols, rows)) = give_back {
            let _ = live.resize(cols, rows);
            // Its prompt was drawn for the phone's width (see PromptRedraw).
            let _ = live.prompt_redraw(shell_is_idle);
        }
        if let Some(report) = unanswered {
            live.answer_cursor_query(&report);
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
            let mut state = live.state.locked();
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
        let Some(live) = self.sessions.locked().remove(&id) else {
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
        let sessions: Vec<Arc<Live>> = self.sessions.locked().values().cloned().collect();
        let mut list: Vec<SessionInfo> =
            sessions.iter().map(|live| live.info(&live.state.locked())).collect();
        // Outside the consoles' locks: it may run git.
        for info in &mut list {
            info.git = info.cwd.as_deref().and_then(status::git_info);
        }
        list.sort_by_key(|info| info.started_at);
        list
    }
}

impl Live {
    fn new(
        id: Uuid,
        path: String,
        cwd: Option<String>,
        started_at: i64,
        (cols, rows): (u16, u16),
        host: Option<(Client, Option<u32>)>,
    ) -> Arc<Self> {
        Arc::new(Live {
            id,
            path,
            cwd,
            started_at,
            pty: OnceLock::new(),
            host,
            state: Mutex::new(State {
                clients: Vec::new(),
                history: History::default(),
                screen: Screen::new(rows, cols),
                seq: 0,
                cols,
                rows,
                title: None,
                exit_code: None,
                cursor_query: false,
                typist: None,
                borrowed: None,
                cwd_now: None,
                foreground: None,
                agent: None,
            }),
        })
    }

    /// Tells whoever shows it what its status bar shows now.
    fn report(&self, state: &mut State) {
        let info = ServerMessage::Info { info: self.info(state) };
        state.clients.retain(|client| client.offer(Frame::Message(info.clone())));
    }

    /// Whether it's one of the app's consoles, hosted over `client`.
    fn hosted_by(&self, client: u64) -> bool {
        self.host.as_ref().is_some_and(|(host, _)| host.id == client)
    }

    /// Types into it: its pty, or the app that runs it.
    fn write(&self, bytes: &[u8]) {
        if let Some((host, _)) = &self.host {
            host.offer(Frame::Input { session: self.id, bytes: bytes.to_vec() });
        } else if let Some(pty) = self.pty.get() {
            let _ = pty.write(bytes);
        }
    }

    /// Has the shell draw its prompt again, if it sits at it. The app
    /// checks that itself for its own consoles.
    fn prompt_redraw(&self, shell_is_idle: impl FnOnce(u32) -> bool) -> Result<(), String> {
        if let Some((host, _)) = &self.host {
            host.offer(Frame::Message(ServerMessage::HostPromptRedraw { session: self.id }));
            return Ok(());
        }
        match self.pty.get() {
            Some(pty) => pty.prompt_redraw(shell_is_idle).map_err(|err| err.to_string()),
            None => Ok(()),
        }
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        if let Some((host, _)) = &self.host {
            host.offer(Frame::Message(ServerMessage::HostResize { session: self.id, cols, rows }));
        } else if let Some(pty) = self.pty.get() {
            // Asked again for the size it has (a tab taken up again): left be.
            let now = {
                let state = self.state.locked();
                (state.cols, state.rows)
            };
            if now == (cols, rows) {
                return Ok(());
            }
            pty.resize(cols, rows).map_err(|err| err.to_string())?;
        }
        self.resized(cols, rows);
        Ok(())
    }

    /// Its copy of the screen follows a size it already has, and those
    /// showing it hear of it: a phone shows it at that size.
    fn resized(&self, cols: u16, rows: u16) {
        let mut state = self.state.locked();
        if (state.cols, state.rows) == (cols, rows) {
            return;
        }
        state.cols = cols;
        state.rows = rows;
        state.screen.resize(rows, cols);
        self.report(&mut state);
    }

    fn info(&self, state: &State) -> SessionInfo {
        SessionInfo {
            id: self.id,
            pid: self.pty.get().and_then(PtySession::pid).or_else(|| self.host.as_ref().and_then(|(_, pid)| *pid)),
            path: self.path.clone(),
            cwd: state.cwd_now.clone().or_else(|| self.cwd.clone()),
            started_at: self.started_at,
            cols: state.cols,
            rows: state.rows,
            title: state.title.clone(),
            attached: state.clients.len() as u32,
            attached_here: Some(state.clients.iter().filter(|client| client.local).count() as u32),
            exit_code: state.exit_code,
            hosted: self.host.is_some(),
            foreground: state.foreground.clone(),
            git: None,
            agent: state.agent.clone(),
        }
    }

    fn output(&self, bytes: Vec<u8>) {
        let answer = {
            let mut state = self.state.locked();
            state.seq += 1;
            let seq = state.seq;
            if let Some(title) = last_title(&bytes) {
                state.title = Some(title);
            }
            if let Some(cwd) = status::last_cwd(&bytes).filter(|cwd| state.cwd_now.as_ref() != Some(cwd)) {
                state.cwd_now = Some(cwd);
                self.report(&mut state);
            }
            state.screen.process(&bytes);
            let frame = Frame::Output { session: self.id, seq, bytes: bytes.clone() };
            state.clients.retain(|client| client.offer(frame.clone()));
            let asked = contains(&bytes, CURSOR_QUERY);
            state.history.push(seq, bytes);
            if asked {
                state.cursor_query = !state.clients.is_empty();
            }
            (asked && state.clients.is_empty()).then(|| state.screen.cursor_report())
        };
        if let Some(report) = answer {
            self.answer_cursor_query(&report);
        }
    }

    /// No terminal to answer ConPTY's question: without an answer the
    /// console would never finish starting (see CURSOR_QUERY).
    fn answer_cursor_query(&self, report: &[u8]) {
        if let Some(pty) = self.pty.get() {
            let _ = pty.write(report);
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

    /// Output after `since`, as frames — None when some of it is no longer
    /// kept.
    fn since(&self, session: Uuid, since: u64) -> Option<Vec<Frame<ServerMessage>>> {
        let first = self.chunks.front().map(|(seq, _)| *seq);
        if self.trimmed && first.is_some_and(|first| since + 1 < first) {
            return None;
        }
        let frames = self
            .chunks
            .iter()
            .filter(|(seq, _)| *seq > since)
            .map(|(seq, bytes)| Frame::Output { session, seq: *seq, bytes: without_queries(bytes) })
            .collect();
        Some(frames)
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

/// Whether `bytes` is nothing but what a terminal sends on its own — its
/// answers to a program's questions (cursor position, device attributes,
/// modes, colors, sizes, its version) and focus reports — rather than
/// anything typed.
fn is_terminal_reply(bytes: &[u8]) -> bool {
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != 0x1b || at + 1 >= bytes.len() {
            return false;
        }
        let end = match bytes[at + 1] {
            b'[' => {
                let start = at + 2;
                let mut end = start;
                while end < bytes.len() && !(0x40..=0x7e).contains(&bytes[end]) {
                    end += 1;
                }
                let Some(&last) = bytes.get(end) else {
                    return false;
                };
                let middle = &bytes[start..end];
                let reply = match last {
                    // Cursor position report.
                    b'R' => !middle.is_empty() && middle.iter().all(|byte| byte.is_ascii_digit() || *byte == b';'),
                    // Device attributes, primary and secondary.
                    b'c' => middle.starts_with(b"?") || middle.starts_with(b">"),
                    // Mode report (DECRPM).
                    b'y' => middle.ends_with(b"$"),
                    // Focus in, focus out.
                    b'I' | b'O' => middle.is_empty(),
                    // Sizes.
                    b't' => middle.first().is_some_and(u8::is_ascii_digit),
                    // The kitty keyboard protocol's flags.
                    b'u' => middle.starts_with(b"?"),
                    // "Terminal OK".
                    b'n' => middle == b"0",
                    _ => false,
                };
                if !reply {
                    return false;
                }
                end + 1
            }
            // A string from the terminal is always an answer: colors (OSC),
            // its version or a setting (DCS).
            b']' | b'P' => {
                let (end, _) = string_sequence(bytes, at + 2, |_| true);
                if end > bytes.len() || !ends_string(&bytes[..end]) {
                    return false;
                }
                end
            }
            _ => return false,
        };
        at = end;
    }
    !bytes.is_empty()
}

fn ends_string(bytes: &[u8]) -> bool {
    bytes.ends_with(b"\x07") || bytes.ends_with(b"\x1b\\")
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
    let daemon = Daemon::new(token.to_string(), Box::new(|_| None), Box::new(|| None));
    std::thread::spawn(move || serve(&daemon, listener));
    Ok(port)
}

/// For tests: a daemon with remote access on, its "tailnet" listener on
/// 127.0.0.1, `whois` standing in for Tailscale. Returns the local port and
/// the remote one.
#[cfg(test)]
pub fn start_remote_for_test(token: &str, dir: &std::path::Path, whois: Whois) -> std::io::Result<(u16, u16)> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
    let port = listener.local_addr()?.port();
    let daemon = Daemon::new(token.to_string(), whois, Box::new(|| Some(IpAddr::from([127, 0, 0, 1]))));
    daemon.set_dir(dir.to_path_buf());
    daemon.apply_remote_config();
    let remote_port = daemon
        .remote
        .listener
        .lock()
        .unwrap()
        .as_ref()
        .map(|(_, bound, _)| bound.port())
        .ok_or_else(|| std::io::Error::other("remote access is off"))?;
    {
        let daemon = Arc::clone(&daemon);
        std::thread::spawn(move || remote_supervisor(&daemon));
    }
    std::thread::spawn(move || serve(&daemon, listener));
    Ok((port, remote_port))
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
        assert_eq!(history.since(id, 0).map(|frames| frames.len()), Some(2));
        history.push(3, chunk.clone());
        assert!(history.trimmed);

        assert!(history.since(id, 0).is_none(), "the first output is gone");
        let newer = history.since(id, 2).unwrap();
        assert_eq!(newer.len(), 1);
        assert!(matches!(&newer[0], Frame::Output { seq: 3, .. }));
        assert!(history.since(id, 3).unwrap().is_empty());
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
    fn tells_a_terminals_answers_from_typing() {
        for reply in [
            &b"\x1b[12;40R"[..],
            b"\x1b[?1;2c",
            b"\x1b[>0;276;0c",
            b"\x1b[?2004;1$y",
            b"\x1b[I",
            b"\x1b]11;rgb:1e1e/1e1e/1e1e\x1b\\",
            b"\x1bP>|xterm.js(5.5.0)\x1b\\",
            b"\x1b[8;24;80t",
            b"\x1b[1;1R\x1b[O",
        ] {
            assert!(is_terminal_reply(reply), "{:?}", String::from_utf8_lossy(reply));
        }
        for typed in [&b"ls\r"[..], b"\x1b[A", b"\x1b[1;5D", b"\x1b[24;7~", b"a\x1b[1;1R", b"", b"\x1b]11;?"] {
            assert!(!is_terminal_reply(typed), "{:?}", String::from_utf8_lossy(typed));
        }
    }

    #[test]
    fn only_the_typists_terminal_answers() {
        let mut state = State {
            clients: Vec::new(),
            history: History::default(),
            screen: Screen::new(24, 80),
            seq: 0,
            cols: 80,
            rows: 24,
            title: None,
            exit_code: None,
            cursor_query: false,
            typist: None,
            borrowed: None,
            cwd_now: None,
            foreground: None,
            agent: None,
        };
        // No one has typed: anyone's answer goes through.
        assert!(state.accepts_input(1, b"\x1b[1;1R"));
        assert!(state.accepts_input(2, b"echo hi\r"));
        assert!(!state.accepts_input(1, b"\x1b[?1;2c"), "not the typist's");
        assert!(state.accepts_input(2, b"\x1b[?1;2c"));
        assert!(state.accepts_input(1, b"x"), "typing always goes through");
        assert!(state.accepts_input(1, b"\x1b[5;5R"));
    }

    #[test]
    fn finds_the_last_title_set() {
        assert_eq!(last_title(b"\x1b]0;one\x07ls\r\n\x1b]2;two\x1b\\"), Some("two".into()));
        assert_eq!(last_title(b"\x1b]7;file:///tmp\x07"), None, "OSC 7 is a directory, not a title");
        assert_eq!(last_title(b"plain"), None);
    }
}
