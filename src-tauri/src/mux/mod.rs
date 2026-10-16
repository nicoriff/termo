//! Consoles that outlive the window, and Termo itself: the sessions daemon.
//!
//! With "Keep consoles running" on (AppConfig.persistent_sessions), a new
//! console isn't started by the app but by a process of its own, `termo
//! --mux`, which keeps it — the shell, and whatever runs under it — when the
//! window closes, the app quits or updates. The app is then one more client
//! of it: it shows what a console prints and types into it, and on the next
//! start it attaches to the ones still running (the way tmux, WezTerm's mux
//! server or shpool work).
//!
//! The daemon listens on 127.0.0.1, on a port of its own choosing, and only
//! answers a client that presents the token in its info file
//! (`daemon-v<protocol>.json`, in a folder only this user can read).
//! Each protocol version has its own daemon and info file, so an app that
//! updated keeps the consoles of the old one running (see protocol.rs).

pub mod client;
pub mod protocol;
pub mod server;

use std::io;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use protocol::PROTOCOL;

/// `termo --mux <build> <folder>`: runs the daemon instead of the app (see
/// lib::run_cli_mode_from_args).
pub const MUX_FLAG: &str = "--mux";

/// The same folder Tauri's `app_data_dir` names (tauri.conf.json's
/// identifier); the daemon has no AppHandle to ask.
const IDENTIFIER: &str = "com.ln.termo";

/// What a running daemon leaves for the app to find it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonInfo {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub version: String,
    /// Which build of Termo it runs (see build_stamp).
    pub build: String,
}

/// The daemon's folder. A dev build's is its own: it must never take over
/// the installed Termo's consoles, nor they its. TERMO_MUX_DIR puts it
/// elsewhere (tests).
pub fn dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("TERMO_MUX_DIR") {
        return Some(PathBuf::from(dir));
    }
    let name = if cfg!(debug_assertions) { "mux-dev" } else { "mux" };
    Some(dirs::data_dir()?.join(IDENTIFIER).join(name))
}

fn info_path(dir: &Path) -> PathBuf {
    dir.join(format!("daemon-v{PROTOCOL}.json"))
}

pub fn read_info(dir: &Path) -> Option<DaemonInfo> {
    let text = std::fs::read_to_string(info_path(dir)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_info(dir: &Path, info: &DaemonInfo) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = info_path(dir);
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(info).map_err(io::Error::other)?;
    crate::agents::console_bridge::write_private(&tmp, &text)?;
    std::fs::rename(&tmp, &path)
}

/// Removes the info file, if it is still this daemon's.
fn remove_info(dir: &Path, pid: u32) {
    if read_info(dir).is_some_and(|info| info.pid == pid) {
        let _ = std::fs::remove_file(info_path(dir));
    }
}

/// A connection to the daemon in `dir`, if one is running there.
pub fn connect(dir: &Path) -> Option<(TcpStream, DaemonInfo)> {
    let info = read_info(dir)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], info.port));
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).ok()?;
    Some((stream, info))
}

/// Identifies a build of Termo: its version, and for builds that share one
/// (every dev build does) the executable's size and time.
pub fn build_stamp(exe: &Path) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let Ok(meta) = std::fs::metadata(exe) else {
        return version.to_string();
    };
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_secs());
    format!("{version}-{:x}-{modified:x}", meta.len())
}

/// Starts a daemon in `dir` — `exe` run with MUX_FLAG — and waits until it
/// can be reached.
pub fn launch(dir: &Path, exe: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let build = build_stamp(exe);
    let program = daemon_program(dir, exe, &build)?;
    let mut command = Command::new(program);
    // The folder as an argument: in the environment, every console the
    // daemon starts would inherit it.
    command
        .arg(MUX_FLAG)
        .arg(&build)
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut command)?;

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if read_info(dir).is_some_and(|info| info.build == build) && connect(dir).is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(io::Error::new(io::ErrorKind::TimedOut, "the sessions daemon did not start"))
}

/// What to run as the daemon. On Windows a copy of the executable rather
/// than the installed one: a running .exe can't be replaced, and the
/// daemon keeps running across updates. Elsewhere replacing a running file
/// is fine — and in an AppImage the executable only works from inside it.
fn daemon_program(dir: &Path, exe: &Path, build: &str) -> io::Result<PathBuf> {
    if cfg!(windows) {
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin)?;
        let copy = bin.join(format!("termo-{build}.exe"));
        if !copy.exists() {
            let tmp = copy.with_extension("exe.tmp");
            std::fs::copy(exe, &tmp)?;
            std::fs::rename(&tmp, &copy)?;
        }
        // Copies no daemon runs from any more. One that's running can't be
        // deleted, which is what keeps it.
        if let Ok(entries) = std::fs::read_dir(&bin) {
            for entry in entries.flatten() {
                if entry.path() != copy {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        return Ok(copy);
    }
    Ok(std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or_else(|| exe.to_path_buf()))
}

/// Lets the daemon outlive the app that started it: no console of its own,
/// out of the app's process group (and job, on Windows), so that ending
/// the app — or the terminal it was started from — doesn't end it.
#[cfg(windows)]
fn detach(command: &mut Command) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
    match command.spawn() {
        Ok(_) => Ok(()),
        // A job that doesn't allow breaking away: the daemon stays in it.
        Err(_) => {
            command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
            command.spawn().map(|_| ())
        }
    }
}

#[cfg(unix)]
fn detach(command: &mut Command) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    // SAFETY: setsid is async-signal-safe, as pre_exec requires.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    // Reaped when it ends, so it doesn't linger as a zombie while the app
    // that started it runs.
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// A random token for the info file: two v4 UUIDs, 244 random bits.
fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}
