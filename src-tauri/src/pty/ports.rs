//! What the consoles serve: the TCP ports something running under each
//! console's shell listens on (a dev server, a database, an API), and which
//! of them answer HTTP, so the tab groups panel can show them and open the
//! web ones. Each system tells listeners and their process apart its own
//! way: Windows through its IP helper table, Linux through /proc, macOS
//! through lsof.
//!
//! A process belongs to a console when it runs under its shell, or, once
//! its parent has ended (a server an agent started in the background), by
//! the TERMO_SESSION_ID it inherited from it (see session.rs).

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;

/// How long a port gets to accept the connection, and then to answer.
const CONNECT_WITHIN: Duration = Duration::from_millis(250);
const ANSWER_WITHIN: Duration = Duration::from_millis(400);
/// New ports probed per call at most: the rest wait for the next one.
const PROBES_PER_CALL: usize = 6;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServedPort {
    pub port: u16,
    /// It answered an HTTP request: it opens in the browser.
    pub http: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsolePorts {
    pub id: String,
    pub ports: Vec<ServedPort>,
}

/// The ports each console's processes listen on: `trees` is each console's
/// id with every process under its shell, `listeners` what listens where
/// (pid, port). A port shows once per console, in order.
pub fn by_console(trees: &[(String, HashSet<u32>)], listeners: &[(u32, u16)], http: impl Fn(u32, u16) -> bool) -> Vec<ConsolePorts> {
    trees
        .iter()
        .filter_map(|(id, tree)| {
            let mut ports: Vec<(u16, u32)> = listeners.iter().filter(|(pid, _)| tree.contains(pid)).map(|(pid, port)| (*port, *pid)).collect();
            ports.sort_unstable();
            ports.dedup_by_key(|(port, _)| *port);
            (!ports.is_empty()).then(|| ConsolePorts {
                id: id.clone(),
                ports: ports.into_iter().map(|(port, pid)| ServedPort { port, http: http(pid, port) }).collect(),
            })
        })
        .collect()
}

/// Whether a port answers HTTP, remembered per process and port: a server
/// doesn't change what it speaks, and probing it on every poll would show
/// up in its log. Ports nobody listens on any more are forgotten.
pub struct HttpProbe(Mutex<HashMap<(u32, u16), bool>>);

impl Default for HttpProbe {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

impl HttpProbe {
    pub fn answers(&self, listeners: &[(u32, u16)]) -> HashMap<(u32, u16), bool> {
        let mut known = self.0.lock().unwrap();
        let live: HashSet<(u32, u16)> = listeners.iter().copied().collect();
        known.retain(|key, _| live.contains(key));
        let mut probed = 0;
        for key in listeners {
            if known.contains_key(key) || probed == PROBES_PER_CALL {
                continue;
            }
            probed += 1;
            known.insert(*key, speaks_http(key.1));
        }
        known.clone()
    }
}

/// Asks the port for its root page and looks at how the answer starts.
fn speaks_http(port: u16) -> bool {
    let addresses = [SocketAddr::from((Ipv4Addr::LOCALHOST, port)), SocketAddr::from((Ipv6Addr::LOCALHOST, port))];
    addresses.iter().any(|address| {
        let Ok(mut stream) = TcpStream::connect_timeout(address, CONNECT_WITHIN) else { return false };
        let _ = stream.set_read_timeout(Some(ANSWER_WITHIN));
        let _ = stream.set_write_timeout(Some(ANSWER_WITHIN));
        if stream.write_all(b"HEAD / HTTP/1.0\r\nHost: localhost\r\n\r\n").is_err() {
            return false;
        }
        let mut start = [0u8; 5];
        stream.read_exact(&mut start).is_ok() && &start == b"HTTP/"
    })
}

/// What listens for TCP connections on this computer, as (pid, port), as
/// far as this user can see.
pub fn listeners() -> Vec<(u32, u16)> {
    let mut found = system_listeners();
    found.sort_unstable();
    found.dedup();
    found
}

/// The console a process was started in, from its environment.
pub fn console_of(environ: &[std::ffi::OsString]) -> Option<String> {
    environ.iter().find_map(|entry| entry.to_str()?.strip_prefix("TERMO_SESSION_ID=").map(str::to_string))
}

#[cfg(windows)]
fn system_listeners() -> Vec<(u32, u16)> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };
    const AF_INET: u32 = 2;
    const AF_INET6: u32 = 23;
    const NO_ERROR: u32 = 0;

    /// The table for one address family, as its rows' raw words: a count,
    /// then the rows.
    fn table(family: u32) -> Option<Vec<u32>> {
        let mut size = 0u32;
        // SAFETY: a null buffer only asks for the size the table needs.
        unsafe { GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, family, TCP_TABLE_OWNER_PID_LISTENER, 0) };
        for _ in 0..3 {
            // u32 words: the rows hold u32 fields, so the buffer is aligned for them.
            let mut buffer = vec![0u32; (size as usize).div_ceil(4) + 1];
            let mut given = (buffer.len() * 4) as u32;
            // SAFETY: the buffer holds `given` bytes, as the call is told.
            let result = unsafe {
                GetExtendedTcpTable(buffer.as_mut_ptr().cast(), &mut given, 0, family, TCP_TABLE_OWNER_PID_LISTENER, 0)
            };
            if result == NO_ERROR {
                return Some(buffer);
            }
            // Grown since it was measured: measure again.
            size = given;
        }
        None
    }

    let port = |raw: u32| u16::from_be(raw as u16);
    let mut found = Vec::new();
    if let Some(buffer) = table(AF_INET) {
        let count = buffer[0] as usize;
        let room = (buffer.len() - 1) * 4 / std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
        // SAFETY: the rows follow the count, within the buffer (`room` caps it).
        let rows = unsafe { std::slice::from_raw_parts(buffer.as_ptr().add(1).cast::<MIB_TCPROW_OWNER_PID>(), count.min(room)) };
        found.extend(rows.iter().map(|row| (row.dwOwningPid, port(row.dwLocalPort))));
    }
    if let Some(buffer) = table(AF_INET6) {
        let count = buffer[0] as usize;
        let room = (buffer.len() - 1) * 4 / std::mem::size_of::<MIB_TCP6ROW_OWNER_PID>();
        // SAFETY: as above.
        let rows = unsafe { std::slice::from_raw_parts(buffer.as_ptr().add(1).cast::<MIB_TCP6ROW_OWNER_PID>(), count.min(room)) };
        found.extend(rows.iter().map(|row| (row.dwOwningPid, port(row.dwLocalPort))));
    }
    found
}

