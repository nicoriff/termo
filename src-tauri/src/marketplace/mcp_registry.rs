//! The MCP registry as a second catalog source:
//! https://registry.modelcontextprotocol.io. Unlike Claude's plugin
//! marketplaces — a few hundred entries, downloaded once and filtered in
//! memory — this one is large and paginated, so a search goes to the network
//! on every query instead of filtering something already fetched.

use serde_json::Value;

use crate::i18n::{tr, Msg};

use super::claude_plugins::github_owner;
use super::model::{targets_for, CatalogItem, ComponentClass, ItemDetail, McpExtra};
use super::names::install_name;

const REGISTRY_BASE: &str = "https://registry.modelcontextprotocol.io/v0/servers";
const PAGE_SIZE: u32 = 30;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
/// What one page can weigh; nothing honest comes near it.
const MAX_BODY: u64 = 4 * 1024 * 1024;

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McpPage {
    pub items: Vec<CatalogItem>,
    pub next_cursor: Option<String>,
}

fn map_server(entry: &Value) -> Option<CatalogItem> {
    let server = entry.get("server")?;
    let name = server.get("name").and_then(Value::as_str)?.to_string();
    let repository = server.get("repository").cloned();
    let remotes = server.get("remotes").and_then(Value::as_array).cloned().unwrap_or_default();
    let packages = server.get("packages").and_then(Value::as_array).cloned().unwrap_or_default();

    // The registry's `name` is a reverse-DNS-style id (`ai.smithery/foo`),
    // useful for the CLI but not for a human — the title, or the last path
    // segment when there is none, reads like an actual name.
    let display_name = server
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| name.rsplit('/').next().unwrap_or(&name).to_string());

    let repo_owner = repository
        .as_ref()
        .and_then(|repo| repo.get("url"))
        .and_then(Value::as_str)
        .and_then(github_owner);
    let icon_url = repo_owner.as_deref().map(|owner| format!("https://github.com/{owner}.png?size=80"));
    // A remote that asks for no header of its own may sign the user in.
    let sign_in = remotes
        .first()
        .is_some_and(|remote| remote.get("headers").and_then(Value::as_array).is_none_or(|headers| headers.is_empty()));
    let extra = McpExtra {
        install_name: install_name(&name),
        source: "official".to_string(),
        remote: Some(!remotes.is_empty()),
        sign_in,
        ..McpExtra::default()
    };

    Some(CatalogItem {
        id: format!("mcp:{name}"),
        class: ComponentClass::Mcp,
        name: name.clone(),
        display_name,
        description: server.get("description").and_then(Value::as_str).unwrap_or_default().to_string(),
        source_id: "mcp-registry:official".to_string(),
        source_label: "MCP Registry".to_string(),
        // The registry has no categories and no download counts at all —
        // unlike Claude's marketplaces, there's no proxy for either, so
        // these stay None rather than showing a fabricated number.
        category: None,
        // Not a published field either, but who publishes the repository is
        // real information the registry does give us — the same signal the
        // icon already uses, just shown as text instead of an avatar.
        author: repo_owner,
        homepage: repository
            .as_ref()
            .and_then(|repo| repo.get("url"))
            .and_then(Value::as_str)
            .map(str::to_string),
        version: server.get("version").and_then(Value::as_str).map(str::to_string),
        tags: Vec::new(),
        install_count: None,
        icon_url,
        rating: None,
        declares_skills: Vec::new(),
        targets: targets_for(ComponentClass::Mcp),
        // Cross-referencing against `claude mcp list` would tell us this,
        // but that's a second CLI round trip per page for a badge — left for
        // later if it turns out to matter.
        installed_in: Vec::new(),
        detail: ItemDetail::Mcp {
            server_name: name,
            remotes,
            packages,
            repository,
        },
        extra,
    })
}

/// Maps a raw registry page into our catalog shape. Pure, so it's testable
/// against a fixture without a network call.
pub fn map_page(payload: &Value) -> McpPage {
    let items = payload
        .get("servers")
        .and_then(Value::as_array)
        .map(|servers| servers.iter().filter_map(map_server).take(PAGE_SIZE as usize * 2).collect())
        .unwrap_or_default();
    let next_cursor = payload
        .get("metadata")
        .and_then(|metadata| metadata.get("nextCursor"))
        .and_then(Value::as_str)
        .map(str::to_string);
    McpPage { items, next_cursor }
}

