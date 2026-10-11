//! The habits that made a session cost more than it had to, read off its
//! turns. Each rule is a pure function over a `SessionUsage` and says what it
//! saw in numbers: the words, and the advice, are the frontend's.
//!
//! They're meant to be right when they speak rather than to catch
//! everything: the thresholds are set high, each finding carries the figures
//! it rests on, and a rule that needs something the agent's files don't say
//! (see `Coverage`) stays quiet.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::{pricing, SessionUsage, Turn};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum FindingKind {
    /// Turn after turn with a very full context, each one reading it all
    /// again.
    ContextBloat,
    /// A pause long enough for the cache to expire: the next turn paid to
    /// write the conversation to it again.
    CacheExpired,
    /// A top-tier model, or a high effort, on turns that asked for little.
    OversizedModel,
    /// Short prompts sent one at a time into a large context.
    SmallTurnsLargeContext,
    /// Tool results large enough to fill the context on their own.
    LargeResults,
    /// The same file read in full several times.
    RepeatedReads,
    /// Most of the session's tokens went to subagents.
    SubagentHeavy,
    /// The very first request was already large: what's loaded before the
    /// user says anything.
    HeavyBaseline,
    /// One session carried on for a great many turns.
    LongSession,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    High,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub kind: FindingKind,
    pub severity: Severity,
    /// The turns it's about, by position in the session.
    pub turns: Vec<usize>,
    /// The figures it rests on, by name: what the panel fills its sentence
    /// with.
    pub numbers: BTreeMap<&'static str, f64>,
    /// Dollars that needn't have been spent, at API prices, where that can
    /// be put a figure on.
    pub wasted_usd: Option<f64>,
}

impl Finding {
    fn new(kind: FindingKind, severity: Severity, turns: Vec<usize>, numbers: &[(&'static str, f64)]) -> Self {
        Self { kind, severity, turns, numbers: numbers.iter().copied().collect(), wasted_usd: None }
    }

    fn wasting(mut self, dollars: Option<f64>) -> Self {
        self.wasted_usd = dollars.filter(|dollars| *dollars > 0.0);
        self
    }
}

/// A context this full is worth compacting or starting over from.
const FULL_CONTEXT: u64 = 150_000;
/// How many turns at that size before it's a habit and not a moment.
const FULL_CONTEXT_TURNS: usize = 5;

/// Caches last five minutes, or an hour when written for one.
const CACHE_TTL_MS: i64 = 5 * 60 * 1000;
const CACHE_TTL_1H_MS: i64 = 60 * 60 * 1000;
/// A rewrite smaller than this isn't worth a word.
const CACHE_REWRITE_MIN: u64 = 20_000;

/// A turn that asked for little: a short prompt, a short answer, hardly any
/// tools.
const TRIVIAL_PROMPT_CHARS: u32 = 200;
const TRIVIAL_OUTPUT: u64 = 500;
const TRIVIAL_REQUESTS: u32 = 2;
const TRIVIAL_TURNS: usize = 5;
const HIGH_EFFORTS: [&str; 2] = ["xhigh", "max"];

const SHORT_PROMPT_CHARS: u32 = 80;
const LARGE_CONTEXT: u64 = 100_000;
const SHORT_TURNS: usize = 6;

/// About 12,000 tokens of one tool's output.
const LARGE_RESULT_CHARS: u64 = 50_000;
const LARGE_RESULTS: usize = 3;

const REPEATED_READS: usize = 3;

const SUBAGENT_SHARE: f64 = 0.5;
const SUBAGENT_MIN: u64 = 500_000;

const HEAVY_BASELINE: u64 = 40_000;

const LONG_SESSION_TURNS: usize = 80;

/// Everything worth pointing out about `session`, the costliest first.
pub fn findings(session: &SessionUsage) -> Vec<Finding> {
    let mut found: Vec<Finding> = [
        context_bloat(session),
        cache_expired(session),
        oversized_model(session),
        small_turns_large_context(session),
        large_results(session),
        repeated_reads(session),
        subagent_heavy(session),
        heavy_baseline(session),
        long_session(session),
    ]
    .into_iter()
    .flatten()
    .collect();
    found.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(b.wasted_usd.unwrap_or(0.0).total_cmp(&a.wasted_usd.unwrap_or(0.0)))
            .then(a.kind.cmp(&b.kind))
    });
    found
}

