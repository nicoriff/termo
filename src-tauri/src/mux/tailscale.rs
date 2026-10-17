//! What Termo asks the user's own Tailscale (its CLI, which talks to the
//! tailscaled running on this computer): whether it's up, this computer's
//! address on the tailnet, the other computers on it, and who is behind a
//! connection that arrives from one (`whois`). Termo embeds nothing of
//! Tailscale's: without it installed and signed in, remote access is off.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The CLI: on the PATH, or where its installers put it.
pub fn cli() -> Option<PathBuf> {
    let name = if cfg!(windows) { "tailscale.exe" } else { "tailscale" };
    if let Some(found) = std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
    {
        return Some(found);
    }
    let known: &[&str] = if cfg!(windows) {
        &["C:\\Program Files\\Tailscale\\tailscale.exe"]
    } else if cfg!(target_os = "macos") {
        &["/Applications/Tailscale.app/Contents/MacOS/Tailscale", "/usr/local/bin/tailscale", "/opt/homebrew/bin/tailscale"]
    } else {
        &["/usr/bin/tailscale", "/usr/local/bin/tailscale", "/usr/sbin/tailscale"]
    };
    known.iter().map(PathBuf::from).find(|path| path.is_file())
}

/// Runs the CLI with `args` and parses what it prints as JSON.
fn run_json(args: &[&str]) -> Option<Value> {
    let cli = cli()?;
    let mut command = Command::new(cli);
    command.args(args).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up from a GUI app.
        command.creation_flags(0x0800_0000);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(command.output());
    });
    let output = rx.recv_timeout(Duration::from_secs(5)).ok()?.ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

/// This computer on the tailnet, and the others on it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Tailscale's own state: "Running" when it's up and signed in.
    pub backend: String,
    pub me: Machine,
    pub peers: Vec<Machine>,
    /// Whether the tailnet's machines are reachable by name (MagicDNS):
    /// otherwise only by address.
    pub magic_dns: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    /// The name the tailnet knows it by (its MagicDNS name's first label).
    pub name: String,
    /// Its full MagicDNS name, without the trailing dot.
    pub dns_name: String,
    /// Its IPv4 address on the tailnet (100.x.y.z).
    pub ip: Option<Ipv4Addr>,
    pub os: String,
    pub online: bool,
    /// Who it belongs to: the owner's login (an email address).
    pub login: Option<String>,
}

impl Status {
    pub fn running(&self) -> bool {
        self.backend == "Running" && self.me.ip.is_some()
    }
}

pub fn status() -> Option<Status> {
    parse_status(&run_json(&["status", "--json"])?)
}

