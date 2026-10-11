//! Where each agent CLI keeps its sessions, and how to read one into a
//! `SessionUsage`. These are other programs' own files, in formats nobody
//! promised to keep: a line that doesn't read is skipped and a session that
//! doesn't read at all is simply not there, never an error in the panel.

pub mod claude;
pub mod codex;
pub mod copilot;
pub mod gemini;
pub mod grok;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::de::{self, DeserializeOwned, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

use super::SessionUsage;

/// How much of each prompt is kept: all of an ordinary one, so it can be
/// shown and looked over (see companion/advice.rs), and the start of one
/// that runs to pages.
pub(crate) const PROMPT_KEPT: usize = 6000;

/// A session found on disk, before it's read.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionFile {
    pub tool: String,
    pub session_id: String,
    #[serde(skip)]
    pub path: PathBuf,
    /// Unix milliseconds of its last change.
    pub modified_at: i64,
    pub bytes: u64,
}

/// Every agent's sessions changed since `since` (Unix milliseconds), newest
/// first.
pub fn list(since: i64) -> Vec<SessionFile> {
    let mut found = claude::list(since);
    found.extend(codex::list(since));
    found.extend(gemini::list(since));
    found.extend(copilot::list(since));
    found.extend(grok::list(since));
    found.sort_by_key(|file| std::cmp::Reverse(file.modified_at));
    found
}

/// One session of `tool`'s, by the agent's own id for it.
pub fn read(tool: &str, session_id: &str) -> Option<SessionUsage> {
    match tool {
        "claude" => claude::read(&claude::find(session_id)?),
        "codex" => codex::read(&codex::find(session_id)?),
        "gemini" => gemini::read(&gemini::find(session_id)?),
        "copilot" => copilot::read(&copilot::find(session_id)?),
        "grok" => grok::read(&grok::find(session_id)?),
        _ => None,
    }
}

/// Unix milliseconds of an RFC 3339 timestamp; 0 when it isn't one.
pub(crate) fn millis(timestamp: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(timestamp).map(|at| at.timestamp_millis()).unwrap_or(0)
}

pub(crate) fn modified_millis(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|since| since.as_millis() as i64)
        .unwrap_or(0)
}

/// The start of `text`, cut at a character boundary.
pub(crate) fn excerpt(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    match trimmed.char_indices().nth(max_chars) {
        Some((end, _)) => format!("{}…", &trimmed[..end]),
        None => trimmed.to_string(),
    }
}

/// The lines of a JSONL file that read as a `T`; the rest are skipped. None
/// when the file can't be opened.
pub(crate) fn jsonl<T: DeserializeOwned>(path: &Path) -> Option<impl Iterator<Item = T>> {
    let reader = BufReader::with_capacity(1 << 20, File::open(path).ok()?);
    Some(reader.split(b'\n').map_while(Result::ok).filter_map(|line| serde_json::from_slice(&line).ok()))
}

/// The characters of text in a JSON value, counted as it's read: a tool
/// result can be megabytes, and none of it is kept.
pub(crate) struct Size(pub u64);

impl<'de> Deserialize<'de> for Size {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Counter;
        impl<'de> Visitor<'de> for Counter {
            type Value = Size;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("any JSON value")
            }
            fn visit_str<E: de::Error>(self, text: &str) -> Result<Size, E> {
                Ok(Size(text.chars().count() as u64))
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Size, E> {
                Ok(Size(0))
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Size, E> {
                Ok(Size(0))
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Size, E> {
                Ok(Size(0))
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Size, E> {
                Ok(Size(0))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Size, E> {
                Ok(Size(0))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<Size, A::Error> {
                let mut total = 0;
                while let Some(Size(size)) = items.next_element()? {
                    total += size;
                }
                Ok(Size(total))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Size, A::Error> {
                let mut total = 0;
                while let Some(key) = entries.next_key::<String>()? {
                    // A block's `type` names it; it isn't part of what it says.
                    if key == "type" {
                        entries.next_value::<de::IgnoredAny>()?;
                    } else {
                        total += entries.next_value::<Size>()?.0;
                    }
                }
                Ok(Size(total))
            }
        }
        deserializer.deserialize_any(Counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against this computer's own sessions, the largest of each agent's, to
    /// see what the rules make of real ones and how long reading takes:
    /// `cargo test --lib companion_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn companion_live_reads_this_computers_sessions() {
        let mut files = list(0);
        files.sort_by_key(|file| std::cmp::Reverse(file.bytes));
        for tool in ["claude", "codex", "gemini", "copilot", "grok"] {
            let of_tool: Vec<&SessionFile> = files.iter().filter(|file| file.tool == tool).collect();
            println!("\n== {tool}: {} sessions", of_tool.len());
            for file in of_tool.iter().take(3) {
                let started = std::time::Instant::now();
                let Some(session) = read(tool, &file.session_id) else {
                    println!("{} DID NOT READ", file.session_id);
                    continue;
                };
                println!(
                    "{} · {:.1} MB in {:?} · {} turns · {:?} · est {:?} · reported {:?} · {:?}",
                    session.session_id,
                    file.bytes as f64 / 1e6,
                    started.elapsed(),
                    session.turns.len(),
                    session.tokens,
                    session.cost_usd,
                    session.reported_cost_usd,
                    session.title,
                );
                if let Some(turn) = session.turns.iter().find(|turn| !turn.automatic) {
                    println!("  first: {} chars, {} requests, {} tools, model {:?}", turn.prompt_chars, turn.request_count, turn.tool_calls, turn.model);
                }
                for finding in crate::companion::rules::findings(&session) {
                    println!("  {:?} {:?} on {} turns, wasted {:?}: {:?}", finding.severity, finding.kind, finding.turns.len(), finding.wasted_usd, finding.numbers);
                }
            }
        }
    }
}
