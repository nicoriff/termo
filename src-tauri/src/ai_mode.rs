//! Modo IA as Termo's one switch for everything AI (Preferences → Modo IA,
//! `AppConfig.ai_mode_enabled`). Off, the backend does none of it:
//!
//! - no agent runs: the scheduler idles, folders stop being watched, and what
//!   git hooks leave for it meanwhile is dropped rather than all set off when
//!   it's back on (see agents/scheduler.rs);
//! - nothing listens on the agents' local port (webhook and MCP server);
//! - Claude Code's and Codex's hooks and status line aren't rewritten at
//!   launch (installing or removing them stays in Preferences).
//!
//! The interface hides the rest: the agents panel, extensions, the Claude and
//! Codex launchers, session tracking (see app-config.svelte.ts).
//! Switching takes effect at once, no restart.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::agents::host;
use crate::agents::hub::Hub;
use crate::agents::runner::Runner;
use crate::agents::{scheduler, webhook};
use crate::pty;

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// For the tests that talk to a real CLI, which have no app to switch it on.
#[cfg(test)]
pub fn set_enabled_for_tests(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

/// At launch, with the saved setting. The scheduler's thread starts either
/// way and idles while Modo IA is off, so turning it on later needs nothing
/// more from it.
pub fn init(app: &tauri::AppHandle, runner: &Arc<Runner>, on: bool) {
    let Ok(hub) = Hub::for_app(app) else {
        ENABLED.store(on, Ordering::SeqCst);
        return;
    };
    init_agents(&hub, runner, on);
    if on {
        refresh_hooks(app);
    }
}

/// The agents' part, wherever they run: the scheduler's thread (which only
/// acts in the process that runs the agents, see host::runs_here) and,
/// switched on, their local server.
pub fn init_agents(hub: &Hub, runner: &Arc<Runner>, on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
    scheduler::start(hub.clone(), runner.clone());
    if on && host::runs_here() {
        webhook::start(hub.clone(), runner.clone());
    }
}

/// A saved config: acts only when the switch actually moved.
pub fn set(app: &tauri::AppHandle, runner: &Arc<Runner>, on: bool) {
    let Ok(hub) = Hub::for_app(app) else { return };
    if set_agents(&hub, runner, on) && on {
        refresh_hooks(app);
    }
}

/// The agents' part of switching; false when it didn't move. Where the
/// agents run elsewhere, that process follows the config on its own.
pub fn set_agents(hub: &Hub, runner: &Arc<Runner>, on: bool) -> bool {
    if ENABLED.swap(on, Ordering::SeqCst) == on {
        return false;
    }
    if !host::runs_here() {
        return true;
    }
    if on {
        webhook::start(hub.clone(), runner.clone());
    } else {
        webhook::stop();
        for run in runner.running() {
            runner.stop(&run.agent_id);
        }
    }
    true
}

/// The consoles' part: their CLIs' hooks and status line point at this Termo.
fn refresh_hooks(app: &tauri::AppHandle) {
    pty::agent_hooks::refresh_installed(app);
    pty::status_line::refresh_installed(app);
    pty::copilot_status::refresh_installed();
}