/// Just enough percent-encoding for a query string: the registry only ever
/// sees a plain search term or an opaque cursor token, never a full URL.
fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// Fetches one page. An empty `query` means "browse everything"; pinning
/// `version=latest` keeps superseded versions of the same server (the
/// registry keeps every one it was ever told about) out of the results.
pub fn search(query: &str, cursor: Option<&str>) -> Result<McpPage, String> {
    let mut url = format!("{REGISTRY_BASE}?limit={PAGE_SIZE}&version=latest");
    if !query.trim().is_empty() {
        url.push_str("&search=");
        url.push_str(&urlencode(query.trim()));
    }
    if let Some(cursor) = cursor {
        url.push_str("&cursor=");
        url.push_str(&urlencode(cursor));
    }

    // Bounded in time and in size: a slow or endless answer can't hold the
    // panel, or the memory, hostage.
    use std::io::Read;
    let agent = ureq::AgentBuilder::new().timeout(TIMEOUT).build();
    let mut body = String::new();
    agent
        .get(&url)
        .call()
        .map_err(|err| tr(Msg::McpRegistryUnreachable(&err.to_string())))?
        .into_reader()
        .take(MAX_BODY)
        .read_to_string(&mut body)
        .map_err(|err| tr(Msg::McpRegistryUnreachable(&err.to_string())))?;
    let payload: Value = serde_json::from_str(&body).map_err(|err| tr(Msg::McpRegistryUnreachable(&err.to_string())))?;
    Ok(map_page(&payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::model::AgentTarget;
    use serde_json::json;

    fn remote_fixture() -> Value {
        json!({
            "servers": [{
                "server": {
                    "name": "ai.smithery/Hint-Services-obsidian-github-mcp",
                    "title": "Obsidian GitHub",
                    "description": "Connects to a vault",
                    "version": "0.4.0",
                    "repository": {
                        "url": "https://github.com/Hint-Services/obsidian-github-mcp",
                        "source": "github"
                    },
                    "remotes": [{
                        "type": "streamable-http",
                        "url": "https://server.smithery.ai/x/mcp",
                        "headers": [{ "name": "Authorization", "isRequired": true, "isSecret": true, "value": "Bearer {key}" }]
                    }]
                },
                "_meta": {}
            }],
            "metadata": { "nextCursor": "abc:1.0.0", "count": 1 }
        })
    }

    #[test]
    fn maps_a_remote_server() {
        let page = map_page(&remote_fixture());
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.next_cursor.as_deref(), Some("abc:1.0.0"));

        let item = &page.items[0];
        assert_eq!(item.id, "mcp:ai.smithery/Hint-Services-obsidian-github-mcp");
        assert_eq!(item.class, ComponentClass::Mcp);
        assert_eq!(item.display_name, "Obsidian GitHub");
        // The registry's id isn't a name Claude Code takes: it gets one.
        assert_eq!(item.extra.install_name, "Hint-Services-obsidian-github-mcp");
        assert_eq!(item.extra.source, "official");
        // It asks for a header: it doesn't sign in by itself.
        assert!(!item.extra.sign_in);
        assert_eq!(item.icon_url.as_deref(), Some("https://github.com/Hint-Services.png?size=80"));
        // Not a field the registry publishes, but who owns the repository
        // is real information — the same signal the icon derives from.
        assert_eq!(item.author.as_deref(), Some("Hint-Services"));
        assert_eq!(
            item.targets,
            vec![AgentTarget::Claude, AgentTarget::Codex, AgentTarget::Gemini, AgentTarget::Copilot, AgentTarget::Grok]
        );
        // No proxy for this one: the registry has no download counts at all.
        assert!(item.install_count.is_none());
        match &item.detail {
            ItemDetail::Mcp { remotes, server_name, .. } => {
                assert_eq!(remotes.len(), 1);
                assert_eq!(server_name, "ai.smithery/Hint-Services-obsidian-github-mcp");
            }
            _ => panic!("expected mcp detail"),
        }
    }

    #[test]
    fn falls_back_to_the_last_path_segment_without_a_title() {
        let mut fixture = remote_fixture();
        fixture["servers"][0]["server"].as_object_mut().unwrap().remove("title");
        let item = &map_page(&fixture).items[0];
        assert_eq!(item.display_name, "Hint-Services-obsidian-github-mcp");
    }

    #[test]
    fn a_page_with_no_servers_is_empty_not_an_error() {
        let page = map_page(&json!({ "servers": [], "metadata": { "count": 0 } }));
        assert!(page.items.is_empty());
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn no_author_shown_without_a_github_repository() {
        let mut fixture = remote_fixture();
        fixture["servers"][0]["server"].as_object_mut().unwrap().remove("repository");
        let item = &map_page(&fixture).items[0];
        assert!(item.author.is_none());
        assert!(item.icon_url.is_none());
    }

    #[test]
    fn a_server_with_no_name_is_skipped_rather_than_crashing() {
        let page = map_page(&json!({ "servers": [{ "server": { "description": "no name" } }] }));
        assert!(page.items.is_empty());
    }

    #[test]
    fn encodes_the_search_term() {
        assert_eq!(urlencode("git hub/x"), "git%20hub%2Fx");
    }
}
