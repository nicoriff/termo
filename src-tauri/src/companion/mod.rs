//! The Companion: what a session with an agent cost, and which habits made
//! it cost more than it had to.
//!
//! Every agent CLI already keeps its sessions on disk (see `sources`). Those
//! are read into one shape — a `SessionUsage`, turn by turn — that `rules`
//! looks over for the habits worth pointing out, and `pricing` puts a figure
//! on. Nothing here asks a model anything, and nothing leaves the computer.
//!
//! Plain data and pure functions: the words are localized in the frontend.

pub mod advice;
pub mod pricing;
pub mod rules;
pub mod sources;
pub mod summary;

use serde::{Deserialize, Serialize};

/// What a request was billed for, by kind. The agents that don't tell cached
/// tokens apart report them all as `input`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    /// Sent and not served from the cache.
    pub input: u64,
    /// Sent and saved to the cache: dearer than `input`.
    pub cache_write: u64,
    /// Served from the cache: a fraction of `input`.
    pub cache_read: u64,
    pub output: u64,
}

impl Tokens {
    /// Everything the model was given to read: how full the context was.
    pub fn context(&self) -> u64 {
        self.input + self.cache_write + self.cache_read
    }

    pub fn total(&self) -> u64 {
        self.context() + self.output
    }

    pub fn add(&mut self, other: &Tokens) {
        self.input += other.input;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
        self.output += other.output;
    }
}

/// One call to the model. A turn is as many of these as the agent needed:
/// one per round of tools.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Request {
    /// Unix milliseconds.
    pub at: i64,
    pub model: String,
    pub tokens: Tokens,
    /// The part of `tokens.cache_write` saved for an hour rather than five
    /// minutes, which costs more.
    pub cache_write_1h: u64,
    /// What the agent itself says it cost, for the ones that do: taken over
    /// the price table's figure.
    pub reported_cost_usd: Option<f64>,
}

/// What the user asked, and everything the agent did to answer it.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    /// Unix milliseconds of the prompt.
    pub at: i64,
    /// The prompt, or the start of a very long one (see
    /// `sources::PROMPT_KEPT`).
    pub prompt: String,
    pub prompt_chars: u32,
    /// Not typed by the user: a background task reporting back, a resumed
    /// session's first work. Nothing about its prompt says anything of them.
    pub automatic: bool,
    /// The model that did most of the turn.
    pub model: Option<String>,
    pub effort: Option<String>,
    pub tokens: Tokens,
    pub request_count: u32,
    pub tool_calls: u32,
    /// The fullest the context got during the turn.
    pub peak_context: u64,
    /// Characters of the largest tool result the turn brought in.
    pub largest_result_chars: u64,
    /// At API prices; None when a model's price isn't known.
    pub cost_usd: Option<f64>,
    #[serde(skip)]
    pub requests: Vec<Request>,
    /// Files read in full during the turn, for spotting the ones read again
    /// and again.
    #[serde(skip)]
    pub files_read: Vec<String>,
}

/// The agent summarized the conversation to make room.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Compaction {
    pub at: i64,
    /// On its own, the context being full, rather than asked for.
    pub automatic: bool,
    pub tokens_before: u64,
    pub tokens_after: u64,
}

/// What an agent's session files say, since they don't all say the same:
/// a rule that needs something missing here stays quiet, and the panel says
/// what isn't known instead of showing a zero.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub tokens: bool,
    /// Cached tokens apart from the rest.
    pub cache: bool,
    pub prompts: bool,
    pub tools: bool,
    pub subagents: bool,
    /// Its tokens are billed at a price each, or would be without a
    /// subscription. Not Copilot CLI's, which counts requests instead.
    pub priced: bool,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionUsage {
    /// "claude", "codex", "gemini", "copilot" or "grok".
    pub tool: String,
    pub session_id: String,
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub turns: Vec<Turn>,
    pub tokens: Tokens,
    /// What its subagents used, counted in `tokens` and `cost_usd` too.
    pub subagent_tokens: Tokens,
    pub subagent_cost_usd: Option<f64>,
    pub compactions: Vec<Compaction>,
    /// At API prices; None when a model's price isn't known.
    pub cost_usd: Option<f64>,
    /// The agent's own figure, when it wrote one down.
    pub reported_cost_usd: Option<f64>,
    pub coverage: Coverage,
}

