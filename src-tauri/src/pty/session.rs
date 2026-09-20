use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::STANDARD, Engine};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;
use uuid::Uuid;

use super::PtyEvent;

pub struct PtySession {
    pub id: Uuid,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    pid: Option<u32>,
    // Keys that make this session's shell redraw its prompt at the current
    // width, for the shells that have to be told (see prompt_redraw). None
    // for everything else — zsh redraws off SIGWINCH on its own.
    prompt_redraw: Option<Vec<u8>>,
    // Stops the foreground-process poller thread (see spawn()) once the
    // session is killed — otherwise it would keep polling a dead PTY forever.
    stop: Arc<AtomicBool>,
}

impl PtySession {
    pub fn spawn(
        shell_path: &str,
        shell_args: &[String],
        env: &[(&str, String)],
        cwd: Option<&str>,
        cols: u16,
        rows: u16,
        prompt_redraw: Option<Vec<u8>>,
        channel: Channel<PtyEvent>,
    ) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell_path);
        cmd.args(shell_args);
        if let Some(cwd) = cwd {
            cmd.cwd(cwd);
        }
        // CommandBuilder otherwise inherits Termini's own process
        // environment, which — being a GUI app, not something launched from
        // a terminal — has no TERM/COLORTERM at all. Color-capability
        // detection in CLI tools (chalk/supports-color and similar) uses
        // these to decide between no color, 16-color, 256-color, or
        // truecolor output; without them, richly-colored TUIs (e.g. `claude`)
        // fall back to a reduced palette even though xterm.js renders full
        // 24-bit color just fine. Every real terminal emulator sets these
        // for its child process for exactly this reason.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, value) in env {
            cmd.env(key, value);
        }
        // Lets agent hooks running inside this console report back to its
        // tab (see pty/agent_hooks.rs).
        let id = Uuid::new_v4();
        cmd.env("TERMINI_SESSION_ID", id.to_string());

        // Also strip anything identifying the process that launched Termini
        // itself (e.g. running `tauri dev` from inside an agent/CI shell
        // leaks CLAUDE_CODE_*/CI/NO_COLOR down into every session this app
        // spawns). Whatever context Termini was started from shouldn't leak
        // into fresh interactive shells it opens — a nested `claude`, for
        // instance, would see CLAUDE_CODE_CHILD_SESSION and assume it's a
        // child of that unrelated session (auto-resuming its history) and
        // scale back its own UI/color output accordingly.
        // Same goes for the host terminal's identity: inheriting Terminal.app's
        // TERM_PROGRAM/TERM_SESSION_ID makes macOS's /etc/zshrc run
        // Terminal.app session save/restore against that app's own session.
        for key in [
            "TERM_PROGRAM",
            "TERM_PROGRAM_VERSION",
            "TERM_SESSION_ID",
            "CI",
            "NO_COLOR",
            "CLAUDE_CODE_CHILD_SESSION",
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_EXECPATH",
            "CLAUDE_PID",
            "CLAUDE_EFFORT",
            // EDITOR/VISUAL belong to the same class, and leak louder than
            // they look: zsh reads them at startup and silently switches its
            // line editor to vi mode when either contains "vi". A console
            // that inherited `EDITOR=vi` from the shell `tauri dev` ran in
            // answers Option+Backspace with "ESC = vi command mode, ^? = move
            // left" instead of deleting the word, and the same goes for every
            // other emacs-mode binding. Users who actually want vi mode set
            // EDITOR in their own .zshenv, which zsh reads before it picks
            // the keymap — so dropping it here restores the default without
            // overriding anyone's choice, and their editor still gets set for
            // git and friends by their own startup files.
            "EDITOR",
            "VISUAL",
        ] {
            cmd.env_remove(key);
        }

        let child = pair.slave.spawn_command(cmd)?;
        let pid = child.process_id();
        // Dropping the slave handle in this process lets the OS signal EOF on
        // the master's reader once the child (and any of its own children)
        // that inherited the slave fd/handle actually exit.
        drop(pair.slave);

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let child = Arc::new(Mutex::new(child));
        let master_arc: Arc<Mutex<Box<dyn MasterPty + Send>>> = Arc::new(Mutex::new(pair.master));
        let stop = Arc::new(AtomicBool::new(false));

        // Cloned before the move below consumes `channel` — Channel<T> is a
        // cheap Clone (it's just a dispatcher handle), so both threads can
        // hold their own independent copy.
        #[cfg(unix)]
        let poll_channel = channel.clone();
        let reader_channel = channel;
        let reader_child = Arc::clone(&child);
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            // Once the webview side is gone (tab closed, frontend reloaded) the
            // output is discarded, but reading continues until EOF: on Windows,
            // closing the PTY blocks until its output pipe is drained, so a
            // reader that stopped here would hang whoever closes the session.
            let mut forwarding = true;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if forwarding {
                            let data = STANDARD.encode(&buf[..n]);
                            forwarding = reader_channel.send(PtyEvent::Data { data }).is_ok();
                        }
                    }
                    Err(_) => break,
                }
            }

            let code = reader_child
                .lock()
                .unwrap()
                .wait()
                .ok()
                .map(|status| status.exit_code() as i32)
                .unwrap_or(-1);
            if forwarding {
                let _ = reader_channel.send(PtyEvent::Exit { code });
            }
        });

        // Confirming signal for Modo IA's tool detection: which process is
        // currently in the foreground of this PTY. A dedicated thread rather
        // than piggybacking on the reader thread above, since that one is
        // parked in a *blocking* read() and won't wake up while the session
        // sits idle with no new output — e.g. Claude Code waiting on a
        // permission prompt writes nothing further until the user responds,
        // which is exactly when this signal matters most.
        #[cfg(unix)]
        {
            let poll_master = Arc::clone(&master_arc);
            let poll_stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut last_path: Option<String> = None;
                while !poll_stop.load(Ordering::Relaxed) {
                    let path = poll_master
                        .lock()
                        .unwrap()
                        .process_group_leader()
                        .and_then(super::foreground::process_path);
                    if path != last_path {
                        last_path = path.clone();
                        if poll_channel.send(PtyEvent::ForegroundProcess { path }).is_err() {
                            break;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                }
            });
        }

        Ok(Self {
            id,
            master: master_arc,
            writer: Mutex::new(writer),
            child,
            pid,
            prompt_redraw,
            stop,
        })
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn write(&self, data: &[u8]) -> anyhow::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        self.master.lock().unwrap().resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Makes the shell re-render its prompt where the width it was drawn
    /// against has changed — see oh_my_posh::POWERSHELL_REDRAW_HANDLER for
    /// what this is fixing. A no-op for sessions whose shell needs no
    /// nudging, so the caller doesn't have to know which shell it has.
    ///
    /// The nudge is delivered as keys, so it's only sent while the shell is
    /// sitting at its prompt (`shell_is_idle`, given the shell's pid): with
    /// a command running, whatever that command is — an editor, an agent, a
    /// REPL — would be the one reading them.
    pub fn prompt_redraw(&self, shell_is_idle: impl FnOnce(u32) -> bool) -> anyhow::Result<()> {
        let Some(keys) = &self.prompt_redraw else {
            return Ok(());
        };
        if !self.pid.is_some_and(shell_is_idle) {
            return Ok(());
        }
        self.write(keys)
    }

    pub fn kill(&self) -> anyhow::Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        self.child.lock().unwrap().kill()?;
        Ok(())
    }
}

