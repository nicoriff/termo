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

    /// Kills every live session — for when the main webview reloads: the JS
    /// state that owned these (the tab tree, `pty-registry.ts`) is about to
    /// be thrown away with it, and without this every shell it had open —
    /// and anything running under it, `claude`/`codex` included — keeps
    /// running with nothing left able to reach or close it.
    pub fn kill_all(&self) {
        let ids: Vec<Uuid> = self.sessions.iter().map(|entry| *entry.key()).collect();
        for id in ids {
            if let Some((_, session)) = self.sessions.remove(&id) {
                let _ = session.kill();
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn discarding_channel() -> Channel<PtyEvent> {
        Channel::new(|_| Ok(()))
    }

    fn spawn_sleeper(manager: &PtyManager) -> Uuid {
        manager
            .spawn("/bin/sh", &["-c".into(), "sleep 30".into()], &[], None, 80, 24, None, discarding_channel())
            .unwrap()
    }

    /// The reload path this exists for (window_commands::on_page_load) has no
    /// per-session ids to give back — it has to be able to clear everything
    /// at once, and leave nothing behind for a later `kill`/`pid` to still
    /// find.
    #[test]
    fn kills_every_session_and_forgets_them_all() {
        let manager = PtyManager::default();
        let a = spawn_sleeper(&manager);
        let b = spawn_sleeper(&manager);
        let pids = manager.pids();
        assert_eq!(pids.len(), 2);

        manager.kill_all();

        assert!(manager.pids().is_empty());
        assert!(manager.kill(a).is_err(), "a already removed by kill_all");
        assert!(manager.kill(b).is_err(), "b already removed by kill_all");
        for pid in pids {
            // The shell (and the `sleep` under it) is actually gone, not
            // just forgotten by the map — give the OS a moment to reap it.
            std::thread::sleep(std::time::Duration::from_millis(200));
            let alive = std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
                .is_ok_and(|status| status.success());
            assert!(!alive, "pid {pid} should have been killed");
        }
    }

    #[test]
    fn kill_all_on_an_empty_manager_does_nothing() {
        let manager = PtyManager::default();
        manager.kill_all();
        assert!(manager.pids().is_empty());
    }
}
