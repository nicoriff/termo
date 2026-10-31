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

pub mod agents;
pub mod client;
pub mod protocol;
pub mod remote;
mod screen;
mod status;
pub mod server;
pub mod tailscale;
mod web;

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

/// The language the app's windows speak ("es" or "en"), in the daemon's
/// folder, for what the daemon says itself (see mux/agents.rs).
pub const LANGUAGE_FILE: &str = "language";

/// Leaves the windows' language for the daemon (see LANGUAGE_FILE).
pub fn set_language(language: &str) {
    if let Some(dir) = dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(LANGUAGE_FILE), language);
    }
}

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

/// Ends the daemon in `dir` outright, with every console it runs: one that
/// stopped answering can't be asked to. Only a process of Termo's — the id
/// in an old info file may be someone else's by now.
pub fn end_daemon(dir: &Path) {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let Some(info) = read_info(dir) else {
        return;
    };
    let pid = Pid::from_u32(info.pid);
    let mut system = System::new();
    let is_termo = |system: &mut System| {
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing());
        system
            .process(pid)
            .is_some_and(|process| process.name().to_string_lossy().to_lowercase().starts_with("termo"))
    };
    if is_termo(&mut system) {
        end_process_tree(info.pid);
        let deadline = Instant::now() + Duration::from_secs(3);
        while is_termo(&mut system) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    remove_info(dir, info.pid);
}

#[cfg(windows)]
fn end_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

/// Its consoles' shells get SIGHUP when the daemon's end of their
/// terminals closes.
#[cfg(unix)]
fn end_process_tree(pid: u32) {
    // SAFETY: a plain signal to a process id.
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

/// Whether a process with this id exists. Unsure — no access to it, say — is
/// a yes: taking a running daemon for gone would start a second one.
#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    // SAFETY: a handle opened, read and closed here.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return GetLastError() != ERROR_INVALID_PARAMETER;
        }
        let mut code = 0u32;
        let read = GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        !read || code == STILL_ACTIVE as u32
    }
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    // SAFETY: signal 0 only asks whether the process is there.
    let sent = unsafe { libc::kill(pid as i32, 0) } == 0;
    sent || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

