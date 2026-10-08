//! Smithery (https://smithery.ai) as a catalog source: its registry API for
//! searching and for a server's details — its tools, how it's reached and
//! what it needs configured.
//!
//! Searching and reading a server's details work without a key today,
//! though Smithery's documentation asks for one: the user's own key (kept
//! in the keychain, see sources.rs) is sent when there is one, and is what
//! to add if Smithery ever turns a request without it down.
//! A search lists servers without how to connect to them; that comes with a
//! server's details, fetched when the user opens it. A server Smithery hosts
//! is reached at its own URL and signs the user in with OAuth, which every
//! agent's CLI does by itself; what it needs besides that — an API key of
//! the service behind it, say — is described by a JSON Schema, read here
//! into the same header and query-parameter specs the MCP registry's
//! entries carry, so one installer serves both.

use std::time::Duration;

use serde_json::{json, Value};

use crate::i18n::{tr, Msg};

use super::model::{targets_for, CatalogItem, ComponentClass, ItemDetail, McpExtra, ToolInfo};
use super::names::install_name;

const API: &str = "https://api.smithery.ai";
pub const SITE: &str = "https://smithery.ai";
const PAGE_SIZE: u32 = 30;
const TIMEOUT: Duration = Duration::from_secs(20);
/// What one answer can weigh: a server with hundreds of tools is a few
/// hundred KB; nothing honest comes near this.
const MAX_BODY: u64 = 4 * 1024 * 1024;
const MAX_TOOLS: usize = 200;
const MAX_FIELDS: usize = 40;

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().trim().to_string()
}

/// Only http(s) links leave here: a catalog entry doesn't get to hand the
/// UI another scheme to open or load.
fn web_url(url: &str) -> Option<String> {
    let url = url.trim();
    (url.starts_with("https://") || url.starts_with("http://")).then(|| url.to_string())
}

/// Just enough percent-encoding for a query string or a path segment.
pub fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// A server's page on Smithery: where each result links back to.
pub fn listing_url(qualified_name: &str) -> String {
    let path: Vec<String> = qualified_name.split('/').map(urlencode).collect();
    format!("{SITE}/servers/{}", path.join("/"))
}

fn base_item(entry: &Value) -> Option<CatalogItem> {
    let name = text(entry, "qualifiedName");
    if name.is_empty() {
        return None;
    }
    let display_name = Some(text(entry, "displayName")).filter(|title| !title.is_empty()).unwrap_or_else(|| name.clone());
    Some(CatalogItem {
        id: format!("smithery:{name}"),
        class: ComponentClass::Mcp,
        name: name.clone(),
        display_name: clip(&display_name, 120),
        description: clip(&text(entry, "description"), 2000),
        source_id: "smithery".to_string(),
        source_label: "Smithery".to_string(),
        category: None,
        author: entry.get("namespace").and_then(Value::as_str).filter(|owner| !owner.is_empty()).map(str::to_string),
        homepage: web_url(&text(entry, "homepage")),
        version: None,
        tags: Vec::new(),
        install_count: entry.get("useCount").and_then(Value::as_u64),
        icon_url: web_url(&text(entry, "iconUrl")),
        rating: None,
        declares_skills: Vec::new(),
        targets: targets_for(ComponentClass::Mcp),
        installed_in: Vec::new(),
        detail: ItemDetail::Mcp { server_name: name.clone(), remotes: Vec::new(), packages: Vec::new(), repository: None },
        extra: McpExtra {
            install_name: install_name(&name),
            source: "smithery".to_string(),
            listing_url: Some(listing_url(&name)),
            verified: entry.get("verified").and_then(Value::as_bool),
            remote: entry.get("remote").and_then(Value::as_bool),
            // How to connect comes with the details.
            needs_detail: true,
            ..McpExtra::default()
        },
    })
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub items: Vec<CatalogItem>,
    pub next_cursor: Option<String>,
}