// NOTE: a PtySession has no terminal of its own, and on Windows ConPTY holds
// the shell's startup until the terminal answers its cursor-position query
// (ESC[6n) — so a test that only spawns sessions here hangs by design. That
// handshake is answered on the frontend; see TerminalInstance's onData handler.

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn console_sees_its_session_id_and_extra_env() {
        let (tx, rx) = mpsc::channel::<String>();
        let tx = Mutex::new(tx);
        let channel = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                let _ = tx.lock().unwrap().send(json);
            }
            Ok(())
        });

        let session = PtySession::spawn(
            "/bin/sh",
            &["-c".into(), r#"printf '%s|%s' "$TERMINI_SESSION_ID" "$TERMINI_TEST_VAR""#.into()],
            &[("TERMINI_TEST_VAR", "hello".to_string())],
            None,
            80,
            24,
            None,
            channel,
        )
        .unwrap();

        let mut output = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while let Ok(json) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            let event: serde_json::Value = serde_json::from_str(&json).unwrap();
            match event["type"].as_str() {
                Some("data") => output.extend(STANDARD.decode(event["data"].as_str().unwrap()).unwrap()),
                Some("exit") => break,
                _ => {}
            }
        }
        let text = String::from_utf8_lossy(&output);
        let _ = session.kill();
        assert!(text.contains(&format!("{}|hello", session.id)), "console output: {text:?}");
    }

    /// Termini started from a shell that exports EDITOR (an agent's, a CI
    /// one) must not pass it down: zsh reads it at startup and switches to
    /// vi mode, which breaks Option+Backspace and every other emacs binding.
    #[test]
    fn console_does_not_inherit_the_launcher_editor() {
        std::env::set_var("EDITOR", "vi");
        std::env::set_var("VISUAL", "vi");

        let (tx, rx) = mpsc::channel::<String>();
        let tx = Mutex::new(tx);
        let channel = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                let _ = tx.lock().unwrap().send(json);
            }
            Ok(())
        });

        let session = PtySession::spawn(
            "/bin/sh",
            &["-c".into(), r#"printf 'EDITOR=[%s] VISUAL=[%s]' "$EDITOR" "$VISUAL""#.into()],
            &[],
            None,
            80,
            24,
            None,
            channel,
        )
        .unwrap();

        let mut output = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while let Ok(json) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            let event: serde_json::Value = serde_json::from_str(&json).unwrap();
            match event["type"].as_str() {
                Some("data") => output.extend(STANDARD.decode(event["data"].as_str().unwrap()).unwrap()),
                Some("exit") => break,
                _ => {}
            }
        }
        let text = String::from_utf8_lossy(&output);
        let _ = session.kill();

        std::env::remove_var("EDITOR");
        std::env::remove_var("VISUAL");

        assert!(text.contains("EDITOR=[] VISUAL=[]"), "console output: {text:?}");
    }
}
