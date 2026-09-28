//! How many times an MCP server's package was downloaded last month, from
//! the package's own registry — the MCP registry doesn't publish usage. npm
//! and PyPI (via pypistats) answer without an account; anything else, and
//! remote servers (nothing to download), have no count.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

/// Asked once per package while the app runs: the counts move slowly, and a
/// search shows the same packages again and again.
static CACHE: Mutex<Option<HashMap<String, Option<u64>>>> = Mutex::new(None);

const TIMEOUT: Duration = Duration::from_secs(6);

/// The URL that answers for a package, or None for a registry without one.
pub fn stats_url(registry_type: &str, identifier: &str) -> Option<String> {
    let identifier = identifier.trim();
    // Only what package names are made of: nothing that could reshape the URL.
    let valid = !identifier.is_empty()
        && identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '/' | '-' | '_' | '.'));
    if !valid {
        return None;
    }
    match registry_type {
        "npm" => Some(format!("https://api.npmjs.org/downloads/point/last-month/{identifier}")),
        "pypi" => Some(format!("https://pypistats.org/api/packages/{}/recent", identifier.to_lowercase())),
        _ => None,
    }
}

/// The count in either API's answer.
pub fn parse_count(registry_type: &str, payload: &Value) -> Option<u64> {
    match registry_type {
        "npm" => payload.get("downloads")?.as_u64(),
        "pypi" => payload.pointer("/data/last_month")?.as_u64(),
        _ => None,
    }
}

pub fn last_month(registry_type: &str, identifier: &str) -> Option<u64> {
    let url = stats_url(registry_type, identifier)?;
    if let Some(cached) = CACHE.lock().unwrap().as_ref().and_then(|cache| cache.get(&url).copied()) {
        return cached;
    }
    let count = ureq::get(&url)
        .timeout(TIMEOUT)
        .call()
        .ok()
        .and_then(|response| response.into_string().ok())
        .and_then(|body| serde_json::from_str::<Value>(&body).ok())
        .and_then(|payload| parse_count(registry_type, &payload));
    CACHE.lock().unwrap().get_or_insert_with(HashMap::new).insert(url, count);
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_for_each_registry() {
        assert_eq!(
            stats_url("npm", "@modelcontextprotocol/server-github").as_deref(),
            Some("https://api.npmjs.org/downloads/point/last-month/@modelcontextprotocol/server-github")
        );
        assert_eq!(
            stats_url("pypi", "Mcp-Server-Fetch").as_deref(),
            Some("https://pypistats.org/api/packages/mcp-server-fetch/recent")
        );
        assert!(stats_url("oci", "ghcr.io/x").is_none());
    }

    #[test]
    fn odd_names_never_reach_a_url() {
        assert!(stats_url("npm", "x?y=1").is_none());
        assert!(stats_url("npm", "a b").is_none());
        assert!(stats_url("npm", "").is_none());
    }

    #[test]
    fn reads_both_answers() {
        assert_eq!(parse_count("npm", &serde_json::json!({ "downloads": 1234 })), Some(1234));
        assert_eq!(parse_count("pypi", &serde_json::json!({ "data": { "last_month": 99 } })), Some(99));
        assert_eq!(parse_count("npm", &serde_json::json!({ "error": "not found" })), None);
    }
}