/// A page of search results in the catalog's shape. Servers their owner
/// switched off aren't listed: there's nothing to connect to.
pub fn map_page(payload: &Value) -> Page {
    let items = payload
        .get("servers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| entry.get("inactive").and_then(Value::as_bool) != Some(true))
        .filter_map(base_item)
        // A page is as many as was asked for, whatever the answer holds.
        .take(PAGE_SIZE as usize * 2)
        .collect();
    let pagination = payload.get("pagination");
    let number = |key: &str| pagination.and_then(|pagination| pagination.get(key)).and_then(Value::as_u64).unwrap_or(0);
    let (current, total) = (number("currentPage"), number("totalPages"));
    Page { items, next_cursor: (current > 0 && current < total).then(|| (current + 1).to_string()) }
}

/// Whether a field holds something to keep out of sight: said by the
/// schema, or by its name.
fn is_secret(name: &str, property: &Value) -> bool {
    if property.get("format").and_then(Value::as_str) == Some("password") {
        return true;
    }
    let lower = name.to_lowercase();
    ["key", "token", "secret", "password", "passwd", "credential", "authorization"].iter().any(|word| lower.contains(word))
}

fn is_field_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 100 && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A connection's JSON Schema as header and query-parameter specs, in the
/// MCP registry's shape. `x-from` says where the server reads each value
/// from — a header or the query string; a field that doesn't say is a
/// header by its own name, which is how Smithery's servers take their
/// settings.
pub fn fields_of(schema: &Value) -> (Vec<Value>, Vec<Value>) {
    let required: Vec<&str> =
        schema.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let mut properties: Vec<(&String, &Value)> =
        schema.get("properties").and_then(Value::as_object).into_iter().flatten().collect();
    properties.sort_by_key(|(name, property)| (property.get("x-order").and_then(Value::as_i64).unwrap_or(i64::MAX), (*name).clone()));
    let (mut headers, mut query) = (Vec::new(), Vec::new());
    for (name, property) in properties.into_iter().take(MAX_FIELDS) {
        let from = property.get("x-from");
        let named = |place: &str| from.and_then(|from| from.get(place)).and_then(Value::as_str).map(str::to_string);
        let (target, sent_as) = match (named("header"), named("query")) {
            (Some(header), _) => (&mut headers, header),
            (None, Some(param)) => (&mut query, param),
            (None, None) => (&mut headers, name.clone()),
        };
        if !is_field_name(&sent_as) {
            continue;
        }
        let secret = is_secret(name, property);
        let mut spec = json!({
            "name": sent_as,
            "description": clip(&text(property, "description"), 400),
            "isRequired": required.contains(&name.as_str()),
            "isSecret": secret,
        });
        // A default is a real value to start from — never for a secret.
        if !secret {
            let default = match property.get("default") {
                Some(Value::String(value)) => Some(value.clone()),
                Some(Value::Number(value)) => Some(value.to_string()),
                Some(Value::Bool(value)) => Some(value.to_string()),
                _ => None,
            };
            if let Some(default) = default {
                spec["default"] = json!(default);
            }
        }
        target.push(spec);
    }
    (headers, query)
}

/// A server's details in the catalog's shape: how to reach it, what it
/// needs, the tools it brings and whether Smithery's scan passed.
pub fn map_detail(payload: &Value) -> Option<CatalogItem> {
    let mut item = base_item(payload)?;
    let connections: Vec<&Value> = payload.get("connections").and_then(Value::as_array).into_iter().flatten().collect();
    let kind = |connection: &&Value| connection.get("type").and_then(Value::as_str).unwrap_or_default().to_string();
    let remotes: Vec<Value> = connections
        .iter()
        .filter(|connection| kind(connection) == "http")
        .filter_map(|connection| {
            let url = web_url(&text(connection, "deploymentUrl"))?;
            let (headers, query) = fields_of(connection.get("configSchema").unwrap_or(&Value::Null));
            Some(json!({ "type": "streamable-http", "url": url, "headers": headers, "queryParams": query }))
        })
        .collect();
    let tools = payload
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            let name = text(tool, "name");
            (!name.is_empty()).then(|| ToolInfo { name: clip(&name, 100), description: clip(&text(tool, "description"), 300) })
        })
        .take(MAX_TOOLS)
        .collect();
    // Only a bundle to run here, with Smithery's own runner: not installed
    // from Termo — the listing says how.
    item.extra.local_only = remotes.is_empty() && connections.iter().any(|connection| kind(connection) == "stdio");
    item.extra.remote = Some(!remotes.is_empty());
    item.extra.needs_detail = false;
    item.extra.tools = tools;
    item.extra.security_scan = payload.pointer("/security/scanPassed").and_then(Value::as_bool);
    // A hosted server signs the user in itself.
    item.extra.sign_in = !remotes.is_empty();
    if let ItemDetail::Mcp { remotes: slot, .. } = &mut item.detail {
        *slot = remotes;
    }
    Some(item)
}

