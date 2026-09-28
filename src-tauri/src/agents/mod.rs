//! Agents the user builds in Termo: a definition (what to do, how, with which
//! tools, when) that Termo runs in the background through Claude Code or
//! Codex (see spec.rs), with its history kept on disk (store.rs).

pub mod draft;
pub mod events;
pub mod git_hooks;
pub mod github;
pub mod integrations;
pub mod portable;
pub mod queue;
pub mod runner;
pub mod scheduler;
pub mod secrets;
pub mod spec;
pub mod store;
pub mod webhook;

/// `termo --run-agent <id> <event>`, called by the git hook Termo installs
/// (see git_hooks.rs) from the repository's folder: leaves a request for the
/// running app, with the commit as context, and returns at once.
pub fn run_agent_cli(agent_id: &str, event: &str) {
    let Some(dir) = queue::dir_outside_app() else { return };
    let here = std::env::current_dir().unwrap_or_default();
    let request = queue::Request {
        agent_id: agent_id.to_string(),
        context: git_hooks::describe_head(&here, event),
    };
    let _ = queue::push(&dir, &request);
}