fn context_bloat(session: &SessionUsage) -> Option<Finding> {
    if !session.coverage.tokens {
        return None;
    }
    let turns: Vec<usize> = indexes(session, |turn| turn.peak_context >= FULL_CONTEXT);
    if turns.len() < FULL_CONTEXT_TURNS {
        return None;
    }
    let peak = session.turns.iter().map(|turn| turn.peak_context).max().unwrap_or(0);
    // What was read above the mark, request after request: what a compacted
    // conversation wouldn't have sent.
    let mut excess = 0u64;
    let mut wasted = Some(0.0);
    for request in turns.iter().flat_map(|index| &session.turns[*index].requests) {
        let over = request.tokens.context().saturating_sub(FULL_CONTEXT);
        excess += over;
        let read = super::Request { tokens: super::Tokens { cache_read: over, ..Default::default() }, ..request.clone() };
        wasted = wasted.zip(pricing::cost(&read)).map(|(sum, cost)| sum + cost);
    }
    let automatic = session.compactions.iter().filter(|compaction| compaction.automatic).count();
    let severity = if peak >= FULL_CONTEXT * 3 { Severity::High } else { Severity::Warning };
    Some(
        Finding::new(
            FindingKind::ContextBloat,
            severity,
            turns.clone(),
            &[("turns", turns.len() as f64), ("peakTokens", peak as f64), ("excessTokens", excess as f64), ("autoCompactions", automatic as f64)],
        )
        .wasting(wasted),
    )
}

fn cache_expired(session: &SessionUsage) -> Option<Finding> {
    if !session.coverage.cache {
        return None;
    }
    let mut turns = Vec::new();
    let mut rewritten = 0u64;
    let mut wasted = Some(0.0);
    let mut longest_gap = 0i64;
    let mut last: Option<&super::Request> = None;
    for (index, turn) in session.turns.iter().enumerate() {
        if let (Some(first), Some(before)) = (turn.requests.first(), last) {
            let ttl = if before.cache_write_1h > 0 || first.cache_write_1h > 0 { CACHE_TTL_1H_MS } else { CACHE_TTL_MS };
            let gap = first.at - before.at;
            let written = first.tokens.cache_write;
            // Written again rather than read: more went to the cache than
            // came from it.
            if gap > ttl && written >= CACHE_REWRITE_MIN && written > first.tokens.cache_read {
                turns.push(index);
                rewritten += written;
                longest_gap = longest_gap.max(gap);
                wasted = wasted.zip(pricing::rewrite_penalty(&first.model, written)).map(|(sum, cost)| sum + cost);
            }
        }
        last = turn.requests.last().or(last);
    }
    if turns.is_empty() {
        return None;
    }
    let severity = if turns.len() >= 3 { Severity::Warning } else { Severity::Info };
    Some(
        Finding::new(
            FindingKind::CacheExpired,
            severity,
            turns.clone(),
            &[("times", turns.len() as f64), ("rewrittenTokens", rewritten as f64), ("longestGapMinutes", (longest_gap / 60_000) as f64)],
        )
        .wasting(wasted),
    )
}

fn is_trivial(turn: &Turn) -> bool {
    !turn.automatic
        && turn.request_count >= 1
        && turn.request_count <= TRIVIAL_REQUESTS
        && turn.tool_calls <= 1
        && turn.prompt_chars <= TRIVIAL_PROMPT_CHARS
        && turn.tokens.output <= TRIVIAL_OUTPUT
}

