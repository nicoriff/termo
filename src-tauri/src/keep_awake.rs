//! Keeping the computer awake while Termo is open, like Caffeine or
//! Amphetamine: while it's on, the system doesn't go to sleep or hibernate
//! on its own (and, if asked, the screen doesn't turn off). Closing the lid
//! or choosing Sleep by hand still works, as with those apps.
//!
//! Each system has its own way, and each is tied to Termo's process so a
//! crash can't leave the computer awake forever:
//! - Windows: `SetThreadExecutionState` from a thread of its own, which
//!   holds the request until told to let go (or the process ends).
//! - macOS: `caffeinate -w <Termo's pid>`, which ends with Termo.
//! - Linux: `systemd-inhibit` around `tail --pid=<Termo's pid>`, which ends
//!   with Termo too.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::Emitter;

use crate::i18n::{tr, Msg};

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeepAwakeState {
    pub active: bool,
    /// The screen stays on too.
    pub display: bool,
    /// When it turns itself off (Unix ms); None: when turned off by hand.
    pub until: Option<i64>,
}

#[derive(Default)]
struct Inner {
    state: KeepAwakeState,
    hold: Option<Hold>,
    /// Bumped on every change, so a timer from an earlier activation doesn't
    /// end a later one.
    generation: u64,
}

#[derive(Default)]
pub struct KeepAwake {
    inner: Mutex<Inner>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// When a session of `minutes` started at `now` ends; none for 0 or none.
pub fn until_for(minutes: Option<u32>, now: i64) -> Option<i64> {
    minutes.filter(|minutes| *minutes > 0).map(|minutes| now + i64::from(minutes) * 60_000)
}

impl KeepAwake {
    pub fn state(&self) -> KeepAwakeState {
        self.inner.lock().unwrap().state.clone()
    }

    /// Turns it on (for `minutes`, or until turned off) or off, and tells
    /// every window.
    pub fn set(
        self: &Arc<Self>,
        app: &tauri::AppHandle,
        active: bool,
        display: bool,
        minutes: Option<u32>,
    ) -> Result<KeepAwakeState, String> {
        let mut inner = self.inner.lock().unwrap();
        // Letting go of the previous hold first: turning the screen option
        // on or off takes a new one.
        inner.hold = None;
        inner.generation += 1;
        inner.state = KeepAwakeState::default();
        if active {
            inner.hold = Some(Hold::start(display)?);
            let until = until_for(minutes, now_ms());
            inner.state = KeepAwakeState { active: true, display, until };
            if let Some(until) = until {
                let (me, app, generation) = (Arc::clone(self), app.clone(), inner.generation);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis((until - now_ms()).max(0) as u64));
                    let mut inner = me.inner.lock().unwrap();
                    if inner.generation == generation {
                        inner.hold = None;
                        inner.generation += 1;
                        inner.state = KeepAwakeState::default();
                        let _ = app.emit("keep-awake", &inner.state);
                    }
                });
            }
        }
        let state = inner.state.clone();
        drop(inner);
        let _ = app.emit("keep-awake", &state);
        Ok(state)
    }
}

/// The system's promise to stay awake, kept for as long as this lives.
#[cfg(windows)]
struct Hold {
    stop: std::sync::mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(windows)]
impl Hold {
    fn start(display: bool) -> Result<Hold, String> {
        use windows_sys::Win32::System::Power::{
            SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
        };
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let (ready, is_ready) = std::sync::mpsc::channel::<bool>();
        // The request belongs to the thread that makes it, so it gets a
        // thread that does nothing else until it's time to let go.
        let thread = std::thread::spawn(move || {
            let flags = ES_CONTINUOUS | ES_SYSTEM_REQUIRED | if display { ES_DISPLAY_REQUIRED } else { 0 };
            let held = unsafe { SetThreadExecutionState(flags) } != 0;
            let _ = ready.send(held);
            if held {
                let _ = stopped.recv();
                unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
            }
        });
        match is_ready.recv() {
            Ok(true) => Ok(Hold { stop, thread: Some(thread) }),
            _ => Err(tr(Msg::KeepAwakeFailed("SetThreadExecutionState"))),
        }
    }
}

#[cfg(windows)]
impl Drop for Hold {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(unix)]
struct Hold {
    child: std::process::Child,
}

#[cfg(unix)]
impl Hold {
    fn start(display: bool) -> Result<Hold, String> {
        use std::process::{Command, Stdio};
        let pid = std::process::id().to_string();
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut command = Command::new("caffeinate");
            // Idle sleep, and system sleep while on power; -d the display.
            command.args(["-i", "-s"]);
            if display {
                command.arg("-d");
            }
            command.args(["-w", &pid]);
            command
        };
        #[cfg(not(target_os = "macos"))]
        let mut command = {
            let mut command = Command::new("systemd-inhibit");
            let what = if display { "sleep:idle" } else { "sleep" };
            let why = tr(Msg::KeepAwakeReason);
            command.args(["--what", what, "--who", "Termo", "--why", &why, "--mode", "block"]);
            command.args(["tail", &format!("--pid={pid}"), "-f", "/dev/null"]);
            command
        };
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| tr(Msg::KeepAwakeFailed(&err.to_string())))?;
        // One that can't do it (no permission, no logind) quits at once.
        std::thread::sleep(Duration::from_millis(200));
        if let Ok(Some(status)) = child.try_wait() {
            return Err(tr(Msg::KeepAwakeFailed(&status.to_string())));
        }
        Ok(Hold { child })
    }
}

#[cfg(unix)]
impl Drop for Hold {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_ends_after_its_minutes_or_never() {
        assert_eq!(until_for(Some(30), 1_000), Some(1_000 + 30 * 60_000));
        assert_eq!(until_for(Some(0), 1_000), None);
        assert_eq!(until_for(None, 1_000), None);
    }

    #[test]
    fn it_starts_off() {
        assert_eq!(KeepAwake::default().state(), KeepAwakeState { active: false, display: false, until: None });
    }

    /// Takes and lets go of the real thing: nothing to see from outside a
    /// test, but it proves the system accepts the request and the hold
    /// comes back on its own thread's exit.
    #[test]
    #[cfg(windows)]
    fn windows_accepts_the_request_and_lets_go() {
        let hold = Hold::start(false).expect("the system takes the request");
        drop(hold);
    }
}
