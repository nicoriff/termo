use dashmap::DashMap;
use tauri::ipc::Channel;
use uuid::Uuid;

use super::{PtyEvent, PtySession};

#[derive(Default)]
pub struct PtyManager {
    sessions: DashMap<Uuid, PtySession>,
}

impl PtyManager {
    pub fn spawn(
        &self,
        shell_path: &str,
        shell_args: &[String],
        env: &[(&str, String)],
        cwd: Option<&str>,
        cols: u16,
        rows: u16,
        prompt_redraw: Option<Vec<u8>>,
        channel: Channel<PtyEvent>,
    ) -> anyhow::Result<Uuid> {
        let session = PtySession::spawn(shell_path, shell_args, env, cwd, cols, rows, prompt_redraw, channel)?;
        let id = session.id;
        self.sessions.insert(id, session);
        Ok(id)
    }

    /// Every live console's process id, for the "something is still running"
    /// check when quitting.
    pub fn pids(&self) -> Vec<u32> {
        self.sessions.iter().filter_map(|session| session.pid()).collect()
    }

    pub fn pid(&self, id: Uuid) -> anyhow::Result<Option<u32>> {
        let session = self
            .sessions
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown pty session: {id}"))?;
        Ok(session.pid())
    }

    pub fn write(&self, id: Uuid, data: &[u8]) -> anyhow::Result<()> {
        let session = self
            .sessions
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown pty session: {id}"))?;
        session.write(data)
    }

    pub fn resize(&self, id: Uuid, cols: u16, rows: u16) -> anyhow::Result<()> {
        let session = self
            .sessions
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown pty session: {id}"))?;
        session.resize(cols, rows)
    }

    pub fn prompt_redraw(&self, id: Uuid, shell_is_idle: impl FnOnce(u32) -> bool) -> anyhow::Result<()> {
        let session = self
            .sessions
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown pty session: {id}"))?;
        session.prompt_redraw(shell_is_idle)
    }

    pub fn kill(&self, id: Uuid) -> anyhow::Result<()> {
        let (_, session) = self
            .sessions
            .remove(&id)
            .ok_or_else(|| anyhow::anyhow!("unknown pty session: {id}"))?;
        session.kill()
    }
}
