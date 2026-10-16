//! Remote access: other computers on the user's tailnet attach to the
//! consoles the sessions daemon runs (see mux/mod.rs).
//!
//! The daemon listens on this computer's Tailscale address only — never on
//! every interface, nor on localhost —, and lets a connection in only if
//! Tailscale vouches for who it is (`whois`) and that someone is on the
//! list the user keeps (`remote.json`, written by the app): by default the
//! user themselves, from any of their computers. Anyone else is turned
//! away and left pending, for the user to allow or not. Read-only access
//! shows the consoles and types nothing into them.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::tailscale::Identity;

/// Where the daemon listens on the tailnet unless told otherwise.
pub const DEFAULT_PORT: u16 = 47830;
/// How many pending requests are kept: someone retrying shouldn't fill the
/// file.
const MAX_PENDING: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Access {
    Full,
    ReadOnly,
}

/// Someone let in: a user, from one of their computers or from any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Allowed {
    pub login: String,
    /// None: any of their computers.
    #[serde(default)]
    pub node: Option<String>,
    pub access: Access,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteConfig {
    pub enabled: bool,
    pub port: u16,
    pub allowed: Vec<Allowed>,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self { enabled: false, port: DEFAULT_PORT, allowed: Vec::new() }
    }
}

impl RemoteConfig {
    /// What `who` may do, if anything.
    pub fn access_for(&self, who: &Identity) -> Option<Access> {
        let matching = self
            .allowed
            .iter()
            .filter(|allowed| allowed.login.eq_ignore_ascii_case(&who.login))
            .filter(|allowed| allowed.node.as_deref().is_none_or(|node| node.eq_ignore_ascii_case(&who.node)));
        // The broadest grant wins.
        matching.map(|allowed| allowed.access).max_by_key(|access| *access == Access::Full)
    }
}

/// Someone turned away, for the user to allow or not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub login: String,
    pub node: String,
    /// Unix seconds of the last attempt.
    pub at: i64,
}

fn config_path(dir: &Path) -> PathBuf {
    dir.join("remote.json")
}

fn pending_path(dir: &Path) -> PathBuf {
    dir.join("remote-pending.json")
}

fn log_path(dir: &Path) -> PathBuf {
    dir.join("remote.log")
}

pub fn read_config(dir: &Path) -> RemoteConfig {
    std::fs::read_to_string(config_path(dir)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

pub fn write_config(dir: &Path, config: &RemoteConfig) -> std::io::Result<()> {
    write_json(&config_path(dir), config)
}

pub fn read_pending(dir: &Path) -> Vec<Pending> {
    std::fs::read_to_string(pending_path(dir)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

pub fn write_pending(dir: &Path, pending: &[Pending]) -> std::io::Result<()> {
    write_json(&pending_path(dir), &pending)
}

fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    crate::agents::console_bridge::write_private(&tmp, &text)?;
    std::fs::rename(&tmp, path)
}

/// Records a turned-away attempt (the newest of each one's, and only the
/// newest few overall).
pub fn add_pending(dir: &Path, who: &Identity, at: i64) {
    let mut pending = read_pending(dir);
    pending.retain(|entry| !(entry.login == who.login && entry.node == who.node));
    pending.push(Pending { login: who.login.clone(), node: who.node.clone(), at });
    let excess = pending.len().saturating_sub(MAX_PENDING);
    pending.drain(..excess);
    let _ = write_pending(dir, &pending);
}

/// Past this, the log starts over (the previous one kept as `remote.log.1`).
const MAX_LOG_BYTES: u64 = 512 * 1024;

/// One line per connection let in or turned away: who, from where, when.
pub fn log(dir: &Path, at: i64, peer: SocketAddr, who: Option<&Identity>, outcome: &str) {
    use std::io::Write;
    let path = log_path(dir);
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES) {
        let _ = std::fs::rename(&path, path.with_extension("log.1"));
    }
    let who = who.map_or_else(|| "?".to_string(), |who| format!("{} ({})", who.login, who.node));
    let line = format!("{at}\t{peer}\t{who}\t{outcome}\n");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Someone connected from the tailnet right now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewer {
    pub login: String,
    pub node: String,
    pub access: Access,
    /// The consoles they're showing.
    pub sessions: Vec<uuid::Uuid>,
    /// Unix seconds.
    pub since: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who(login: &str, node: &str) -> Identity {
        Identity { login: login.into(), node: node.into() }
    }

    #[test]
    fn lets_in_whom_the_list_names_and_no_one_else() {
        let config = RemoteConfig {
            enabled: true,
            port: DEFAULT_PORT,
            allowed: vec![
                Allowed { login: "me@example.com".into(), node: None, access: Access::Full },
                Allowed { login: "friend@example.com".into(), node: Some("laptop".into()), access: Access::ReadOnly },
            ],
        };
        assert_eq!(config.access_for(&who("me@example.com", "phone")), Some(Access::Full));
        assert_eq!(config.access_for(&who("ME@example.com", "anything")), Some(Access::Full));
        assert_eq!(config.access_for(&who("friend@example.com", "laptop")), Some(Access::ReadOnly));
        assert_eq!(config.access_for(&who("friend@example.com", "desktop")), None, "only from that computer");
        assert_eq!(config.access_for(&who("stranger@example.com", "laptop")), None);
    }

    #[test]
    fn the_broadest_grant_wins() {
        let config = RemoteConfig {
            enabled: true,
            port: DEFAULT_PORT,
            allowed: vec![
                Allowed { login: "me@x".into(), node: Some("phone".into()), access: Access::ReadOnly },
                Allowed { login: "me@x".into(), node: None, access: Access::Full },
            ],
        };
        assert_eq!(config.access_for(&who("me@x", "phone")), Some(Access::Full));
    }

    #[test]
    fn pending_keeps_one_entry_per_someone_and_only_the_newest() {
        let dir = std::env::temp_dir().join(format!("termo-remote-test-{}", uuid::Uuid::new_v4().simple()));
        add_pending(&dir, &who("a@x", "one"), 1);
        add_pending(&dir, &who("a@x", "one"), 2);
        add_pending(&dir, &who("b@x", "two"), 3);
        let pending = read_pending(&dir);
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].at, 2);
        for i in 0..30 {
            add_pending(&dir, &who(&format!("{i}@x"), "n"), i);
        }
        assert_eq!(read_pending(&dir).len(), MAX_PENDING);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_old_or_partial_file_still_reads() {
        let config: RemoteConfig = serde_json::from_str(r#"{ "enabled": true }"#).unwrap();
        assert_eq!(config.port, DEFAULT_PORT);
        assert!(config.allowed.is_empty());
    }
}
