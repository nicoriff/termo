//! Why a run failed, said precisely: which service answered (the provider's
//! API behind each CLI, or nothing remote at all), with which HTTP status,
//! what kind of failure that is, and what to do about it. The CLIs only print
//! a line of text for it, each in its own words; this reads those words (and
//! what the CLI said when it started, see events.rs) into one shape the
//! console, the agent's view and the orchestration all show the same way.
//!
//! Plain data and pure functions: the words are localized in the frontend.

use serde::{Deserialize, Serialize};

use super::spec::Engine;

/// Who answered with the error.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    /// Anthropic's API, through Claude Code.
    Anthropic,
    /// OpenAI's API, through Codex.
    Openai,
    /// Google's Gemini API, through Gemini CLI.
    Google,
    /// GitHub Copilot's service, through Copilot CLI.
    Github,
    /// xAI's API, through Grok.
    Xai,
    /// Nothing remote: the CLI itself, this computer, or Termo.
    #[default]
    Local,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum FailureKind {
    /// Not logged in, a key that's wrong or expired.
    Auth,
    /// Logged in, but not allowed to use that model (or it doesn't exist).
    ModelAccess,
    /// Too many requests, or the plan's usage limit for now.
    RateLimit,
    /// Out of credit or quota: billing.
    Quota,
    /// The service is overloaded: try again in a while.
    Overloaded,
    /// Some other error on the service's side.
    Server,
    /// The request was refused as malformed (too long, bad parameter).
    BadRequest,
    /// Couldn't reach the service.
    Network,
    /// The run ran out of time.
    Timeout,
    /// The CLI ended with an error and said nothing more.
    #[default]
    Unknown,
}

impl FailureKind {
    /// Failures of the account or the service, not of the work: retrying
    /// the task won't help until something changes outside it.
    pub fn blocks_account(self) -> bool {
        matches!(self, FailureKind::Auth | FailureKind::ModelAccess | FailureKind::RateLimit | FailureKind::Quota)
    }
}

/// What a failed run's error was, as precisely as its CLI said.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Failure {
    pub provider: Provider,
    /// The CLI that ran it.
    pub engine: Engine,
    pub kind: FailureKind,
    /// The HTTP status the service answered with, when it said.
    pub status: Option<u16>,
    /// The error as the CLI printed it.
    pub message: String,
    /// The provider's id for the request, for its support, when given.
    pub request_id: String,
    /// The model it was asked to use (as the CLI reported it, if it did).
    pub model: String,
    pub exit_code: Option<i32>,
    /// The last lines the CLI printed on stderr.
    pub stderr: Vec<String>,
}

/// Who answered, by name: the provider and the CLI it went through.
pub fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Anthropic => "Anthropic (Claude Code)",
        Provider::Openai => "OpenAI (Codex)",
        Provider::Google => "Google (Gemini CLI)",
        Provider::Github => "GitHub (Copilot CLI)",
        Provider::Xai => "xAI (Grok)",
        Provider::Local => "CLI",
    }
}

/// What a run left to explain its failure.
#[derive(Debug, Default)]
pub struct Facts<'a> {
    pub engine: Engine,
    pub model: &'a str,
    /// Errors the CLI reported in its output, in order.
    pub errors: &'a [String],
    pub stderr: &'a [String],
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

fn provider_of(engine: Engine) -> Provider {
    match engine {
        Engine::Claude => Provider::Anthropic,
        Engine::Codex => Provider::Openai,
        Engine::Gemini => Provider::Google,
        Engine::Copilot => Provider::Github,
        Engine::Grok => Provider::Xai,
    }
}

/// `text[start..end]`, both ends moved in to the nearest character
/// boundary: the window around a status can't split a character.
fn window(text: &str, mut start: usize, mut end: usize) -> &str {
    end = end.min(text.len());
    while start < end && !text.is_char_boundary(start) {
        start += 1;
    }
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[start..end]
}

/// The first HTTP error status the text names: "API Error: 403", "status
/// 429", "401 Unauthorized", `"code": 503`…
pub fn status_in(text: &str) -> Option<u16> {
    // ASCII-only lowercasing keeps every byte where it was.
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut best: Option<(usize, u16)> = None;
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let digits = &bytes[index..index + 3];
        let bounded = (index == 0 || !bytes[index - 1].is_ascii_alphanumeric())
            && (index + 3 == bytes.len() || !bytes[index + 3].is_ascii_alphanumeric());
        if bounded && digits.iter().all(u8::is_ascii_digit) && matches!(digits[0], b'4' | b'5') {
            let code: u16 = std::str::from_utf8(digits).ok()?.parse().ok()?;
            let before = window(&lower, index.saturating_sub(24), index);
            let after = window(&lower, index + 3, index + 24);
            let named = ["error", "status", "http", "code", "api"].iter().any(|word| before.contains(word))
                || [
                    "unauthorized",
                    "forbidden",
                    "not found",
                    "too many",
                    "overloaded",
                    "internal",
                    "bad request",
                    "service unavailable",
                    "bad gateway",
                    "gateway",
                ]
                .iter()
                .any(|word| after.trim_start_matches([' ', ':', '-']).starts_with(word));
            if named {
                best = Some((index, code));
                break;
            }
        }
        index += 1;
    }
    best.map(|(_, code)| code)
}