impl SessionUsage {
    /// Sums the turns into the session's totals and prices them. Called once
    /// a source has filled in the turns.
    pub fn finish(&mut self) {
        let mut tokens = self.subagent_tokens;
        let mut cost = if self.subagent_tokens.total() == 0 { Some(0.0) } else { self.subagent_cost_usd };
        for turn in &mut self.turns {
            turn.tokens = Tokens::default();
            turn.peak_context = 0;
            let mut turn_cost = Some(0.0);
            for request in &turn.requests {
                turn.tokens.add(&request.tokens);
                turn.peak_context = turn.peak_context.max(request.tokens.context());
                let priced = if self.coverage.priced { pricing::cost(request) } else { None };
                turn_cost = match (turn_cost, request.reported_cost_usd.or(priced)) {
                    (Some(sum), Some(cost)) => Some(sum + cost),
                    _ => None,
                };
            }
            turn.request_count = turn.requests.len() as u32;
            turn.model = main_model(&turn.requests);
            turn.cost_usd = if turn.requests.is_empty() { None } else { turn_cost };
            tokens.add(&turn.tokens);
            cost = match (cost, turn.cost_usd) {
                (Some(sum), Some(of_turn)) => Some(sum + of_turn),
                // A turn the agent never answered cost nothing.
                (sum, None) if turn.requests.is_empty() => sum,
                _ => None,
            };
        }
        self.tokens = tokens;
        // A session nothing was asked of has no cost to speak of, not a zero.
        self.cost_usd = if self.coverage.tokens && tokens.total() > 0 { cost } else { None };
    }
}

/// The model that produced the most output among `requests`.
fn main_model(requests: &[Request]) -> Option<String> {
    let mut by_model: Vec<(&str, u64)> = Vec::new();
    for request in requests {
        match by_model.iter_mut().find(|(model, _)| *model == request.model) {
            Some(entry) => entry.1 += request.tokens.output,
            None => by_model.push((&request.model, request.tokens.output)),
        }
    }
    by_model.into_iter().max_by_key(|(_, output)| *output).map(|(model, _)| model.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(model: &str, input: u64, output: u64) -> Request {
        Request { model: model.into(), tokens: Tokens { input, output, ..Default::default() }, ..Default::default() }
    }

    #[test]
    fn a_session_adds_up_its_turns_and_its_subagents() {
        let mut session = SessionUsage {
            coverage: Coverage { tokens: true, priced: true, ..Default::default() },
            subagent_tokens: Tokens { input: 1_000_000, ..Default::default() },
            subagent_cost_usd: Some(1.0),
            turns: vec![Turn {
                requests: vec![request("claude-opus-5-5", 1_000_000, 0), request("claude-haiku-5-5", 0, 1_000_000)],
                ..Default::default()
            }],
            ..Default::default()
        };
        session.finish();
        let turn = &session.turns[0];
        assert_eq!(turn.request_count, 2);
        assert_eq!(turn.model.as_deref(), Some("claude-haiku-5-5"));
        assert_eq!(turn.peak_context, 1_000_000);
        // $4 of Opus input and $0.50 of Haiku output.
        assert_eq!(turn.cost_usd, Some(4.5));
        assert_eq!(session.tokens.input, 2_000_000);
        assert_eq!(session.cost_usd, Some(5.5));
    }

    #[test]
    fn a_model_without_a_price_leaves_the_cost_unknown() {
        let mut session = SessionUsage {
            coverage: Coverage { tokens: true, priced: true, ..Default::default() },
            subagent_cost_usd: Some(0.0),
            turns: vec![Turn { requests: vec![request("some-new-model", 10, 10)], ..Default::default() }],
            ..Default::default()
        };
        session.finish();
        assert_eq!(session.turns[0].cost_usd, None);
        assert_eq!(session.cost_usd, None);
        assert_eq!(session.tokens.total(), 20);
    }
}