/// A connection to the daemon in `dir`, if one is running there.
pub fn connect(dir: &Path) -> Option<(TcpStream, DaemonInfo)> {
    let info = read_info(dir)?;
    // A daemon that's gone leaves its info file behind (the computer
    // restarted, say), and connecting to its port anyway takes Windows two
    // seconds to give up on — at startup, before the window can show.
    if !process_exists(info.pid) {
        return None;
    }
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
///
/// The copy is always the same file, brought up to date: Windows' firewall
/// asks about a program by its path, once — the tailnet listener (see
/// server.rs) would otherwise have it asking after every update, and in a
/// dev build after every rebuild. Only while a daemon of another build
/// still runs from it (one of an older protocol, kept for its consoles)
/// does a copy named after the build stand in.
fn daemon_program(dir: &Path, exe: &Path, build: &str) -> io::Result<PathBuf> {
    program_copy(&dir.join("bin"), DAEMON_EXE, DAEMON_BUILD, exe, build)
}

/// The same, for any process of Termo's that outlives the app (the agents'
/// too, see agents/host/): `name` in `bin`, which holds nothing else.
pub(crate) fn program_copy(bin: &Path, name: &str, stamp_name: &str, exe: &Path, build: &str) -> io::Result<PathBuf> {
    if cfg!(windows) {
        std::fs::create_dir_all(bin)?;
        let fixed = bin.join(name);
        let stamp = bin.join(stamp_name);
        // The build it says it is, and byte for byte this executable: the
        // folder is the user's to write in, and the firewall lets what's
        // at that path listen — something else put there mustn't run.
        let up_to_date = std::fs::read_to_string(&stamp).is_ok_and(|text| text == build) && same_contents(exe, &fixed);
        let program = if up_to_date {
            fixed
        } else if replace(exe, &fixed).is_ok() {
            std::fs::write(&stamp, build)?;
            fixed
        } else {
            let copy = bin.join(format!("termo-{build}.exe"));
            if !same_contents(exe, &copy) {
                replace(exe, &copy)?;
            }
            copy
        };
        // Copies no daemon runs from any more. One that's running can't be
        // deleted, which is what keeps it.
        if let Ok(entries) = std::fs::read_dir(bin) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path != program && path != bin.join(name) && path != stamp {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        return Ok(program);
    }
    Ok(std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or_else(|| exe.to_path_buf()))
}

/// The daemon's copy of the executable on Windows, and which build it is.
const DAEMON_EXE: &str = "termo-daemon.exe";
const DAEMON_BUILD: &str = "termo-daemon.build";

/// Whether two files hold the same bytes.
fn same_contents(a: &Path, b: &Path) -> bool {
    use std::io::Read;
    let (Ok(first), Ok(second)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
        return false;
    };
    if first.len() != second.len() {
        return false;
    }
    let (Ok(mut a), Ok(mut b)) = (std::fs::File::open(a), std::fs::File::open(b)) else {
        return false;
    };
    let mut left = vec![0u8; 1 << 20];
    let mut right = vec![0u8; 1 << 20];
    loop {
        let Ok(read) = a.read(&mut left) else { return false };
        if read == 0 {
            return true;
        }
        if b.read_exact(&mut right[..read]).is_err() || left[..read] != right[..read] {
            return false;
        }
    }
}

/// Puts a copy of `exe` at `to` all at once; fails if a process runs from
/// `to`.
fn replace(exe: &Path, to: &Path) -> io::Result<()> {
    let tmp = to.with_extension("exe.tmp");
    std::fs::copy(exe, &tmp)?;
    std::fs::rename(&tmp, to).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Lets the daemon outlive the app that started it: no console of its own,
/// out of the app's process group (and job, on Windows), so that ending
/// the app — or the terminal it was started from — doesn't end it.
#[cfg(windows)]
pub(crate) fn detach(command: &mut Command) -> io::Result<()> {
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
pub(crate) fn detach(command: &mut Command) -> io::Result<()> {
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
pub(crate) fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    /// One path for the firewall to know, whatever the build: brought up to
    /// date in place, and the copies of earlier builds cleared away.
    #[test]
    fn the_daemon_runs_from_the_same_file_build_after_build() {
        let dir = std::env::temp_dir().join(format!("termo-daemon-program-{}", uuid::Uuid::new_v4().simple()));
        let exe = dir.join("termo.exe");
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin").join("termo-0.1.0-old.exe"), b"old").unwrap();

        std::fs::write(&exe, b"first").unwrap();
        let first = daemon_program(&dir, &exe, "first").unwrap();
        assert_eq!(first, dir.join("bin").join(DAEMON_EXE));
        assert_eq!(std::fs::read(&first).unwrap(), b"first");

        std::fs::write(&exe, b"second").unwrap();
        let second = daemon_program(&dir, &exe, "second").unwrap();
        assert_eq!(second, first);
        assert_eq!(std::fs::read(&second).unwrap(), b"second");

        // Something else put there, under the same build: replaced, not run.
        std::fs::write(&second, b"planted").unwrap();
        let third = daemon_program(&dir, &exe, "second").unwrap();
        assert_eq!(std::fs::read(&third).unwrap(), b"second");

        let mut left: Vec<String> =
            std::fs::read_dir(dir.join("bin")).unwrap().flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, [DAEMON_BUILD, DAEMON_EXE]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod process_tests {
    use super::*;

    #[test]
    fn a_running_process_exists_and_one_that_ended_does_not() {
        assert!(process_exists(std::process::id()));
        let mut child = if cfg!(windows) {
            Command::new("cmd").args(["/C", "exit"]).spawn().unwrap()
        } else {
            Command::new("true").spawn().unwrap()
        };
        let pid = child.id();
        child.wait().unwrap();
        // Reaped and its handle closed: the id names nothing now.
        drop(child);
        assert!(!process_exists(pid));
    }
}