fn get(path: &str, key: Option<&str>) -> Result<Value, String> {
    // No redirects followed: the key goes to Smithery's API and nowhere a
    // response might point to.
    let agent = ureq::AgentBuilder::new().timeout(TIMEOUT).redirects(0).build();
    let mut request = agent.get(&format!("{API}{path}"));
    if let Some(key) = key {
        request = request.set("Authorization", &format!("Bearer {key}"));
    }
    let response = match request.call() {
        Ok(response) => response,
        Err(ureq::Error::Status(401 | 403, _)) => {
            return Err(tr(if key.is_some() { Msg::SmitheryKeyRejected } else { Msg::SourceKeyMissing }));
        }
        Err(ureq::Error::Status(404, _)) => return Err(tr(Msg::SmitheryNotFound)),
        Err(ureq::Error::Status(429, _)) => return Err(tr(Msg::SmitheryRateLimited)),
        // Never the error's own text when it could carry the request: the
        // key rides in a header, but nothing of the request is echoed.
        Err(ureq::Error::Status(code, _)) => return Err(tr(Msg::SmitheryUnreachable(&format!("HTTP {code}")))),
        Err(ureq::Error::Transport(transport)) => return Err(tr(Msg::SmitheryUnreachable(&transport.kind().to_string()))),
    };
    let mut body = String::new();
    use std::io::Read;
    response
        .into_reader()
        .take(MAX_BODY)
        .read_to_string(&mut body)
        .map_err(|err| tr(Msg::SmitheryUnreachable(&err.kind().to_string())))?;
    serde_json::from_str(&body).map_err(|_| tr(Msg::SmitheryUnreachable("invalid answer")))
}

/// One page of a search; an empty query browses. `cursor` is the page
/// number as the previous page gave it.
pub fn search(key: Option<&str>, query: &str, cursor: Option<&str>) -> Result<Page, String> {
    let page = cursor.and_then(|cursor| cursor.parse::<u32>().ok()).filter(|page| (1..=10_000).contains(page)).unwrap_or(1);
    let mut path = format!("/servers?pageSize={PAGE_SIZE}&page={page}");
    if !query.trim().is_empty() {
        path.push_str("&q=");
        path.push_str(&urlencode(query.trim()));
    }
    Ok(map_page(&get(&path, key)?))
}

/// A server's details, by its qualified name.
pub fn detail(key: Option<&str>, qualified_name: &str) -> Result<CatalogItem, String> {
    let payload = get(&format!("/servers/{}", urlencode(qualified_name.trim())), key)?;
    map_detail(&payload).ok_or_else(|| tr(Msg::SmitheryNotFound))
}