fn oversized_model(session: &SessionUsage) -> Option<Finding> {
    if !(session.coverage.tokens && session.coverage.prompts) {
        return None;
    }
    let dear = |turn: &Turn| {
        turn.model.as_deref().and_then(pricing::is_top_tier).unwrap_or(false)
            || turn.effort.as_deref().is_some_and(|effort| HIGH_EFFORTS.contains(&effort))
    };
    let turns = indexes(session, |turn| is_trivial(turn) && dear(turn));
    if turns.len() < TRIVIAL_TURNS {
        return None;
    }
    let spent: Option<f64> = turns.iter().map(|index| session.turns[*index].cost_usd).sum();
    let high_effort = turns.iter().filter(|index| session.turns[**index].effort.as_deref().is_some_and(|effort| HIGH_EFFORTS.contains(&effort))).count();
    let mut finding = Finding::new(
        FindingKind::OversizedModel,
        Severity::Info,
        turns.clone(),
        &[("turns", turns.len() as f64), ("highEffortTurns", high_effort as f64)],
    );
    // What those turns cost, not what a smaller model would have saved: that
    // depends on which one, so it isn't called wasted.
    if let Some(spent) = spent {
        finding.numbers.insert("spentUsd", spent);
    }
    Some(finding)
}

fn small_turns_large_context(session: &SessionUsage) -> Option<Finding> {
    if !(session.coverage.tokens && session.coverage.prompts) {
        return None;
    }
    let turns = indexes(session, |turn| !turn.automatic && turn.prompt_chars <= SHORT_PROMPT_CHARS && turn.peak_context >= LARGE_CONTEXT);
    if turns.len() < SHORT_TURNS {
        return None;
    }
    let read: u64 = turns.iter().map(|index| session.turns[*index].tokens.context()).sum();
    let spent: Option<f64> = turns.iter().map(|index| session.turns[*index].cost_usd).sum();
    let mut finding = Finding::new(
        FindingKind::SmallTurnsLargeContext,
        Severity::Info,
        turns.clone(),
        &[("turns", turns.len() as f64), ("contextTokens", read as f64)],
    );
    if let Some(spent) = spent {
        finding.numbers.insert("spentUsd", spent);
    }
    Some(finding)
}

fn large_results(session: &SessionUsage) -> Option<Finding> {
    if !session.coverage.tools {
        return None;
    }
    let turns = indexes(session, |turn| turn.largest_result_chars >= LARGE_RESULT_CHARS);
    if turns.len() < LARGE_RESULTS {
        return None;
    }
    let largest = session.turns.iter().map(|turn| turn.largest_result_chars).max().unwrap_or(0);
    Some(Finding::new(
        FindingKind::LargeResults,
        Severity::Warning,
        turns.clone(),
        &[("turns", turns.len() as f64), ("largestChars", largest as f64)],
    ))
}

fn repeated_reads(session: &SessionUsage) -> Option<Finding> {
    if !session.coverage.tools {
        return None;
    }
    let mut reads: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, turn) in session.turns.iter().enumerate() {
        for path in &turn.files_read {
            reads.entry(path).or_default().push(index);
        }
    }
    let again: Vec<&Vec<usize>> = reads.values().filter(|turns| turns.len() >= REPEATED_READS).collect();
    if again.is_empty() {
        return None;
    }
    let most = again.iter().map(|turns| turns.len()).max().unwrap_or(0);
    let mut turns: Vec<usize> = again.iter().flat_map(|turns| turns.iter().copied()).collect();
    turns.sort_unstable();
    turns.dedup();
    Some(Finding::new(
        FindingKind::RepeatedReads,
        Severity::Info,
        turns,
        &[("files", again.len() as f64), ("mostReads", most as f64)],
    ))
}

fn subagent_heavy(session: &SessionUsage) -> Option<Finding> {
    if !session.coverage.subagents {
        return None;
    }
    let used = session.subagent_tokens.total();
    let all = session.tokens.total();
    if used < SUBAGENT_MIN || all == 0 || (used as f64 / all as f64) < SUBAGENT_SHARE {
        return None;
    }
    let mut finding = Finding::new(
        FindingKind::SubagentHeavy,
        Severity::Info,
        Vec::new(),
        &[("tokens", used as f64), ("share", used as f64 / all as f64)],
    );
    if let Some(spent) = session.subagent_cost_usd {
        finding.numbers.insert("spentUsd", spent);
    }
    Some(finding)
}

