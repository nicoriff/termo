use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::STANDARD, Engine};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;
use uuid::Uuid;

use super::PtyEvent;

pub struct PtySession {
    pub id: Uuid,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
}

impl PtySession {
    pub fn spawn(
        shell_path: &str,
        shell_args: &[String],
        cwd: Option<&str>,
        cols: u16,
        rows: u16,
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

        // Also strip anything identifying the process that launched Termini
        // itself (e.g. running `tauri dev` from inside an agent/CI shell
        // leaks CLAUDE_CODE_*/CI/NO_COLOR down into every session this app
        // spawns). Whatever context Termini was started from shouldn't leak
        // into fresh interactive shells it opens — a nested `claude`, for
        // instance, would see CLAUDE_CODE_CHILD_SESSION and assume it's a
        // child of that unrelated session (auto-resuming its history) and
        // scale back its own UI/color output accordingly.
        for key in [
            "CI",
            "NO_COLOR",
            "CLAUDE_CODE_CHILD_SESSION",
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_EXECPATH",
            "CLAUDE_PID",
            "CLAUDE_EFFORT",
        ] {
            cmd.env_remove(key);
        }

        let child = pair.slave.spawn_command(cmd)?;
        // Dropping the slave handle in this process lets the OS signal EOF on
        // the master's reader once the child (and any of its own children)
        // that inherited the slave fd/handle actually exit.
        drop(pair.slave);

        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let id = Uuid::new_v4();
        let child = Arc::new(Mutex::new(child));

        let reader_channel = channel;
        let reader_child = Arc::clone(&child);
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = STANDARD.encode(&buf[..n]);
                        if reader_channel.send(PtyEvent::Data { data }).is_err() {
                            break;
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
            let _ = reader_channel.send(PtyEvent::Exit { code });
        });

        Ok(Self {
            id,
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            child,
        })
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

    pub fn kill(&self) -> anyhow::Result<()> {
        self.child.lock().unwrap().kill()?;
        Ok(())
    }
}