/// The provider's request id, when the text carries one: Anthropic's
/// `req_…`, or a `request id` / `request_id` / `x-request-id` field.
pub fn request_id_in(text: &str) -> String {
    let token = |rest: &str| -> String {
        rest.trim_start_matches(|c: char| c == '"' || c == '\'' || c == ':' || c == '=' || c.is_whitespace())
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect()
    };
    if let Some(at) = text.find("req_") {
        let id = token(&text[at..]);
        if id.len() > 8 {
            return id;
        }
    }
    // Positions found in it are positions in `text` (see status_in).
    let lower = text.to_ascii_lowercase();
    for key in ["x-request-id", "request_id", "request-id", "requestid", "request id"] {
        if let Some(at) = lower.find(key) {
            let id = token(&text[at + key.len()..]);
            if id.len() >= 8 {
                return id;
            }
        }
    }
    String::new()
}

fn has(text: &str, words: &[&str]) -> bool {
    words.iter().any(|word| text.contains(word))
}

/// The kind of failure, from the status and the words around it.
pub fn kind_of(status: Option<u16>, text: &str) -> FailureKind {
    let lower = text.to_lowercase();
    let model_words = has(&lower, &["model", "access grant", "not available for", "not supported"]);
    if has(&lower, &["insufficient_quota", "credit balance", "billing", "out of credits", "exceeded your current quota", "quota exceeded", "quota_exceeded"]) {
        return FailureKind::Quota;
    }
    if status == Some(429) || has(&lower, &["rate limit", "rate_limit", "usage limit", "too many requests", "limit reached", "resource_exhausted", "resource exhausted"]) {
        return FailureKind::RateLimit;
    }
    match status {
        Some(401) => return FailureKind::Auth,
        Some(403) => return if model_words { FailureKind::ModelAccess } else { FailureKind::Auth },
        Some(404) if model_words => return FailureKind::ModelAccess,
        Some(529) => return FailureKind::Overloaded,
        Some(code) if code >= 500 => {
            return if has(&lower, &["overloaded"]) { FailureKind::Overloaded } else { FailureKind::Server };
        }
        Some(400) | Some(413) | Some(422) => return FailureKind::BadRequest,
        _ => {}
    }
    if has(&lower, &["overloaded"]) {
        return FailureKind::Overloaded;
    }
    if has(
        &lower,
        &[
            "failed to authenticate",
            "invalid api key",
            "invalid x-api-key",
            "not logged in",
            "please run /login",
            "login required",
            "authentication",
            "unauthorized",
            "oauth token",
            "token has expired",
            "credentials",
        ],
    ) {
        return if model_words && has(&lower, &["access"]) { FailureKind::ModelAccess } else { FailureKind::Auth };
    }
    if has(&lower, &["model"]) && has(&lower, &["not found", "does not exist", "not available", "no access", "not supported", "unknown model"]) {
        return FailureKind::ModelAccess;
    }
    if has(
        &lower,
        &[
            "enotfound",
            "econnrefused",
            "econnreset",
            "etimedout",
            "eai_again",
            "fetch failed",
            "network error",
            "connection error",
            "connection refused",
            "could not resolve",
            "dns",
            "socket hang up",
            "unable to connect",
        ],
    ) {
        return FailureKind::Network;
    }
    FailureKind::Unknown
}

