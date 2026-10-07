//! Where MCP servers are searched: the official MCP registry and Smithery,
//! behind one way of asking — a page of results, and a server's details —
//! so the panel offers them in one list and everything found installs the
//! same way, whichever it came from.
//!
//! A source that takes a key gets the user's own, entered in Preferences and
//! kept in the system keychain: never in Termo's settings file, never sent
//! back to the window once saved. Smithery's is optional — it answers
//! without one today — and is used whenever it's there.

use serde::{Deserialize, Serialize};

use crate::i18n::{tr, Msg};

use super::installed::Installed;
use super::mcp_registry::{self, McpPage};
use super::model::CatalogItem;
use super::smithery;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SourceId {
    Official,
    Smithery,
}

impl SourceId {
    pub const ALL: [SourceId; 2] = [SourceId::Official, SourceId::Smithery];

    fn key_name(self) -> Option<&'static str> {
        match self {
            SourceId::Official => None,
            SourceId::Smithery => Some("smithery/api-key"),
        }
    }
}

/// What the panel and Preferences know about a source.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub id: SourceId,
    /// It can be given an API key (optional: it searches without one).
    pub takes_key: bool,
    /// One is saved in the keychain.
    pub has_key: bool,
}

const SERVICE: &str = "termo-marketplace";

fn entry(name: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, name)
}

fn key_of(source: SourceId) -> Option<String> {
    let name = source.key_name()?;
    entry(name).and_then(|entry| entry.get_password()).ok().filter(|key| !key.trim().is_empty())
}

/// A key as a source would take it: one token, nothing that could break
/// out of the header it rides in.
pub fn is_key(key: &str) -> bool {
    (8..=512).contains(&key.len()) && key.chars().all(|c| c.is_ascii_graphic())
}

pub fn status(source: SourceId) -> SourceStatus {
    SourceStatus { id: source, takes_key: source.key_name().is_some(), has_key: key_of(source).is_some() }
}

pub fn statuses() -> Vec<SourceStatus> {
    SourceId::ALL.into_iter().map(status).collect()
}

/// Saves the source's key in the keychain, once it's been seen to work.
pub fn set_key(source: SourceId, key: &str) -> Result<SourceStatus, String> {
    let name = source.key_name().ok_or_else(|| tr(Msg::SourceNeedsNoKey))?;
    let key = key.trim();
    if !is_key(key) {
        return Err(tr(Msg::SourceKeyInvalid));
    }
    test_with(source, key)?;
    entry(name).and_then(|entry| entry.set_password(key)).map_err(|err| tr(Msg::SourceKeyNotSaved(&err.to_string())))?;
    Ok(status(source))
}

pub fn clear_key(source: SourceId) -> SourceStatus {
    if let Some(name) = source.key_name() {
        if let Ok(entry) = entry(name) {
            let _ = entry.delete_credential();
        }
    }
    status(source)
}

fn test_with(source: SourceId, key: &str) -> Result<(), String> {
    match source {
        SourceId::Official => Ok(()),
        SourceId::Smithery => smithery::test(key),
    }
}

/// Whether the saved key still works.
pub fn test(source: SourceId) -> Result<(), String> {
    match source.key_name() {
        None => Ok(()),
        Some(_) => test_with(source, &key_of(source).ok_or_else(|| tr(Msg::SourceKeyMissing))?),
    }
}

fn mark(items: &mut [CatalogItem], installed: &Installed) {
    for item in items {
        item.installed_in = installed.agents_with(&item.name, &item.extra.install_name);
    }
}

/// One page of a search in `source`, each result marked with the agents
/// that already have it.
pub fn search(source: SourceId, query: &str, cursor: Option<&str>, installed: &Installed) -> Result<McpPage, String> {
    let mut page = match source {
        SourceId::Official => mcp_registry::search(query, cursor)?,
        SourceId::Smithery => {
            let page = smithery::search(key_of(source).as_deref(), query, cursor)?;
            McpPage { items: page.items, next_cursor: page.next_cursor }
        }
    };
    mark(&mut page.items, installed);
    Ok(page)
}

/// A server's details: how to connect to it and what it brings. The
/// registry's results already carry theirs.
pub fn detail(source: SourceId, name: &str, installed: &Installed) -> Result<CatalogItem, String> {
    let mut item = match source {
        SourceId::Official => return Err(tr(Msg::SmitheryNotFound)),
        SourceId::Smithery => smithery::detail(key_of(source).as_deref(), name)?,
    };
    mark(std::slice::from_mut(&mut item), installed);
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_one_plain_token() {
        assert!(is_key("00000000-0000-4000-8000-000000000000"));
        for bad in ["", "short", "has space in it", "line\nbreak-in-the-key", "tab\tin-the-key!", &"x".repeat(600)] {
            assert!(!is_key(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_registry_needs_no_key() {
        assert_eq!(status(SourceId::Official), SourceStatus { id: SourceId::Official, takes_key: false, has_key: false });
        assert!(set_key(SourceId::Official, "whatever-key-123").is_err());
        assert_eq!(serde_json::to_string(&SourceId::Smithery).unwrap(), "\"smithery\"");
    }
}