/// Whether the key works. Asked of an endpoint that needs one: the search
/// answers the same with a wrong key as with none.
pub fn test(key: &str) -> Result<(), String> {
    get("/namespaces", Some(key)).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shapes as the API answered on 2026-10-07 (trimmed).
    fn list_fixture() -> Value {
        json!({
            "servers": [
                { "id": "944c", "qualifiedName": "github", "namespace": "github", "slug": "", "displayName": "GitHub",
                  "description": "Connect your AI agents to GitHub", "iconUrl": "https://api.smithery.ai/servers/github/icon",
                  "verified": true, "useCount": 6059, "remote": true, "isDeployed": true, "inactive": false,
                  "homepage": "https://github.com/", "bySmithery": true },
                { "id": "f50b", "qualifiedName": "docfork/docfork", "namespace": "docfork", "displayName": "Docfork",
                  "description": "Docs", "verified": true, "useCount": 254, "remote": true, "inactive": true,
                  "homepage": "javascript:alert(1)" },
                { "id": "469c", "qualifiedName": "vercel/grep", "namespace": "vercel", "slug": "grep", "displayName": "Vercel Grep",
                  "description": "Search code", "iconUrl": "data:text/html,x", "verified": false, "useCount": 1219,
                  "remote": true, "inactive": false, "homepage": "https://smithery.ai/servers/vercel/grep" }
            ],
            "pagination": { "currentPage": 1, "pageSize": 30, "totalPages": 35, "totalCount": 139 }
        })
    }

    #[test]
    fn maps_a_page_and_leaves_out_what_was_switched_off() {
        let page = map_page(&list_fixture());
        assert_eq!(page.next_cursor.as_deref(), Some("2"));
        let names: Vec<&str> = page.items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, ["github", "vercel/grep"]);
        let github = &page.items[0];
        assert_eq!(github.id, "smithery:github");
        assert_eq!(github.install_count, Some(6059));
        assert_eq!(github.extra.verified, Some(true));
        assert_eq!(github.extra.install_name, "github");
        assert_eq!(github.extra.listing_url.as_deref(), Some("https://smithery.ai/servers/github"));
        assert!(github.extra.needs_detail);
        let grep = &page.items[1];
        assert_eq!(grep.extra.install_name, "vercel-grep");
        assert_eq!(grep.extra.listing_url.as_deref(), Some("https://smithery.ai/servers/vercel/grep"));
        // Not a web address: not an icon to load.
        assert!(grep.icon_url.is_none());
    }

    #[test]
    fn the_last_page_has_no_next() {
        let mut fixture = list_fixture();
        fixture["pagination"]["currentPage"] = json!(35);
        assert!(map_page(&fixture).next_cursor.is_none());
        assert!(map_page(&json!({})).items.is_empty());
    }

    #[test]
    fn a_hosted_server_is_a_remote_that_signs_in() {
        let payload = json!({
            "qualifiedName": "exa", "displayName": "Exa Search", "description": "Search", "remote": true,
            "deploymentUrl": "https://exa.run.tools",
            "connections": [{ "type": "http", "deploymentUrl": "https://exa.run.tools", "configSchema": {} }],
            "security": null,
            "tools": [{ "name": "web_search_exa", "description": "Search the web" }, { "description": "no name" }]
        });
        let item = map_detail(&payload).unwrap();
        assert!(!item.extra.needs_detail && !item.extra.local_only && item.extra.sign_in);
        assert_eq!(item.extra.tools, vec![ToolInfo { name: "web_search_exa".into(), description: "Search the web".into() }]);
        assert_eq!(item.extra.security_scan, None);
        match &item.detail {
            ItemDetail::Mcp { remotes, .. } => {
                assert_eq!(remotes[0]["url"], "https://exa.run.tools");
                assert_eq!(remotes[0]["type"], "streamable-http");
                assert!(remotes[0]["headers"].as_array().unwrap().is_empty());
            }
            _ => panic!("expected mcp detail"),
        }
    }

    /// The schema says where each setting goes; a secret never comes with
    /// a default to prefill.
    #[test]
    fn settings_become_headers_and_query_parameters() {
        let schema = json!({
            "type": "object",
            "required": ["BUTTERBASE_API_KEY"],
            "properties": {
                "BUTTERBASE_API_KEY": { "type": "string", "x-to": { "header": "Authorization" },
                    "x-from": { "header": "BUTTERBASE_API_KEY" }, "x-order": 0, "description": "Your API key.", "default": "leak" },
                "projectId": { "type": "string", "x-from": { "query": "projectId" }, "x-order": 1, "default": "main" },
                "region": { "type": "string", "x-order": 2 },
                "retries": { "type": "number", "default": 3, "x-order": 3 },
                "bad name!": { "type": "string" }
            }
        });
        let (headers, query) = fields_of(&schema);
        assert_eq!(headers.len(), 3);
        assert_eq!(headers[0]["name"], "BUTTERBASE_API_KEY");
        assert_eq!(headers[0]["isRequired"], true);
        assert_eq!(headers[0]["isSecret"], true);
        assert!(headers[0].get("default").is_none());
        assert_eq!(headers[1]["name"], "region");
        assert_eq!(headers[2]["default"], "3");
        assert_eq!(query.len(), 1);
        assert_eq!(query[0]["name"], "projectId");
        assert_eq!(query[0]["default"], "main");
        assert!(fields_of(&json!({})).0.is_empty());
    }

    #[test]
    fn a_bundle_to_run_here_is_not_installed_from_termo() {
        let payload = json!({
            "qualifiedName": "dpflucas/mysql-mcp-server", "displayName": "MySQL",
            "connections": [{ "type": "stdio", "bundleUrl": "https://backend.smithery.ai/x/server.mcpb", "runtime": "node",
                "configSchema": { "properties": { "mysqlHost": { "type": "string" } } } }],
            "security": { "scanPassed": true }
        });
        let item = map_detail(&payload).unwrap();
        assert!(item.extra.local_only && !item.extra.sign_in);
        assert_eq!(item.extra.security_scan, Some(true));
        assert_eq!(item.extra.remote, Some(false));
        match &item.detail {
            ItemDetail::Mcp { remotes, packages, .. } => assert!(remotes.is_empty() && packages.is_empty()),
            _ => panic!("expected mcp detail"),
        }
    }

    /// A deployment address that isn't http(s) is not a remote to offer.
    #[test]
    fn only_web_addresses_are_remotes() {
        let payload = json!({
            "qualifiedName": "x/y",
            "connections": [{ "type": "http", "deploymentUrl": "file:///etc/passwd", "configSchema": {} }]
        });
        let item = map_detail(&payload).unwrap();
        match &item.detail {
            ItemDetail::Mcp { remotes, .. } => assert!(remotes.is_empty()),
            _ => panic!("expected mcp detail"),
        }
        assert!(map_detail(&json!({ "displayName": "no name" })).is_none());
    }

    /// Against the real API, with a key from the environment (never one
    /// written here): `SMITHERY_TEST_KEY=… cargo test --lib smithery -- --ignored`.
    #[test]
    #[ignore = "needs SMITHERY_TEST_KEY and the network"]
    fn live_search_and_details() {
        let key = std::env::var("SMITHERY_TEST_KEY").expect("SMITHERY_TEST_KEY");
        test(&key).expect("the key works");
        let key = Some(key.as_str());
        let page = search(key, "github", None).expect("a page");
        assert!(!page.items.is_empty());
        assert!(page.items.iter().all(|item| item.extra.needs_detail && !item.extra.install_name.is_empty()));
        assert!(page.next_cursor.is_some());
        let second = search(key, "github", page.next_cursor.as_deref()).expect("the next page");
        assert_ne!(second.items[0].name, page.items[0].name);
        let exa = detail(key, "exa").expect("exa's details");
        assert!(!exa.extra.needs_detail && exa.extra.sign_in && !exa.extra.tools.is_empty());
        let grep = detail(key, "vercel/grep").expect("a name with a slash");
        assert_eq!(grep.extra.install_name, "vercel-grep");
        assert!(matches!(test("not-a-real-key-000000"), Err(error) if error == tr(Msg::SmitheryKeyRejected)));
        assert!(detail(key, "no-such-owner/no-such-server-xyz").is_err());
        // Without a key, for as long as Smithery answers that way.
        assert!(!search(None, "github", None).expect("a page without a key").items.is_empty());
    }

    #[test]
    fn names_are_encoded_into_the_path() {
        assert_eq!(urlencode("vercel/grep"), "vercel%2Fgrep");
        assert_eq!(urlencode("a b?c=d&e"), "a%20b%3Fc%3Dd%26e");
    }
}