fn parse_status(json: &Value) -> Option<Status> {
    let users = json.get("User").and_then(Value::as_object);
    let login_of = |node: &Value| -> Option<String> {
        let id = node.get("UserID")?.as_i64()?.to_string();
        Some(users?.get(&id)?.get("LoginName")?.as_str()?.to_string())
    };
    let machine = |node: &Value| -> Machine {
        let dns_name = node.get("DNSName").and_then(Value::as_str).unwrap_or("").trim_end_matches('.').to_string();
        let host = node.get("HostName").and_then(Value::as_str).unwrap_or("");
        let name = dns_name.split('.').next().filter(|first| !first.is_empty()).unwrap_or(host).to_string();
        let ip = node
            .get("TailscaleIPs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .find_map(|ip| ip.parse::<Ipv4Addr>().ok());
        Machine {
            name,
            dns_name,
            ip,
            os: node.get("OS").and_then(Value::as_str).unwrap_or("").to_string(),
            online: node.get("Online").and_then(Value::as_bool).unwrap_or(false),
            login: login_of(node),
        }
    };
    let me = machine(json.get("Self")?);
    let mut peers: Vec<Machine> =
        json.get("Peer").and_then(Value::as_object).into_iter().flat_map(|peers| peers.values()).map(machine).collect();
    peers.sort_by_key(|peer| peer.name.to_lowercase());
    Some(Status {
        backend: json.get("BackendState").and_then(Value::as_str).unwrap_or("").to_string(),
        me,
        peers,
        magic_dns: json
            .pointer("/CurrentTailnet/MagicDNSEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Who is behind a connection from the tailnet, as Tailscale vouches for it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    /// The user's login (an email address).
    pub login: String,
    /// The computer it comes from.
    pub node: String,
}

/// Asks tailscaled who `peer` is. None when it isn't someone on the tailnet
/// (or Tailscale can't say).
pub fn whois(peer: SocketAddr) -> Option<Identity> {
    if !is_tailnet_address(peer.ip()) {
        return None;
    }
    parse_whois(&run_json(&["whois", "--json", &peer.to_string()])?)
}

fn parse_whois(json: &Value) -> Option<Identity> {
    let login = json.get("UserProfile")?.get("LoginName")?.as_str()?.to_string();
    let node = json.get("Node")?;
    let name = node
        .get("ComputedName")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .or_else(|| node.get("Hostinfo").and_then(|info| info.get("Hostname")).and_then(Value::as_str))
        .or_else(|| node.get("Name").and_then(Value::as_str).and_then(|name| name.split('.').next()))?;
    Some(Identity { login, node: name.to_string() })
}

/// Tailscale's addresses: 100.64.0.0/10 (CGNAT space), and its IPv6 range.
pub fn is_tailnet_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            a == 100 && (64..128).contains(&b)
        }
        IpAddr::V6(v6) => v6.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_status() -> Value {
        json!({
            "Version": "1.80.0",
            "BackendState": "Running",
            "CurrentTailnet": { "MagicDNSSuffix": "tail1234.ts.net", "MagicDNSEnabled": true },
            "Self": {
                "HostName": "DESKTOP-ABC",
                "DNSName": "desktop.tail1234.ts.net.",
                "OS": "windows",
                "UserID": 111,
                "TailscaleIPs": ["fd7a:115c:a1e0::1", "100.101.102.103"],
                "Online": true
            },
            "Peer": {
                "nodekey:b": { "HostName": "iPhone", "DNSName": "phone.tail1234.ts.net.", "OS": "iOS",
                    "UserID": 111, "TailscaleIPs": ["100.70.0.2"], "Online": false },
                "nodekey:a": { "HostName": "mac", "DNSName": "macbook.tail1234.ts.net.", "OS": "macOS",
                    "UserID": 222, "TailscaleIPs": ["100.70.0.3"], "Online": true }
            },
            "User": {
                "111": { "ID": 111, "LoginName": "nicolas@example.com" },
                "222": { "ID": 222, "LoginName": "someone@example.com" }
            }
        })
    }

    #[test]
    fn reads_this_computer_and_the_others() {
        let status = parse_status(&sample_status()).unwrap();
        assert!(status.running());
        assert_eq!(status.me.name, "desktop");
        assert_eq!(status.me.dns_name, "desktop.tail1234.ts.net");
        assert!(status.magic_dns);
        assert_eq!(status.me.ip, Some(Ipv4Addr::new(100, 101, 102, 103)), "the IPv4 one");
        assert_eq!(status.me.login.as_deref(), Some("nicolas@example.com"));
        let names: Vec<&str> = status.peers.iter().map(|peer| peer.name.as_str()).collect();
        assert_eq!(names, ["macbook", "phone"]);
        assert_eq!(status.peers[0].login.as_deref(), Some("someone@example.com"));
        assert!(!status.peers[1].online);
    }

    #[test]
    fn signed_out_is_not_running() {
        let mut json = sample_status();
        json["BackendState"] = json!("NeedsLogin");
        assert!(!parse_status(&json).unwrap().running());
    }

    #[test]
    fn says_who_a_connection_is() {
        let json = json!({
            "Node": { "Name": "phone.tail1234.ts.net.", "ComputedName": "phone", "Hostinfo": { "Hostname": "iPhone" } },
            "UserProfile": { "LoginName": "nicolas@example.com", "DisplayName": "Nicolás" }
        });
        let who = parse_whois(&json).unwrap();
        assert_eq!(who, Identity { login: "nicolas@example.com".into(), node: "phone".into() });
        // Older answers without ComputedName still name the computer.
        let older = json!({ "Node": { "Name": "phone.tail1234.ts.net." }, "UserProfile": { "LoginName": "n@x" } });
        assert_eq!(parse_whois(&older).unwrap().node, "phone");
        assert!(parse_whois(&json!({ "Node": {} })).is_none());
    }

    #[test]
    fn only_tailnet_addresses_count() {
        assert!(is_tailnet_address("100.101.102.103".parse().unwrap()));
        assert!(is_tailnet_address("fd7a:115c:a1e0::5".parse().unwrap()));
        assert!(!is_tailnet_address("100.128.0.1".parse().unwrap()));
        assert!(!is_tailnet_address("192.168.1.10".parse().unwrap()));
        assert!(!is_tailnet_address("127.0.0.1".parse().unwrap()));
    }
}