fn heavy_baseline(session: &SessionUsage) -> Option<Finding> {
    if !session.coverage.tokens {
        return None;
    }
    let (index, first) = session.turns.iter().enumerate().find_map(|(index, turn)| Some((index, turn.requests.first()?)))?;
    let baseline = first.tokens.context();
    // A resumed session starts with its whole past: that's not a baseline.
    if baseline < HEAVY_BASELINE || session.turns[index].automatic {
        return None;
    }
    Some(Finding::new(FindingKind::HeavyBaseline, Severity::Info, vec![index], &[("tokens", baseline as f64)]))
}

fn long_session(session: &SessionUsage) -> Option<Finding> {
    let typed = session.turns.iter().filter(|turn| !turn.automatic).count();
    if typed < LONG_SESSION_TURNS {
        return None;
    }
    let days = (session.ended_at - session.started_at) as f64 / 86_400_000.0;
    Some(Finding::new(FindingKind::LongSession, Severity::Info, Vec::new(), &[("turns", typed as f64), ("days", days)]))
}

fn indexes(session: &SessionUsage, matches: impl Fn(&Turn) -> bool) -> Vec<usize> {
    session.turns.iter().enumerate().filter(|(_, turn)| matches(turn)).map(|(index, _)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::companion::{Compaction, Coverage, Request, Tokens};

    const MINUTE: i64 = 60_000;
    const FULL: Coverage = Coverage { tokens: true, cache: true, prompts: true, tools: true, subagents: true };

    fn request(at: i64, tokens: Tokens) -> Request {
        Request { at, model: "claude-opus-5-5".into(), tokens, cache_write_1h: 0 }
    }

    fn turn(prompt_chars: u32, requests: Vec<Request>) -> Turn {
        Turn { at: requests.first().map(|request| request.at).unwrap_or(0), prompt_chars, requests, ..Default::default() }
    }

    fn session(turns: Vec<Turn>) -> SessionUsage {
        let mut session = SessionUsage { coverage: FULL, turns, ..Default::default() };
        session.finish();
        session
    }

    fn kinds(session: &SessionUsage) -> Vec<FindingKind> {
        let mut kinds: Vec<FindingKind> = findings(session).into_iter().map(|finding| finding.kind).collect();
        kinds.sort();
        kinds
    }

    fn only(session: &SessionUsage, kind: FindingKind) -> Finding {
        findings(session).into_iter().find(|finding| finding.kind == kind).unwrap_or_else(|| panic!("no {kind:?}"))
    }

    /// A working session of ordinary size: a long prompt, some tools, a
    /// modest context.
    fn ordinary_turn(at: i64) -> Turn {
        let mut turn = turn(600, vec![request(at, Tokens { input: 50, cache_read: 30_000, cache_write: 500, output: 1_500 })]);
        turn.tool_calls = 4;
        turn
    }

    #[test]
    fn an_ordinary_session_has_nothing_to_point_out() {
        let session = session((0..20).map(|n| ordinary_turn(n * MINUTE)).collect());
        assert!(findings(&session).is_empty(), "{:?}", findings(&session));
    }

    #[test]
    fn many_turns_on_a_full_context_are_bloat() {
        let full = |at| {
            let mut turn = turn(600, vec![request(at, Tokens { cache_read: 250_000, output: 2_000, ..Default::default() })]);
            turn.tool_calls = 4;
            turn
        };
        let mut session = session((0..5).map(|n| full(n * MINUTE)).collect());
        let finding = only(&session, FindingKind::ContextBloat);
        assert_eq!(finding.turns, [0, 1, 2, 3, 4]);
        assert_eq!(finding.numbers["peakTokens"], 250_000.0);
        // 100,000 tokens over the mark, five times, read from the cache.
        assert_eq!(finding.numbers["excessTokens"], 500_000.0);
        assert!((finding.wasted_usd.unwrap() - 0.1).abs() < 1e-9);
        assert_eq!(finding.severity, Severity::Warning);

        // Four such turns aren't a habit yet.
        session.turns.truncate(4);
        assert!(!kinds(&session).contains(&FindingKind::ContextBloat));

        session.turns[0].requests[0].tokens.cache_read = 600_000;
        session.turns.push(full(9 * MINUTE));
        session.compactions.push(Compaction { at: 0, automatic: true, tokens_before: 600_000, tokens_after: 20_000 });
        session.finish();
        let finding = only(&session, FindingKind::ContextBloat);
        assert_eq!((finding.severity, finding.numbers["autoCompactions"]), (Severity::High, 1.0));
    }

    #[test]
    fn a_pause_past_the_caches_life_pays_to_write_it_again() {
        let warm = Tokens { cache_read: 80_000, cache_write: 1_000, output: 500, ..Default::default() };
        let cold = Tokens { cache_read: 0, cache_write: 81_000, output: 500, ..Default::default() };
        let mut turns = vec![ordinary_turn(0), turn(600, vec![request(MINUTE, warm)])];
        // Twenty minutes later the cache is gone and gets written again.
        turns.push(turn(600, vec![request(21 * MINUTE, cold)]));
        // Back within its life: read, not written.
        turns.push(turn(600, vec![request(23 * MINUTE, warm)]));
        let session = session(turns);
        let finding = only(&session, FindingKind::CacheExpired);
        assert_eq!(finding.turns, [2]);
        assert_eq!(finding.numbers["rewrittenTokens"], 81_000.0);
        assert_eq!(finding.numbers["longestGapMinutes"], 20.0);
        // Written at $5 a million instead of read at $0.20.
        assert!((finding.wasted_usd.unwrap() - 81_000.0 * 4.8 / 1e6).abs() < 1e-9);
    }

    #[test]
    fn an_hours_cache_isnt_expired_by_twenty_minutes() {
        let mut first = request(0, Tokens { cache_write: 80_000, ..Default::default() });
        first.cache_write_1h = 80_000;
        let later = request(20 * MINUTE, Tokens { cache_write: 81_000, ..Default::default() });
        let session = session(vec![turn(600, vec![first]), turn(600, vec![later])]);
        assert!(!kinds(&session).contains(&FindingKind::CacheExpired));
    }

    #[test]
    fn little_asked_of_a_top_model_is_pointed_out_and_of_a_small_one_isnt() {
        let small = |at| turn(40, vec![request(at, Tokens { cache_read: 20_000, output: 120, ..Default::default() })]);
        let mut session = session((0..5).map(|n| small(n * MINUTE)).collect());
        let finding = only(&session, FindingKind::OversizedModel);
        assert_eq!((finding.numbers["turns"], finding.numbers["highEffortTurns"]), (5.0, 0.0));
        assert!(finding.numbers["spentUsd"] > 0.0 && finding.wasted_usd.is_none());

        for turn in &mut session.turns {
            turn.requests[0].model = "claude-haiku-5-5".into();
        }
        session.finish();
        assert!(!kinds(&session).contains(&FindingKind::OversizedModel));

        // A small model at a very high effort is the same habit.
        for turn in &mut session.turns {
            turn.effort = Some("max".into());
        }
        assert_eq!(only(&session, FindingKind::OversizedModel).numbers["highEffortTurns"], 5.0);

        // And turns that worked for their answer aren't trivial.
        for turn in &mut session.turns {
            turn.tool_calls = 6;
        }
        assert!(!kinds(&session).contains(&FindingKind::OversizedModel));
    }

    #[test]
    fn short_prompts_into_a_large_context_add_up() {
        let short = |at| {
            let mut turn = turn(30, vec![request(at, Tokens { cache_read: 120_000, output: 3_000, ..Default::default() })]);
            turn.tool_calls = 5;
            turn
        };
        let session = session((0..6).map(|n| short(n * MINUTE)).collect());
        let finding = only(&session, FindingKind::SmallTurnsLargeContext);
        assert_eq!((finding.numbers["turns"], finding.numbers["contextTokens"]), (6.0, 720_000.0));

        // A background task reporting back isn't a prompt of the user's.
        let mut automatic = session.clone();
        for turn in &mut automatic.turns {
            turn.automatic = true;
        }
        assert!(!kinds(&automatic).contains(&FindingKind::SmallTurnsLargeContext));
    }

    #[test]
    fn large_tool_results_and_files_read_again_and_again() {
        let mut turns: Vec<Turn> = (0..4).map(|n| ordinary_turn(n * MINUTE)).collect();
        for turn in turns.iter_mut().take(3) {
            turn.largest_result_chars = 200_000;
            turn.files_read = vec!["/work/schema.sql".into(), "/work/other.rs".into()];
        }
        turns[3].files_read = vec!["/work/other.rs".into()];
        let session = session(turns);
        let large = only(&session, FindingKind::LargeResults);
        assert_eq!((large.turns.as_slice(), large.numbers["largestChars"]), ([0, 1, 2].as_slice(), 200_000.0));
        let reads = only(&session, FindingKind::RepeatedReads);
        assert_eq!((reads.numbers["files"], reads.numbers["mostReads"]), (2.0, 4.0));
        assert_eq!(reads.turns, [0, 1, 2, 3]);
    }

    #[test]
    fn subagents_taking_most_of_a_session() {
        let mut session = session(vec![ordinary_turn(0)]);
        session.subagent_tokens = Tokens { input: 900_000, ..Default::default() };
        session.subagent_cost_usd = Some(3.6);
        session.finish();
        let finding = only(&session, FindingKind::SubagentHeavy);
        assert!(finding.numbers["share"] > 0.9);
        assert_eq!(finding.numbers["spentUsd"], 3.6);
    }

    #[test]
    fn a_first_request_thats_already_large() {
        let heavy = turn(600, vec![request(0, Tokens { cache_write: 65_000, output: 900, ..Default::default() })]);
        let session_heavy = session(vec![heavy.clone()]);
        assert_eq!(only(&session_heavy, FindingKind::HeavyBaseline).numbers["tokens"], 65_000.0);

        // A resumed session begins with everything said before.
        let resumed = session(vec![Turn { automatic: true, ..heavy }]);
        assert!(!kinds(&resumed).contains(&FindingKind::HeavyBaseline));
    }

    #[test]
    fn a_session_of_a_great_many_turns() {
        let mut long = session((0..80).map(|n| ordinary_turn(n * MINUTE)).collect());
        long.started_at = 0;
        long.ended_at = 2 * 86_400_000;
        let finding = only(&long, FindingKind::LongSession);
        assert_eq!((finding.numbers["turns"], finding.numbers["days"]), (80.0, 2.0));
    }

    #[test]
    fn a_rule_stays_quiet_about_what_the_agent_doesnt_report() {
        let full = |at| turn(30, vec![request(at, Tokens { input: 250_000, output: 100, ..Default::default() })]);
        let mut session = session((0..8).map(|n| full(n * MINUTE)).collect());
        assert!(kinds(&session).contains(&FindingKind::ContextBloat));
        session.coverage = Coverage::default();
        assert!(findings(&session).is_empty(), "{:?}", findings(&session));
    }

    #[test]
    fn the_costliest_findings_come_first() {
        let full = |at| turn(30, vec![request(at, Tokens { cache_read: 600_000, output: 100, ..Default::default() })]);
        let session = session((0..8).map(|n| full(n * MINUTE)).collect());
        let found = findings(&session);
        assert_eq!(found[0].kind, FindingKind::ContextBloat);
        assert!(found.windows(2).all(|pair| pair[0].severity >= pair[1].severity));
    }
}
