//! Debugging aid: records everything a console exchanges with its PTY —
//! output exactly as ConPTY/the tty sent it, input as the frontend wrote it,
//! and resizes — so a rendering glitch can be replayed outside the app and
//! pinned on a specific byte sequence. Dev builds only, and off unless
//! `TERMO_PTY_RECORD` names a directory when Termo starts; each console then gets its own
//! `<session id>.jsonl` there, one event per line:
//! `{"t":<ms since spawn>,"k":"o"|"i","d":"<base64>"}` or
//! `{"t":…,"k":"r","cols":…,"rows":…}`.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;
use uuid::Uuid;

pub struct Recorder {
    file: Mutex<File>,
    started: Instant,
}

impl Recorder {
    pub fn from_env(id: Uuid, cols: u16, rows: u16) -> Option<Self> {
        // Input is recorded too, passwords included, so an installed build
        // never records, whatever its environment says.
        if !cfg!(debug_assertions) {
            return None;
        }
        let dir = PathBuf::from(std::env::var_os("TERMO_PTY_RECORD")?);
        std::fs::create_dir_all(&dir).ok()?;
        let file = File::create(dir.join(format!("{id}.jsonl"))).ok()?;
        let recorder = Self { file: Mutex::new(file), started: Instant::now() };
        recorder.resize(cols, rows);
        Some(recorder)
    }

    fn line(&self, mut event: serde_json::Value) {
        event["t"] = json!(self.started.elapsed().as_millis() as u64);
        let mut file = self.file.lock().unwrap();
        let _ = writeln!(file, "{event}");
    }

    pub fn output(&self, data: &[u8]) {
        self.line(json!({ "k": "o", "d": STANDARD.encode(data) }));
    }

    pub fn input(&self, data: &[u8]) {
        self.line(json!({ "k": "i", "d": STANDARD.encode(data) }));
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        self.line(json!({ "k": "r", "cols": cols, "rows": rows }));
    }
}
