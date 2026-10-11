//! What a request costs at each provider's API prices (`prices.json`,
//! dollars per million tokens).
//!
//! An estimate, and shown as one: someone on a subscription pays no such
//! bill, and prices change. It's there to weigh one habit against another,
//! not to be reconciled with an invoice. A model that isn't in the table has
//! no cost at all rather than a guessed one.

use std::sync::OnceLock;

use serde::Deserialize;

use super::Request;

/// Where the table doesn't say: a cache read costs a tenth of the input
/// price, a write kept five minutes a quarter more than it, and a write kept
/// an hour double.
const CACHE_READ: f64 = 0.1;
const CACHE_WRITE_5M: f64 = 1.25;
const CACHE_WRITE_1H: f64 = 2.0;

/// An output price from which a model counts as a top-tier one: more than a
/// simple task needs.
const TOP_TIER_OUTPUT: f64 = 20.0;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Table {
    as_of: String,
    models: Vec<Price>,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    /// The model's id, or the start of it: ids often carry a date after.
    id: String,
    pub input: f64,
    pub output: f64,
    cache_read: Option<f64>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| serde_json::from_str(include_str!("prices.json")).expect("prices.json is valid"))
}

/// The day the prices were last checked.
pub fn as_of() -> &'static str {
    &table().as_of
}

/// The price of `model`: the entry with the longest id it starts with, so
/// `claude-opus-5-5` isn't priced as `claude-opus-5`.
pub fn price_of(model: &str) -> Option<&'static Price> {
    table()
        .models
        .iter()
        .filter(|price| model.starts_with(price.id.as_str()))
        .max_by_key(|price| price.id.len())
}

/// Whether `model` is one of the dear ones. None when its price isn't known.
pub fn is_top_tier(model: &str) -> Option<bool> {
    price_of(model).map(|price| price.output >= TOP_TIER_OUTPUT)
}

/// Dollars for one request.
pub fn cost(request: &Request) -> Option<f64> {
    let price = price_of(&request.model)?;
    let tokens = &request.tokens;
    let write_1h = request.cache_write_1h.min(tokens.cache_write);
    let write_5m = tokens.cache_write - write_1h;
    let dollars = tokens.input as f64 * price.input
        + tokens.output as f64 * price.output
        + tokens.cache_read as f64 * price.cache_read.unwrap_or(price.input * CACHE_READ)
        + write_5m as f64 * price.input * CACHE_WRITE_5M
        + write_1h as f64 * price.input * CACHE_WRITE_1H;
    Some(dollars / 1_000_000.0)
}

/// What reading `tokens` from the cache costs on `model`, against writing
/// them again: what a cache left to expire throws away.
pub fn rewrite_penalty(model: &str, tokens: u64) -> Option<f64> {
    let price = price_of(model)?;
    let read = price.cache_read.unwrap_or(price.input * CACHE_READ);
    Some(tokens as f64 * (price.input * CACHE_WRITE_5M - read) / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::companion::Tokens;

    #[test]
    fn a_dated_id_takes_its_models_price_and_not_a_shorter_namesakes() {
        assert_eq!(price_of("claude-haiku-4-5-20251001").unwrap().input, 1.0);
        assert_eq!(price_of("claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(price_of("claude-opus-5").unwrap().input, 5.0);
        assert!(price_of("gpt-5").is_none());
        assert_eq!(is_top_tier("claude-opus-5-5"), Some(true));
        assert_eq!(is_top_tier("claude-haiku-5-5"), Some(false));
        assert_eq!(is_top_tier("who-knows"), None);
    }

    /// The figures of a real session's own cost report: 54,238 input, 838,004
    /// output, 256,198,795 read from the cache and 7,369,985 written to it
    /// for an hour, which Claude Code priced at $223.02.
    #[test]
    fn it_agrees_with_what_claude_code_charged_a_real_session() {
        let request = Request {
            at: 0,
            model: "claude-opus-5".into(),
            tokens: Tokens { input: 54_238, output: 838_004, cache_read: 256_198_795, cache_write: 7_369_985 },
            cache_write_1h: 7_369_985,
        };
        let cost = cost(&request).unwrap();
        assert!((cost - 223.02).abs() < 0.01, "{cost}");
    }

    #[test]
    fn a_five_minute_write_costs_less_than_an_hours() {
        let tokens = Tokens { cache_write: 1_000_000, ..Default::default() };
        let short = cost(&Request { at: 0, model: "claude-opus-5".into(), tokens, cache_write_1h: 0 }).unwrap();
        let long = cost(&Request { at: 0, model: "claude-opus-5".into(), tokens, cache_write_1h: 1_000_000 }).unwrap();
        assert_eq!((short, long), (6.25, 10.0));
        // A stated cache-read price wins over the tenth.
        let read = Tokens { cache_read: 1_000_000, ..Default::default() };
        assert_eq!(cost(&Request { at: 0, model: "claude-opus-5-5".into(), tokens: read, cache_write_1h: 0 }), Some(0.2));
    }
}
