//! A repository's harness: one set of rules for every agent working in it
//! (paths they mustn't touch, commands they mustn't run or must ask about,
//! checks that decide when they're done, and what they're told), kept in
//! `.termo/harness.json` and written into each agent's own project files.
//! Those files are committed with the repository, so the rules hold for
//! anyone opening an agent there, with Termo or without.

pub mod checks;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod detect;
pub mod external;
pub mod gemini;
pub mod instructions;
pub mod json_merge;
pub mod markers;
pub mod model;
pub mod sync;
