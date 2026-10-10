//! What a model costs, for the estimate a plan shows before anything runs:
//! each provider's public API price per million tokens. An estimate only —
//! a subscription (Claude Max, ChatGPT, Copilot) is billed differently, and
//! the token counts it multiplies are the planner's guess. A model that isn't
//! here has no price, and its tasks show tokens alone: nothing is made up.
//!
//! Prices as published on 2026-10-04; reviewed with each release.

use serde::Serialize;

use super::super::spec::Engine;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    /// USD per million input tokens.
    pub input: f64,
    /// USD per million output tokens.
    pub output: f64,
}

const fn price(input: f64, output: f64) -> Price {
    Price { input, output }
}

/// By id prefix, so a dated or suffixed id (`claude-opus-5-5[1m]`) finds its
/// model; the longest prefix wins (`gpt-5.4-mini` before `gpt-5.4`).
const CLAUDE: &[(&str, Price)] = &[
    ("claude-fable-5", price(10.0, 50.0)),
    ("claude-opus-5-5", price(4.0, 20.0)),
    ("claude-opus-5", price(5.0, 25.0)),
    ("claude-opus-4", price(5.0, 25.0)),
    ("claude-sonnet-5", price(2.0, 10.0)),
    ("claude-sonnet-4", price(3.0, 15.0)),
    ("claude-haiku-4-5", price(1.0, 5.0)),
];

/// Claude Code's own short names, which follow its current models.
const CLAUDE_ALIASES: &[(&str, &str)] = &[
    ("fable", "claude-fable-5-1"),
    ("opus", "claude-opus-5-5"),
    ("sonnet", "claude-sonnet-5-5"),
    ("haiku", "claude-haiku-4-5"),
];

const CODEX: &[(&str, Price)] = &[
    ("gpt-6-astra", price(10.0, 50.0)),
    ("gpt-6.1-sol", price(2.0, 10.0)),
    ("gpt-6-sol", price(2.0, 10.0)),
    ("gpt-6-luna", price(0.10, 0.50)),
    ("gpt-5.6-sol", price(4.0, 20.0)),
    ("gpt-5.6-terra", price(2.0, 12.0)),
    ("gpt-5.6-luna", price(0.20, 1.20)),
    ("gpt-5.5-pro", price(30.0, 180.0)),
    ("gpt-5.5", price(5.0, 30.0)),
    ("gpt-5.4-pro", price(30.0, 180.0)),
    ("gpt-5.4-mini", price(0.75, 4.50)),
    ("gpt-5.4-nano", price(0.20, 1.25)),
    ("gpt-5.4", price(2.50, 15.0)),
    ("gpt-5.3-codex", price(1.75, 14.0)),
    ("gpt-5.2-pro", price(21.0, 168.0)),
    ("gpt-5.2", price(1.75, 14.0)),
    ("gpt-5.1", price(1.25, 10.0)),
    ("gpt-5-pro", price(15.0, 120.0)),
    ("gpt-5-mini", price(0.25, 2.0)),
    ("gpt-5-nano", price(0.05, 0.40)),
    ("gpt-5", price(1.25, 10.0)),
];

const GEMINI: &[(&str, Price)] = &[
    ("gemini-3.8-flash", price(0.75, 3.75)),
    ("gemini-3.7-flash", price(0.75, 3.75)),
    ("gemini-3.6-flash", price(0.75, 3.75)),
    ("gemini-3.5-flash-lite", price(0.30, 2.50)),
    ("gemini-3.5-flash", price(1.50, 9.0)),
    ("gemini-3.1-flash-lite", price(0.25, 1.50)),
    ("gemini-3.1-pro", price(2.0, 12.0)),
    ("gemini-2.5-pro", price(1.25, 10.0)),
    ("gemini-2.5-flash-lite", price(0.10, 0.40)),
    ("gemini-2.5-flash", price(0.30, 2.50)),
];

/// Gemini CLI's aliases (see models.rs), priced as the newest model of each
/// family: which one the CLI picks changes from one version to the next.
const GEMINI_ALIASES: &[(&str, &str)] = &[
    ("pro", "gemini-3.1-pro"),
    ("flash", "gemini-3.8-flash"),
    ("flash-lite", "gemini-3.5-flash-lite"),
];

fn lookup(table: &[(&str, Price)], id: &str) -> Option<Price> {
    table
        .iter()
        .filter(|(prefix, _)| id.starts_with(prefix))
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, price)| *price)
}

/// The price of `model` on `engine`, when it's known. Copilot bills by
/// premium requests rather than tokens, and "auto" names no model at all:
/// neither has one.
pub fn price_of(engine: Engine, model: &str) -> Option<Price> {
    let id = model.trim().to_lowercase();
    // `claude-opus-5-5[1m]`: the same model with a longer context.
    let id = id.split('[').next().unwrap_or_default().trim();
    if id.is_empty() {
        return None;
    }
    let (table, aliases): (&[(&str, Price)], &[(&str, &str)]) = match engine {
        Engine::Claude => (CLAUDE, CLAUDE_ALIASES),
        Engine::Codex => (CODEX, &[]),
        Engine::Gemini => (GEMINI, GEMINI_ALIASES),
        // Grok's token prices aren't in here: its runs report what they
        // cost, and a plan with it shows no estimate.
        Engine::Copilot | Engine::Grok => return None,
    };
    let id = aliases.iter().find(|(alias, _)| *alias == id).map_or(id, |(_, full)| *full);
    lookup(table, id)
}

/// What `tokens_in` + `tokens_out` cost at `price`, in USD.
pub fn cost(price: Price, tokens_in: u64, tokens_out: u64) -> f64 {
    (tokens_in as f64 * price.input + tokens_out as f64 * price.output) / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_prefix_names_the_model() {
        assert_eq!(price_of(Engine::Codex, "gpt-5.4-mini"), Some(price(0.75, 4.50)));
        assert_eq!(price_of(Engine::Codex, "gpt-5.4"), Some(price(2.50, 15.0)));
        assert_eq!(price_of(Engine::Claude, "claude-opus-5-5"), Some(price(4.0, 20.0)));
        assert_eq!(price_of(Engine::Claude, "claude-opus-5"), Some(price(5.0, 25.0)));
    }

    #[test]
    fn aliases_and_suffixes_find_their_model() {
        assert_eq!(price_of(Engine::Claude, "sonnet"), Some(price(2.0, 10.0)));
        assert_eq!(price_of(Engine::Claude, " Claude-Opus-5-5[1m] "), Some(price(4.0, 20.0)));
        assert_eq!(price_of(Engine::Gemini, "flash-lite"), Some(price(0.30, 2.50)));
    }

    #[test]
    fn what_is_not_listed_has_no_price() {
        assert_eq!(price_of(Engine::Claude, ""), None);
        assert_eq!(price_of(Engine::Gemini, "auto"), None);
        assert_eq!(price_of(Engine::Copilot, "gpt-5.5"), None);
        assert_eq!(price_of(Engine::Codex, "some-new-model"), None);
    }

    #[test]
    fn cost_is_per_million_tokens() {
        let total = cost(price(2.0, 10.0), 500_000, 100_000);
        assert!((total - 2.0).abs() < 1e-9);
    }
}
