//! Agents the user builds in Termo: a definition (what to do, how, with which
//! tools, when) that Termo runs in the background through Claude Code or
//! Codex (see spec.rs), with its history kept on disk (store.rs).

pub mod draft;
pub mod events;
pub mod integrations;
pub mod runner;
pub mod secrets;
pub mod spec;
pub mod store;
