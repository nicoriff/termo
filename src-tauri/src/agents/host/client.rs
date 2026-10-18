//! The app's side of the agents' own process (see mod.rs): finding it or
//! starting it, handing it the calls only it can answer (see ops.rs), and
//! passing on to the windows what the agents say.

use std::collections::HashMap;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, Manager};

use crate::agents::hub::Hub;
use crate::agents::ops::{self, Agents};
use crate::agents::webhook;
use crate::config::{store as config_store, AppConfig};
use crate::i18n::{tr, Msg};
use crate::mux::protocol::{read_frame, write_frame, Frame};
use crate::state::AppState;

use super::notify::APP_PATH_FILE;
use super::protocol::{FromHost, HostStatus, ToHost, PROTOCOL};
use super::{read_info, HOST_FLAG};

/// The process's copy of the executable on Windows (see mux::program_copy).
const HOST_EXE: &str = "termo-agents.exe";
const HOST_BUILD: &str = "termo-agents.build";
/// How long it gets to start.
const START_WAIT: Duration = Duration::from_secs(12);
/// What the windows hear about it (Preferences, the quit prompt).
const STATUS_EVENT: &str = "agents-host";

struct Conn {
    writer: Mutex<TcpStream>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Result<Value, String>>>>,
    next: AtomicU64,
    /// It's of another build: used until it leaves, which it was asked to.
    retiring: bool,
}

impl Conn {
    fn send(&self, message: &ToHost) -> std::io::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        write_frame(&mut *writer, &Frame::Message(message))?;
        writer.flush()
    }
}

/// Whether the agents run in their own process (the setting on, with Modo
/// IA): then this app looks for it, starting it when it's not there.
static WANTED: AtomicBool = AtomicBool::new(false);
static SUPERVISING: AtomicBool = AtomicBool::new(false);

fn current() -> &'static Mutex<Option<Arc<Conn>>> {
    static CONN: OnceLock<Mutex<Option<Arc<Conn>>>> = OnceLock::new();
    CONN.get_or_init(Mutex::default)
}

fn last_status() -> &'static Mutex<Option<HostStatus>> {
    static STATUS: OnceLock<Mutex<Option<HostStatus>>> = OnceLock::new();
    STATUS.get_or_init(Mutex::default)
}

fn language() -> &'static Mutex<String> {
    static LANGUAGE: OnceLock<Mutex<String>> = OnceLock::new();
    LANGUAGE.get_or_init(Mutex::default)
}

fn app() -> &'static OnceLock<tauri::AppHandle> {
    static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
    &APP
}

/// Whether calls go to the agents' process.
pub fn active() -> bool {
    WANTED.load(Ordering::SeqCst)
}

/// What the windows show of it.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HostState {
    /// The agents run in their own process.
    pub enabled: bool,
    /// It answers.
    pub connected: bool,
    pub status: Option<HostStatus>,
}

pub fn state() -> HostState {
    HostState {
        enabled: active(),
        connected: current().lock().unwrap().as_ref().is_some_and(|conn| !conn.retiring),
        status: last_status().lock().unwrap().clone(),
    }
}

fn tell_windows() {
    if let Some(app) = app().get() {
        let _ = app.emit(STATUS_EVENT, state());
    }
}

/// The language the windows speak, for what the process says itself.
pub fn set_language(value: &str) {
    *language().lock().unwrap() = value.to_string();
    if let Some(conn) = current().lock().unwrap().clone() {
        let _ = conn.send(&ToHost::Language { language: value.to_string() });
    }
}

/// The agents go to their own process: this app stops running them and
/// looks for it from now on.
pub fn start(app_handle: &tauri::AppHandle) {
    let _ = app().set(app_handle.clone());
    super::set_runs_here(false);
    WANTED.store(true, Ordering::SeqCst);
    if !SUPERVISING.swap(true, Ordering::SeqCst) {
        std::thread::spawn(supervise);
    }
}

