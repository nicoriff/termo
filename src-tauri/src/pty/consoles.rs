//! Every console the app shows, wherever it runs: in this process (see
//! manager.rs), or — with "Keep consoles running" on — in the sessions
//! daemon (see mux/), where it outlives the window and the app.
//!
//! Where a console runs is decided when it starts, so turning the setting
//! on or off applies to new consoles and leaves the open ones be.

use std::sync::{Arc, OnceLock};

use uuid::Uuid;

use super::{EventSink, PtyManager, SpawnSpec};
use crate::mux::client::{Endpoint, MuxClient};
use crate::mux::protocol::SessionInfo;

#[derive(Default)]
pub struct Consoles {
    local: PtyManager,
    mux: OnceLock<Option<Arc<MuxClient>>>,
}

impl Consoles {
    /// The daemon's client; connecting is left for the first request.
    fn mux(&self) -> Option<&Arc<MuxClient>> {
        self.mux
            .get_or_init(|| crate::mux::dir().map(|dir| MuxClient::new(Endpoint::Dir(dir))))
            .as_ref()
    }

    /// The client, only if something already asked for it.
    fn mux_if_used(&self) -> Option<&Arc<MuxClient>> {
        self.mux.get().and_then(Option::as_ref)
    }

    /// The daemon's client, for a console it runs.
    fn mux_for(&self, id: Uuid) -> Option<&Arc<MuxClient>> {
        self.mux_if_used().filter(|mux| mux.shows(id))
    }

    pub fn spawn(&self, owner: &str, spec: &SpawnSpec, sink: EventSink, persistent: bool) -> anyhow::Result<Uuid> {
        if persistent {
            let mux = self.mux().ok_or_else(|| anyhow::anyhow!("no folder for the sessions daemon"))?;
            return mux.spawn(owner, spec, sink).map_err(anyhow::Error::msg);
        }
        self.local.spawn(owner, spec, sink)
    }

    /// Shows a console the daemon kept running.
    pub fn attach(&self, owner: &str, id: Uuid, sink: EventSink) -> anyhow::Result<SessionInfo> {
        let mux = self.mux().ok_or_else(|| anyhow::anyhow!("no folder for the sessions daemon"))?;
        mux.attach(owner, id, sink).map_err(anyhow::Error::msg)
    }

    /// The consoles the daemon runs, shown or not — none if it isn't
    /// running (asking doesn't start it).
    pub fn background(&self) -> Vec<SessionInfo> {
        self.mux().map(|mux| mux.list_if_running()).unwrap_or_default()
    }

    pub fn write(&self, id: Uuid, data: &[u8]) -> anyhow::Result<()> {
        match self.mux_for(id) {
            Some(mux) => mux.write(id, data).map_err(anyhow::Error::msg),
            None => self.local.write(id, data),
        }
    }

    pub fn resize(&self, id: Uuid, cols: u16, rows: u16) -> anyhow::Result<()> {
        match self.mux_for(id) {
            Some(mux) => mux.resize(id, cols, rows).map_err(anyhow::Error::msg),
            None => self.local.resize(id, cols, rows),
        }
    }

    pub fn prompt_redraw(&self, id: Uuid, shell_is_idle: impl FnOnce(u32) -> bool) -> anyhow::Result<()> {
        match self.mux_for(id) {
            Some(mux) => {
                if mux.pid(id).is_some_and(shell_is_idle) {
                    mux.prompt_redraw(id).map_err(anyhow::Error::msg)?;
                }
                Ok(())
            }
            None => self.local.prompt_redraw(id, shell_is_idle),
        }
    }

    pub fn kill(&self, id: Uuid) -> anyhow::Result<()> {
        match self.mux_for(id) {
            Some(mux) => mux.kill(id).map_err(anyhow::Error::msg),
            None => self.local.kill(id),
        }
    }

    /// Ends a console the daemon runs, shown here or not (the background
    /// sessions menu).
    pub fn kill_kept(&self, id: Uuid) -> anyhow::Result<()> {
        let mux = self.mux().ok_or_else(|| anyhow::anyhow!("no folder for the sessions daemon"))?;
        mux.kill(id).map_err(anyhow::Error::msg)
    }

    pub fn pid(&self, id: Uuid) -> anyhow::Result<Option<u32>> {
        match self.mux_for(id) {
            Some(mux) => Ok(mux.pid(id)),
            None => self.local.pid(id),
        }
    }

    /// The process ids of the consoles quitting would end: this process's.
    /// The daemon's keep running.
    pub fn pids(&self) -> Vec<u32> {
        self.local.pids()
    }

    /// The same, for one window's consoles: what closing it would end.
    pub fn pids_of(&self, owner: &str) -> Vec<u32> {
        self.local.pids_of(owner)
    }

    /// Every console shown, with its process id, for the diagnostics panel.
    pub fn roots(&self) -> Vec<(Uuid, u32)> {
        let mut roots = self.local.roots();
        if let Some(mux) = self.mux_if_used() {
            roots.extend(mux.roots());
        }
        roots
    }

    /// A window closed or reloaded: its consoles end — or, the daemon's,
    /// stop being shown and keep running.
    pub fn release_owned_by(&self, owner: &str) {
        self.local.kill_owned_by(owner);
        if let Some(mux) = self.mux_if_used() {
            mux.detach_owned_by(owner);
        }
    }
}