#[cfg(target_os = "linux")]
fn system_listeners() -> Vec<(u32, u16)> {
    // Listening sockets by inode, then whose they are.
    let mut ports: HashMap<u64, u16> = HashMap::new();
    for file in ["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(text) = std::fs::read_to_string(file) {
            ports.extend(text.lines().skip(1).filter_map(listening_socket));
        }
    }
    if ports.is_empty() {
        return Vec::new();
    }
    let pids = std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok());
    let mut found = Vec::new();
    for pid in pids {
        // Other users' processes can't be read: nothing of theirs is a console's.
        let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) else { continue };
        for entry in entries.flatten() {
            let Ok(target) = std::fs::read_link(entry.path()) else { continue };
            let target = target.to_string_lossy();
            let inode = target.strip_prefix("socket:[").and_then(|rest| rest.strip_suffix(']')).and_then(|n| n.parse::<u64>().ok());
            if let Some(port) = inode.and_then(|inode| ports.get(&inode)) {
                found.push((pid, *port));
            }
        }
    }
    found
}

/// One line of /proc/net/tcp: its inode and port when it's listening (state 0A).
#[cfg(any(target_os = "linux", test))]
fn listening_socket(line: &str) -> Option<(u64, u16)> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.get(3) != Some(&"0A") {
        return None;
    }
    let port = u16::from_str_radix(fields.get(1)?.rsplit(':').next()?, 16).ok()?;
    let inode = fields.get(9)?.parse().ok()?;
    Some((inode, port))
}

#[cfg(target_os = "macos")]
fn system_listeners() -> Vec<(u32, u16)> {
    let Ok(output) = std::process::Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pn"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    lsof_listeners(&String::from_utf8_lossy(&output.stdout))
}

/// lsof's field output: a "p<pid>" line, then an "n<address>:<port>" line
/// per socket of that process.
#[cfg(any(target_os = "macos", test))]
fn lsof_listeners(text: &str) -> Vec<(u32, u16)> {
    let mut found = Vec::new();
    let mut pid = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            pid = rest.parse::<u32>().ok();
        } else if let (Some(rest), Some(pid)) = (line.strip_prefix('n'), pid) {
            if let Some(port) = rest.rsplit(':').next().and_then(|port| port.parse::<u16>().ok()) {
                found.push((pid, port));
            }
        }
    }
    found
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn system_listeners() -> Vec<(u32, u16)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn each_console_gets_the_ports_of_its_own_processes() {
        let trees = vec![("a".to_string(), HashSet::from([10, 11])), ("b".to_string(), HashSet::from([20]))];
        let listeners = [(11, 5173), (11, 5173), (10, 3000), (30, 8080)];
        let ports = by_console(&trees, &listeners, |_, port| port == 5173);
        assert_eq!(
            ports,
            vec![ConsolePorts {
                id: "a".into(),
                ports: vec![ServedPort { port: 3000, http: false }, ServedPort { port: 5173, http: true }],
            }]
        );
    }

    #[test]
    fn the_console_comes_from_the_inherited_variable() {
        let environ: Vec<std::ffi::OsString> = vec!["PATH=/bin".into(), "TERMO_SESSION_ID=abc".into()];
        assert_eq!(console_of(&environ).as_deref(), Some("abc"));
        assert_eq!(console_of(&environ[..1]), None);
    }

    #[test]
    fn proc_net_tcp_lines_give_listening_sockets() {
        let listening = "   0: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 4242 1";
        let connected = "   1: 0100007F:A3C2 0100007F:1F90 01 00000000:00000000 00:00000000 00000000  1000        0 4343 1";
        assert_eq!(listening_socket(listening), Some((4242, 8080)));
        assert_eq!(listening_socket(connected), None);
    }

    #[test]
    fn lsof_output_gives_pids_and_ports() {
        let text = "p123\nn*:3000\nn[::1]:5173\np456\nn127.0.0.1:8080\n";
        assert_eq!(lsof_listeners(text), vec![(123, 3000), (123, 5173), (456, 8080)]);
    }

    #[test]
    fn a_port_of_this_process_is_found_and_told_from_http() {
        let socket = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        let me = std::process::id();
        assert!(listeners().contains(&(me, port)));

        // Something that answers like a web server.
        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = socket.accept() {
                let mut request = [0u8; 64];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(b"HTTP/1.0 200 OK\r\n\r\n");
            }
        });
        assert!(speaks_http(port));
        let _ = server.join();
        // Nobody there any more.
        assert!(!speaks_http(port));
    }
}
