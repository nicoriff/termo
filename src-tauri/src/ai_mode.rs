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

use crate::agents::runner::Runner;
use crate::agents::{scheduler, webhook};
use crate::pty;

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// At launch, with the saved setting. The scheduler's thread starts either
/// way and idles while Modo IA is off, so turning it on later needs nothing
/// more from it.
pub fn init(app: &tauri::AppHandle, runner: &Arc<Runner>, on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
    scheduler::start(app.clone(), runner.clone());
    if on {
        turn_on(app, runner);
    }
}

/// A saved config: acts only when the switch actually moved.
pub fn set(app: &tauri::AppHandle, runner: &Arc<Runner>, on: bool) {
    if ENABLED.swap(on, Ordering::SeqCst) == on {
        return;
    }
    if on {
        turn_on(app, runner);
    } else {
        turn_off(runner);
    }
}

fn turn_on(app: &tauri::AppHandle, runner: &Arc<Runner>) {
    webhook::start(app.clone(), runner.clone());
    pty::agent_hooks::refresh_installed(app);
    pty::status_line::refresh_installed(app);
    pty::copilot_status::refresh_installed();
}

fn turn_off(runner: &Arc<Runner>) {
    webhook::stop();
    for run in runner.running() {
        runner.stop(&run.agent_id);
    }
}