/// Keeps a connection to it while it's wanted: finds it, starts it when
/// it's not there, and again when it left (an older build that was asked
/// to, a crash).
fn supervise() {
    loop {
        if !WANTED.load(Ordering::SeqCst) {
            SUPERVISING.store(false, Ordering::SeqCst);
            // Turned back on as this was leaving.
            if WANTED.load(Ordering::SeqCst) && !SUPERVISING.swap(true, Ordering::SeqCst) {
                continue;
            }
            return;
        }
        let connected = current().lock().unwrap().is_some();
        if !connected {
            if let Err(error) = connect_or_launch() {
                eprintln!("agents host: {error}");
            }
            tell_windows();
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
}

fn connect_or_launch() -> Result<(), String> {
    let dir = super::dir().ok_or("no data folder")?;
    if connect(&dir).is_ok() {
        return Ok(());
    }
    launch(&dir)?;
    let deadline = Instant::now() + START_WAIT;
    while Instant::now() < deadline {
        if connect(&dir).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("it did not start".into())
}

fn build() -> String {
    std::env::current_exe().map(|exe| crate::mux::build_stamp(&exe)).unwrap_or_default()
}

/// What runs it: on Windows a copy of the executable that an update can't
/// be blocked by (see mux::program_copy); elsewhere the app itself (or its
/// AppImage).
pub fn program(dir: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    if cfg!(windows) {
        crate::mux::program_copy(&dir.join("bin"), HOST_EXE, HOST_BUILD, &exe, &build()).map_err(|err| err.to_string())
    } else {
        Ok(std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or(exe))
    }
}

/// Where the app is, for a notification's click to open it.
fn app_path() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).or_else(|| std::env::current_exe().ok())
}

fn launch(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    if let Some(app) = app_path() {
        let _ = std::fs::write(dir.join(APP_PATH_FILE), app.display().to_string());
    }
    let mut command = Command::new(program(dir)?);
    // The folder as an argument: in the environment, every agent's CLI
    // would inherit it.
    command.arg(HOST_FLAG).arg(build()).arg(dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    crate::mux::detach(&mut command).map_err(|err| err.to_string())
}

fn connect(dir: &Path) -> Result<(), String> {
    let info = read_info(dir).ok_or("not running")?;
    let addr = SocketAddr::from(([127, 0, 0, 1], info.port));
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|err| err.to_string())?;
    let _ = stream.set_nodelay(true);
    let mut reader = stream.try_clone().map_err(|err| err.to_string())?;
    let mut writer = stream;
    let ours = build();
    write_frame(&mut writer, &Frame::Message(ToHost::Hello { protocol: PROTOCOL, token: info.token.clone(), build: ours.clone(), observer: false }))
        .map_err(|err| err.to_string())?;
    let _ = reader.set_read_timeout(Some(Duration::from_secs(5)));
    let Ok(Some(Frame::Message(FromHost::Welcome { protocol, build, .. }))) = read_frame::<FromHost>(&mut reader) else {
        return Err("no welcome".into());
    };
    let _ = reader.set_read_timeout(None);
    // Another build: asked to leave once nothing runs, then this app starts
    // its own. Meanwhile it's used as it is — unless it speaks another
    // protocol.
    let retiring = build != ours || protocol != PROTOCOL;
    let conn = Arc::new(Conn { writer: Mutex::new(writer), pending: Mutex::default(), next: AtomicU64::new(1), retiring: protocol != PROTOCOL });
    if retiring {
        let _ = conn.send(&ToHost::Retire);
    }
    let said = language().lock().unwrap().clone();
    if !said.is_empty() {
        let _ = conn.send(&ToHost::Language { language: said });
    }
    if let Some(app) = app_path() {
        let _ = std::fs::write(dir.join(APP_PATH_FILE), app.display().to_string());
    }
    *current().lock().unwrap() = Some(conn.clone());
    std::thread::spawn(move || listen(conn, reader));
    Ok(())
}

/// Reads what it says until the connection ends.
fn listen(conn: Arc<Conn>, mut reader: TcpStream) {
    while let Ok(Some(Frame::Message(message))) = read_frame::<FromHost>(&mut reader) {
        match message {
            FromHost::Reply { id, result } => {
                if let Some(waiting) = conn.pending.lock().unwrap().remove(&id) {
                    let _ = waiting.send(result);
                }
            }
            FromHost::Event { name, payload } => {
                if let Some(app) = app().get() {
                    let _ = app.emit(&name, payload);
                }
            }
            FromHost::Status(status) => {
                webhook::set_listening_elsewhere(status.listening && !conn.retiring);
                *last_status().lock().unwrap() = Some(status);
                tell_windows();
            }
            FromHost::Welcome { .. } => {}
        }
    }
    // Gone: whoever was waiting hears it, and the supervisor looks again.
    for (_, waiting) in conn.pending.lock().unwrap().drain() {
        let _ = waiting.send(Err(tr(Msg::AgentsHostDown)));
    }
    let mut current = current().lock().unwrap();
    if current.as_ref().is_some_and(|current| Arc::ptr_eq(current, &conn)) {
        *current = None;
    }
    drop(current);
    webhook::set_listening_elsewhere(false);
    *last_status().lock().unwrap() = None;
    tell_windows();
}

/// How long a call may take: writing a report takes as long as the CLI
/// writing it; everything else answers at once.
fn patience(name: &str) -> Duration {
    match name {
        "orchestration_report" => Duration::from_secs(20 * 60),
        _ => Duration::from_secs(60),
    }
}

/// Hands one of ops::call's to the agents' process and waits for its answer.
pub fn call(name: &str, args: Value) -> Result<Value, String> {
    // Just started, or starting again: a moment to find it.
    let deadline = Instant::now() + START_WAIT;
    let conn = loop {
        if let Some(conn) = current().lock().unwrap().clone().filter(|conn| !conn.retiring) {
            break conn;
        }
        if Instant::now() >= deadline || !active() {
            return Err(tr(Msg::AgentsHostDown));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let id = conn.next.fetch_add(1, Ordering::SeqCst);
    let (sender, answer) = mpsc::channel();
    conn.pending.lock().unwrap().insert(id, sender);
    if conn.send(&ToHost::Call { id, name: name.to_string(), args }).is_err() {
        conn.pending.lock().unwrap().remove(&id);
        return Err(tr(Msg::AgentsHostDown));
    }
    match answer.recv_timeout(patience(name)) {
        Ok(result) => result,
        Err(_) => {
            conn.pending.lock().unwrap().remove(&id);
            Err(tr(Msg::AgentsHostDown))
        }
    }
}

/// Runs one of ops::call's where the agents run, off the main thread, with
/// the answer as the type the command returns.
pub async fn dispatch<T: DeserializeOwned + Send + 'static>(app: tauri::AppHandle, name: &'static str, args: Value) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let value = if active() {
            call(name, args)?
        } else {
            ops::call(&local(&app)?, name, args)?
        };
        serde_json::from_value(value).map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

/// The same, waiting on this thread, with the answer as JSON: for what the
/// tailnet asks through the sessions daemon (see mux/agents.rs).
pub fn call_where_they_run(app: &tauri::AppHandle, name: &str, args: Value) -> Result<Value, String> {
    if active() {
        call(name, args)
    } else {
        ops::call(&local(app)?, name, args)
    }
}

fn local(app: &tauri::AppHandle) -> Result<Agents, String> {
    let state = app.state::<AppState>();
    Ok(Agents { hub: Hub::for_app(app)?, runner: state.agents.clone(), orchestrator: state.orchestrator.clone() })
}

/// Tells it the config changed, so it follows at once.
fn config_changed() {
    if let Some(conn) = current().lock().unwrap().clone() {
        let _ = conn.send(&ToHost::ConfigChanged);
    }
}

/// A config was saved: Modo IA switched, or anything it follows. The agents'
/// process runs while the setting and Modo IA are both on.
pub fn config_saved(app_handle: &tauri::AppHandle, config: &AppConfig) {
    if !config.background_agents {
        return;
    }
    let wanted = config.ai_mode_enabled;
    if wanted && !active() {
        start(app_handle);
    } else if !wanted && active() {
        // It leaves by itself once it reads the config; nothing to look for.
        WANTED.store(false, Ordering::SeqCst);
    }
    config_changed();
}

/// Whether anything runs in this app's own agents.
fn busy_here(app: &tauri::AppHandle) -> bool {
    let state = app.state::<AppState>();
    !state.agents.running().is_empty() || state.orchestrator.active_count() > 0
}

fn busy_there() -> bool {
    last_status().lock().unwrap().as_ref().is_some_and(|status| status.running_runs > 0 || status.running_orchestrations > 0)
}

fn save(app: &tauri::AppHandle, change: impl FnOnce(&mut AppConfig)) -> Result<AppConfig, String> {
    let mut config = config_store::load(app);
    change(&mut config);
    config_store::save(app, &config).map_err(|err| err.to_string())?;
    Ok(config)
}

/// The setting switched: the agents move to their own process, or back to
/// the app. Refused while some are at work — a run can't move.
pub fn switch(app_handle: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let _ = app().set(app_handle.clone());
    let config = config_store::load(app_handle);
    if config.background_agents == enabled {
        return Ok(());
    }
    if enabled {
        if busy_here(app_handle) {
            return Err(tr(Msg::AgentsHostBusy));
        }
        let config = save(app_handle, |config| config.background_agents = true)?;
        super::set_runs_here(false);
        webhook::stop();
        if config.ai_mode_enabled {
            start(app_handle);
            let deadline = Instant::now() + START_WAIT + Duration::from_secs(3);
            while !state().connected {
                if Instant::now() >= deadline {
                    // Back as it was: the agents stay in the app.
                    let _ = back_to_app(app_handle);
                    return Err(tr(Msg::AgentsHostStartFailed(&tr(Msg::AgentsHostDown))));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        super::login::sync(&config);
        tell_windows();
        Ok(())
    } else {
        if busy_there() {
            return Err(tr(Msg::AgentsHostBusy));
        }
        back_to_app(app_handle)
    }
}

/// The agents back in the app: the process asked to leave (it does at once,
/// with nothing running), and the app's own server and scheduler on.
fn back_to_app(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let config = save(app_handle, |config| config.background_agents = false)?;
    WANTED.store(false, Ordering::SeqCst);
    if let Some(conn) = current().lock().unwrap().clone() {
        let _ = conn.send(&ToHost::Retire);
    }
    // Its port is the webhook's: free before the app takes it.
    if let Some(dir) = super::dir() {
        let deadline = Instant::now() + Duration::from_secs(10);
        while read_info(&dir).is_some_and(|info| super::server::is_termo_process(info.pid)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    super::set_runs_here(true);
    if config.ai_mode_enabled {
        let state = app_handle.state::<AppState>();
        if let Ok(hub) = Hub::for_app(app_handle) {
            webhook::start(hub, state.agents.clone());
        }
    }
    super::login::sync(&config);
    tell_windows();
    Ok(())
}
