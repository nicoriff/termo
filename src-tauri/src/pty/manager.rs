use dashmap::DashMap;
use tauri::ipc::Channel;
use uuid::Uuid;

use super::{PtyEvent, PtySession};

#[derive(Default)]
pub struct PtyManager {
    sessions: DashMap<Uuid, PtySession>,
    /// The window each console belongs to (see terminal_windows.rs): what
    /// closing or reloading that window ends.
    owners: DashMap<Uuid, String>,
}

impl PtyManager {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &self,
        owner: &str,
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
        self.owners.insert(id, owner.to_string());
        Ok(id)
    }

    /// Every live console's process id, for the "something is still running"
    /// check when quitting.
    pub fn pids(&self) -> Vec<u32> {
        self.sessions.iter().filter_map(|session| session.pid()).collect()
    }

    /// The same, for one window's consoles: what closing it would end.
    pub fn pids_of(&self, owner: &str) -> Vec<u32> {
        self.sessions
            .iter()
            .filter(|session| self.owners.get(session.key()).is_some_and(|label| label.as_str() == owner))
            .filter_map(|session| session.pid())
            .collect()
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
        self.owners.remove(&id);
        session.kill()
    }

    /// Kills one window's sessions — for when it reloads or closes: the JS
    /// state that owned these (the tab tree, `pty-registry.ts`) is thrown
    /// away with it, and without this every shell it had open — and anything
    /// running under it, `claude`/`codex` included — would keep running with
    /// nothing left able to reach or close it. Other windows' stay.
    pub fn kill_owned_by(&self, owner: &str) {
        let ids: Vec<Uuid> = self
            .owners
            .iter()
            .filter(|entry| entry.value() == owner)
            .map(|entry| *entry.key())
            .collect();
        for id in ids {
            self.owners.remove(&id);
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

    fn spawn_sleeper(manager: &PtyManager, owner: &str) -> Uuid {
        manager
            .spawn(owner, "/bin/sh", &["-c".into(), "sleep 30".into()], &[], None, 80, 24, None, discarding_channel())
            .unwrap()
    }

    /// The reload and close paths this exists for (window_commands::
    /// on_page_load, terminal_windows::on_destroyed) have no per-session ids
    /// to give back — they clear a window's consoles at once, leave nothing
    /// behind for a later `kill`/`pid` to still find, and leave other
    /// windows' alone.
    #[test]
    fn kills_one_windows_sessions_and_forgets_them() {
        let manager = PtyManager::default();
        let a = spawn_sleeper(&manager, "main");
        let b = spawn_sleeper(&manager, "main");
        let other = spawn_sleeper(&manager, "main-2");
        let pids = manager.pids_of("main");
        assert_eq!(pids.len(), 2);
        assert_eq!(manager.pids().len(), 3);

        manager.kill_owned_by("main");

        assert!(manager.pids_of("main").is_empty());
        assert_eq!(manager.pids_of("main-2").len(), 1, "another window's console stays");
        assert!(manager.kill(a).is_err(), "a already removed by kill_owned_by");
        assert!(manager.kill(b).is_err(), "b already removed by kill_owned_by");
        manager.kill(other).unwrap();
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
    fn killing_a_window_without_consoles_does_nothing() {
        let manager = PtyManager::default();
        manager.kill_owned_by("main");
        assert!(manager.pids().is_empty());
    }
}