/// Why the run failed, from what it left.
pub fn diagnose(facts: &Facts) -> Failure {
    let stderr: Vec<String> = facts.stderr.iter().rev().take(12).rev().cloned().collect();
    let base = Failure {
        engine: facts.engine,
        model: facts.model.to_string(),
        exit_code: facts.exit_code,
        stderr: stderr.clone(),
        ..Failure::default()
    };
    if facts.timed_out {
        return Failure { kind: FailureKind::Timeout, ..base };
    }
    // The CLI's own words first; stderr when it said nothing else.
    let said: Vec<&String> = facts.errors.iter().filter(|text| !text.trim().is_empty()).collect();
    let message = said.last().map(|text| text.trim().to_string()).or_else(|| stderr.last().cloned()).unwrap_or_default();
    let everything = said.iter().map(|text| text.as_str()).chain(stderr.iter().map(String::as_str)).collect::<Vec<_>>().join("\n");
    let status = status_in(&message).or_else(|| status_in(&everything));
    let kind = kind_of(status, &everything);
    // Something answered from the other side: a status, or a kind only a
    // service gives. Otherwise the CLI failed here.
    let remote = status.is_some()
        || matches!(
            kind,
            FailureKind::Auth
                | FailureKind::ModelAccess
                | FailureKind::RateLimit
                | FailureKind::Quota
                | FailureKind::Overloaded
                | FailureKind::Server
        );
    Failure {
        provider: if remote { provider_of(facts.engine) } else { Provider::Local },
        kind,
        status,
        request_id: request_id_in(&everything),
        message,
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(engine: Engine, errors: &'a [String], stderr: &'a [String]) -> Facts<'a> {
        Facts { engine, model: "m", errors, stderr, exit_code: Some(1), timed_out: false }
    }

    /// The case that started this: Claude Code, Opus without the grant.
    #[test]
    fn claude_without_access_to_a_model() {
        let errors = ["Failed to authenticate. API Error: 403 Access to this model requires an access grant your request does not have.".to_string()];
        let failure = diagnose(&facts(Engine::Claude, &errors, &[]));
        assert_eq!(failure.provider, Provider::Anthropic);
        assert_eq!(failure.status, Some(403));
        assert_eq!(failure.kind, FailureKind::ModelAccess);
        assert!(failure.kind.blocks_account());
        assert!(failure.message.starts_with("Failed to authenticate"));
    }

    #[test]
    fn statuses_are_found_where_they_are_named() {
        assert_eq!(status_in("API Error: 529 {\"type\":\"overloaded_error\"}"), Some(529));
        assert_eq!(status_in("unexpected status 401 Unauthorized: invalid key"), Some(401));
        assert_eq!(status_in("{\"error\":{\"code\":429,\"status\":\"RESOURCE_EXHAUSTED\"}}"), Some(429));
        assert_eq!(status_in("HTTP 500 Internal Server Error"), Some(500));
        // Numbers that aren't statuses.
        assert_eq!(status_in("edited 403 files in 2024"), None);
        assert_eq!(status_in("port 4000 and id a404b"), None);
    }

    #[test]
    fn each_kind_from_its_words() {
        assert_eq!(kind_of(Some(401), ""), FailureKind::Auth);
        assert_eq!(kind_of(Some(403), "Forbidden"), FailureKind::Auth);
        assert_eq!(kind_of(Some(404), "model: claude-x not found"), FailureKind::ModelAccess);
        assert_eq!(kind_of(Some(429), ""), FailureKind::RateLimit);
        assert_eq!(kind_of(None, "Claude AI usage limit reached|1791273600"), FailureKind::RateLimit);
        assert_eq!(kind_of(Some(429), "You exceeded your current quota, check your billing"), FailureKind::Quota);
        assert_eq!(kind_of(None, "Your credit balance is too low"), FailureKind::Quota);
        assert_eq!(kind_of(Some(529), ""), FailureKind::Overloaded);
        assert_eq!(kind_of(Some(502), "Bad Gateway"), FailureKind::Server);
        assert_eq!(kind_of(Some(400), "prompt is too long"), FailureKind::BadRequest);
        assert_eq!(kind_of(None, "Invalid API key · Please run /login"), FailureKind::Auth);
        assert_eq!(kind_of(None, "request failed: getaddrinfo ENOTFOUND api.anthropic.com"), FailureKind::Network);
        assert_eq!(kind_of(None, "something odd"), FailureKind::Unknown);
    }

    #[test]
    fn a_failure_with_nothing_remote_is_local() {
        let stderr = ["Error: Cannot find module 'x'".to_string()];
        let failure = diagnose(&facts(Engine::Codex, &[], &stderr));
        assert_eq!(failure.provider, Provider::Local);
        assert_eq!(failure.kind, FailureKind::Unknown);
        assert_eq!(failure.message, "Error: Cannot find module 'x'");
        assert_eq!(failure.stderr, stderr);
        let timed = diagnose(&Facts { timed_out: true, ..facts(Engine::Gemini, &[], &[]) });
        assert_eq!((timed.kind, timed.provider), (FailureKind::Timeout, Provider::Local));
    }

    #[test]
    fn each_cli_maps_to_its_provider() {
        let errors = ["unexpected status 401 Unauthorized".to_string()];
        assert_eq!(diagnose(&facts(Engine::Codex, &errors, &[])).provider, Provider::Openai);
        assert_eq!(diagnose(&facts(Engine::Gemini, &errors, &[])).provider, Provider::Google);
        assert_eq!(diagnose(&facts(Engine::Copilot, &errors, &[])).provider, Provider::Github);
    }

    /// Error text is whatever a service or a CLI printed, in any language:
    /// reading it never panics, whatever characters sit around the status.
    #[test]
    fn non_ascii_text_around_a_status_is_read_safely() {
        let spanish = format!("{} error 403: acceso denegado al modelo — reintentá", "é".repeat(20));
        assert_eq!(status_in(&spanish), Some(403));
        let after = format!("API error 429{}", "ñ".repeat(30));
        assert_eq!(status_in(&after), Some(429));
        // Lowercasing changes byte lengths before the key: still no panic.
        assert_eq!(request_id_in("İİİİ request_id: abcdef123456"), "abcdef123456");
        let failure = diagnose(&facts(Engine::Claude, &[spanish], &[]));
        assert_eq!(failure.status, Some(403));
    }

    #[test]
    fn request_ids_are_kept_for_support() {
        assert_eq!(request_id_in("API Error: 500 {\"request_id\":\"req_011CfkAA5YSPcwc5\"}"), "req_011CfkAA5YSPcwc5");
        assert_eq!(request_id_in("status 502, x-request-id: 9f8e7d6c-1234"), "9f8e7d6c-1234");
        assert_eq!(request_id_in("no id here"), "");
    }
}
