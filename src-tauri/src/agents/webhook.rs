//! The agents' local webhook: `POST http://127.0.0.1:47821/agents/<id>/run`
//! with the agent's token starts a run, and the request's body reaches the
//! agent as context. Only this computer can reach it — it listens on the
//! loopback address alone — and only callers holding the token, sent in a
//! header (never in the URL, where it would end up in shell history and
//! logs), can use it.
//!
//! The same port serves the agents' own MCP server at `/mcp` (see
//! mcp_server.rs).

use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tiny_http::{Header, Method, Response, Server};

use super::hub::Hub;
use super::mcp_server;
use super::ops::Agents;
use super::runner::Runner;
use super::spec::is_valid_id;
use super::store::{self, RunTrigger};

pub const PORT: u16 = 47821;
/// What a call can send as context; the rest is cut.
const MAX_BODY: u64 = 64 * 1024;

static LISTENING: AtomicBool = AtomicBool::new(false);
/// Calls being handled right now. Each gets its own thread, so a client
/// that never finishes sending can only hold up itself; past this many, new
/// calls are turned away instead of piling up threads.
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
const MAX_IN_FLIGHT: usize = 8;
/// The same for agents' MCP calls, counted apart: asking another agent
/// holds a call open for minutes, and that mustn't turn webhooks away.
static MCP_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
const MAX_MCP_IN_FLIGHT: usize = 16;

/// The same, said by the agents' own process when they run there (see
/// host/client.rs): consoles the app opens get the server only if it's up.
static ELSEWHERE: AtomicBool = AtomicBool::new(false);

/// Whether the webhook could take its port (another program may hold it),
/// here or in the agents' process.
pub fn listening() -> bool {
    if super::host::runs_here() {
        LISTENING.load(Ordering::Relaxed)
    } else {
        ELSEWHERE.load(Ordering::Relaxed)
    }
}

pub fn set_listening_elsewhere(listening: bool) {
    ELSEWHERE.store(listening, Ordering::Relaxed);
}

/// Where agent runs reach Termo's MCP server.
pub fn mcp_url() -> String {
    format!("http://127.0.0.1:{PORT}{}", mcp_server::PATH)
}

/// The agent a request is for (`/agents/<id>/run`). A query string is
/// ignored — a token there doesn't count.
pub fn parse_target(url: &str) -> Option<String> {
    let path = url.split_once('?').map_or(url, |(path, _)| path);
    let mut parts = path.trim_matches('/').split('/');
    let (Some("agents"), Some(id), Some("run"), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    is_valid_id(id).then(|| id.to_string())
}

/// Compares without stopping at the first difference, so the time a wrong
/// token takes says nothing about how much of it was right.
pub fn token_matches(given: &str, expected: &str) -> bool {
    if expected.is_empty() || given.len() != expected.len() {
        return false;
    }
    given.bytes().zip(expected.bytes()).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}

fn header(request: &tiny_http::Request, name: &str) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|header| header.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str().to_string())
}

fn reply(request: tiny_http::Request, status: u16, body: serde_json::Value) {
    let content_type = Header::from_bytes("Content-Type", "application/json").expect("valid header");
    let _ = request.respond(Response::from_string(body.to_string()).with_status_code(status).with_header(content_type));
}

/// The server while it's up, so `stop` can close it.
static SERVER: Mutex<Option<Arc<Server>>> = Mutex::new(None);

/// Listens until `stop` (Modo IA turned off; see ai_mode.rs).
pub fn start(agents: Agents) {
    std::thread::spawn(move || {
        let root = agents.hub.root().to_path_buf();
        // Turned back on right after being turned off, the old socket may
        // take a moment to be let go.
        let Some(server) = (0..10).find_map(|attempt| {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(200));
            }
            Server::http(("127.0.0.1", PORT)).ok()
        }) else {
            return;
        };
        let server = Arc::new(server);
        {
            let mut slot = SERVER.lock().unwrap();
            // Turned off again while it was waiting for the port.
            if !crate::ai_mode::enabled() {
                return;
            }
            *slot = Some(server.clone());
        }
        LISTENING.store(true, Ordering::Relaxed);
        // Ends when `stop` unblocks the server.
        for request in server.incoming_requests() {
            let mcp = mcp_server::is_mcp(request.url());
            let (counter, max) = if mcp { (&MCP_IN_FLIGHT, MAX_MCP_IN_FLIGHT) } else { (&IN_FLIGHT, MAX_IN_FLIGHT) };
            if counter.fetch_add(1, Ordering::SeqCst) >= max {
                counter.fetch_sub(1, Ordering::SeqCst);
                reply(request, 503, json!({ "error": "busy" }));
                continue;
            }
            let (agents, root) = (agents.clone(), root.clone());
            std::thread::spawn(move || {
                if mcp {
                    mcp_server::handle(request, &agents, &root);
                } else {
                    handle(request, &agents.hub, &agents.runner, &root);
                }
                counter.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
}

/// Closes the port: the accept loop above ends, and the socket goes with the
/// last reference to the server. Calls already being handled finish.
pub fn stop() {
    if let Some(server) = SERVER.lock().unwrap().take() {
        LISTENING.store(false, Ordering::Relaxed);
        server.unblock();
    }
}

fn handle(mut request: tiny_http::Request, hub: &Hub, runner: &Arc<Runner>, root: &std::path::Path) {
    if request.method() == &Method::Get && request.url() == "/health" {
        return reply(request, 200, json!({ "ok": true }));
    }
    let Some(id) = parse_target(request.url()) else {
        return reply(request, 404, json!({ "error": "not found" }));
    };
    if request.method() != &Method::Post {
        return reply(request, 405, json!({ "error": "use POST" }));
    }
    let token = header(&request, "Authorization")
        .and_then(|value| value.strip_prefix("Bearer ").map(str::to_string))
        .or_else(|| header(&request, "X-Termo-Token"))
        .unwrap_or_default();
    // An unknown agent and a wrong token answer the same, so the webhook
    // doesn't say which agents exist.
    let Some(spec) = store::get_in(root, &id)
        .filter(|spec| spec.triggers.webhook.as_ref().is_some_and(|hook| token_matches(&token, &hook.token)))
    else {
        return reply(request, 404, json!({ "error": "not found" }));
    };
    if !spec.enabled {
        return reply(request, 409, json!({ "error": "agent paused" }));
    }
    let mut body = String::new();
    let _ = request.as_reader().take(MAX_BODY).read_to_string(&mut body);
    match runner.start(hub, &spec, RunTrigger::Webhook, Some(&body)) {
        Ok(run) => reply(request, 202, json!({ "runId": run.id })),
        Err(error) => reply(request, 409, json!({ "error": error })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_agent_and_ignores_the_query() {
        assert_eq!(parse_target("/agents/a1/run"), Some("a1".into()));
        assert_eq!(parse_target("/agents/a1/run?token=xyz"), Some("a1".into()));
        assert_eq!(parse_target("/agents/a1"), None);
        assert_eq!(parse_target("/agents/../run"), None);
        assert_eq!(parse_target("/agents/a1/run/extra"), None);
    }

    #[test]
    fn tokens_must_match_exactly() {
        assert!(token_matches("abc", "abc"));
        assert!(!token_matches("abd", "abc"));
        assert!(!token_matches("ab", "abc"));
        // An agent with no token set can't be called at all.
        assert!(!token_matches("", ""));
    }
}
