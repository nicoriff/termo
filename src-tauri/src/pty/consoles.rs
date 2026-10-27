//! Every console the app shows, wherever it runs: in this process (see
//! manager.rs), or — with "Keep consoles running" on — in the sessions
//! daemon (see mux/), where it outlives the window and the app.
//!
//! Where a console runs is decided when it starts, so turning the setting
//! on or off applies to new consoles and leaves the open ones be. Either
//! way, while remote access is on, they can all be shown from the tailnet:
//! the app's own ones through the daemon too (see sharing.rs).

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};

use dashmap::DashMap;
use uuid::Uuid;

use super::sharing::Sharing;
use super::{EventSink, PtyManager, SpawnSpec};
use crate::mux::client::{Endpoint, HostEvent, MuxClient};
use crate::mux::protocol::SessionInfo;

#[derive(Default)]
pub struct Consoles {
    local: Arc<PtyManager>,
    sharing: Sharing,
    mux: OnceLock<Option<Arc<MuxClient>>>,
    /// Other computers' daemons, over the tailnet (see mux/remote.rs).
    remotes: DashMap<SocketAddr, Arc<MuxClient>>,
}

impl Consoles {
    /// The daemon's client; connecting is left for the first request.
    fn mux(&self) -> Option<&Arc<MuxClient>> {
        self.mux
            .get_or_init(|| {
                let mux = MuxClient::new(Endpoint::Dir(crate::mux::dir()?));
                // What the tailnet asks of the consoles the app runs itself.
                let local = Arc::clone(&self.local);
                let sharing = self.sharing.clone();
                mux.on_host(move |event| match event {
                    HostEvent::Input { session, bytes } => {
                        let _ = local.write(session, &bytes);
                    }
                    HostEvent::Resize { session, cols, rows } => {
                        if local.resize(session, cols, rows).is_ok() {
                            sharing.resized_from_tailnet(session, cols, rows);
                        }
                    }
                    HostEvent::PromptRedraw { session } => {
                        let _ = local.prompt_redraw(session, shell_is_idle);
                    }
                    HostEvent::Open { request, choice, cwd } => sharing.open(request, choice, cwd),
                    HostEvent::Close { session } => sharing.close(session),
                    HostEvent::AgentsCall { request, name, args } => sharing.agents_call(request, name, args),
                    HostEvent::AgentsWatch { on } => sharing.set_agents_watched(on),
                    HostEvent::AgentsChanged => sharing.agents_changed(),
                });
                Some(mux)
            })
            .as_ref()
    }

    /// What the status bar knows of the agent in a console, for whoever
    /// shows it from the tailnet (see sharing.rs).
    pub fn set_agent(&self, id: Uuid, agent: Option<crate::mux::protocol::AgentInfo>) {
        self.sharing.set_agent(self.mux_if_used().map(Arc::as_ref), id, agent);
    }

    /// Remote access turned on or off (see sharing.rs).
    pub fn share(&self, enabled: bool) {
        self.sharing.set_enabled(self.mux().cloned(), enabled);
    }

    /// What can be opened from the tailnet: what the new-tab menu offers,
    /// and the folders to offer first.
    pub fn set_open_choices(&self, choices: Vec<crate::mux::protocol::OpenChoice>, recent: Vec<String>) {
        self.sharing.set_choices(choices, recent);
    }

    /// Where a new console asked from the tailnet goes (see sharing.rs).
    pub fn on_open(&self, handle: impl Fn(u64, String, Option<String>) + Send + Sync + 'static) {
        self.sharing.on_open(handle);
    }

    /// Where a console closed from the tailnet goes (see sharing.rs).
    pub fn on_close(&self, handle: impl Fn(Uuid) + Send + Sync + 'static) {
        self.sharing.on_close(handle);
    }

    /// What came of it: the new console's id, or why not.
    pub fn opened(&self, request: u64, result: Result<Uuid, String>) {
        if let Some(mux) = self.mux_if_used() {
            mux.opened(request, result);
        }
    }

    /// Where what the tailnet asks of the agents goes (see sharing.rs).
    pub fn on_agents_call(&self, handle: impl Fn(u64, String, serde_json::Value) + Send + Sync + 'static) {
        self.sharing.on_agents_call(handle);
    }

    /// What came of it.
    pub fn agents_answer(&self, request: u64, result: Result<serde_json::Value, String>) {
        if let Some(mux) = self.mux_if_used() {
            mux.agents_answer(request, result);
        }
    }

    /// Where an agent saved or deleted from the tailnet is told.
    pub fn on_agents_changed(&self, handle: impl Fn() + Send + Sync + 'static) {
        self.sharing.on_agents_changed(handle);
    }

    /// What an agent said, for whoever watches from the tailnet.
    pub fn agent_event(&self, name: &str, payload: serde_json::Value) {
        self.sharing.agent_event(self.mux_if_used().map(Arc::as_ref), name, payload);
    }

    /// Starts a console in the app itself.
    fn spawn_here(&self, owner: &str, spec: &SpawnSpec, sink: EventSink) -> anyhow::Result<Uuid> {
        let id = Uuid::new_v4();
        let sink = self.sharing.tap(self.mux().cloned(), id, spec, sink);
        match self.local.spawn_as(owner, id, spec, sink) {
            Ok(id) => {
                self.sharing.started(id, self.local.pid(id).ok().flatten());
                Ok(id)
            }
            Err(err) => {
                self.sharing.forget(None, id);
                Err(err)
            }
        }
    }

    /// The client, only if something already asked for it.
    fn mux_if_used(&self) -> Option<&Arc<MuxClient>> {
        self.mux.get().and_then(Option::as_ref)
    }

    /// The client of the daemon — this computer's or another's — that shows
    /// a console.
    fn mux_for(&self, id: Uuid) -> Option<Arc<MuxClient>> {
        if let Some(mux) = self.mux_if_used().filter(|mux| mux.shows(id)) {
            return Some(Arc::clone(mux));
        }
        self.remotes.iter().find(|remote| remote.shows(id)).map(|remote| Arc::clone(remote.value()))
    }

    fn is_remote(&self, id: Uuid) -> bool {
        self.remotes.iter().any(|remote| remote.shows(id))
    }

    /// The client of another computer's daemon.
    pub fn remote(&self, addr: SocketAddr) -> Arc<MuxClient> {
        Arc::clone(self.remotes.entry(addr).or_insert_with(|| MuxClient::new(Endpoint::Remote(addr))).value())
    }

    /// The client of this computer's daemon, connecting to it only if it's
    /// already running.
    pub fn local_daemon(&self) -> Option<&Arc<MuxClient>> {
        self.mux()
    }

    /// Calls `report` whenever this computer's daemon stops answering, or
    /// answers again.
    pub fn on_daemon_health(&self, report: impl Fn(bool) + Send + Sync + 'static) {
        if let Some(mux) = self.mux() {
            mux.on_health(report);
        }
    }

    pub fn spawn(&self, owner: &str, spec: &SpawnSpec, sink: EventSink, persistent: bool) -> anyhow::Result<Uuid> {
        if let Some(mux) = self.mux().filter(|_| persistent) {
            // The sink goes to the daemon's console; a copy stays, should
            // the console have to open here after all.
            let shared = Arc::clone(&sink);
            match mux.spawn(owner, spec, sink) {
                Ok(id) => return Ok(id),
                // The daemon can't start it — it isn't answering, or won't
                // start: the console opens anyway, in the app, as if "Keep
                // consoles running" were off. The status bar says why (see
                // on_daemon_health).
                Err(err) => {
                    eprintln!("sessions daemon: {err}; the console runs in the app");
                    return self.spawn_here(owner, spec, shared);
                }
            }
        }
        self.spawn_here(owner, spec, sink)
    }

    /// Shows a console the daemon kept running — this computer's, or with
    /// `host`, another's on the tailnet.
    pub fn attach(&self, owner: &str, id: Uuid, sink: EventSink, host: Option<SocketAddr>) -> anyhow::Result<SessionInfo> {
        if let Some(host) = host {
            return self.remote(host).attach(owner, id, sink).map_err(anyhow::Error::msg);
        }
        let mux = self.mux().ok_or_else(|| anyhow::anyhow!("no folder for the sessions daemon"))?;
        mux.attach(owner, id, sink).map_err(anyhow::Error::msg)
    }

    /// The consoles the daemon runs, shown or not — none if it isn't
    /// running (asking doesn't start it).
    pub fn background(&self) -> Vec<SessionInfo> {
        let all = self.mux().map(|mux| mux.list_if_running()).unwrap_or_default();
        // Not the app's own, lent to it while remote access is on.
        all.into_iter().filter(|session| !session.hosted).collect()
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
            None => {
                self.local.resize(id, cols, rows)?;
                self.sharing.resized(self.mux_if_used().map(Arc::as_ref), id, cols, rows);
                Ok(())
            }
        }
    }

    /// The size its tab has, back, when the tab is taken up again: someone
    /// on the tailnet (a phone) may have sized it to their screen
    /// meanwhile. Whether it changed — its prompt then wants drawing again.
    pub fn claim_size(&self, id: Uuid, cols: u16, rows: u16) -> anyhow::Result<bool> {
        match self.mux_for(id) {
            Some(mux) => mux.claim(id, cols, rows).map_err(anyhow::Error::msg),
            None => {
                if self.local.size(id)? == (cols, rows) {
                    return Ok(false);
                }
                self.resize(id, cols, rows)?;
                Ok(true)
            }
        }
    }

    pub fn prompt_redraw(&self, id: Uuid, shell_is_idle: impl FnOnce(u32) -> bool) -> anyhow::Result<()> {
        // Whether another computer's shell is at its prompt can't be told
        // from here: it gets no keys.
        if self.is_remote(id) {
            return Ok(());
        }
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

    /// Ends a console — but one of another computer's only stops being
    /// shown here: closing its tab shouldn't end it there.
    pub fn kill(&self, id: Uuid) -> anyhow::Result<()> {
        if self.is_remote(id) {
            if let Some(remote) = self.mux_for(id) {
                remote.detach(id);
            }
            return Ok(());
        }
        match self.mux_for(id) {
            Some(_) => self.kill_kept(id),
            None => {
                self.sharing.forget(self.mux_if_used().map(Arc::as_ref), id);
                self.local.kill(id)
            }
        }
    }

    /// Stops showing a console without ending it: the daemon (this
    /// computer's or another's) keeps it running, and its tab attaches to it
    /// again later — what putting a tab to sleep does. False for a console
    /// only this process runs: letting go of it would end it.
    pub fn release(&self, id: Uuid) -> bool {
        match self.mux_for(id) {
            Some(mux) => {
                mux.detach(id);
                true
            }
            None => false,
        }
    }

    /// Ends a console the daemon runs, shown here or not (the background
    /// sessions menu).
    pub fn kill_kept(&self, id: Uuid) -> anyhow::Result<()> {
        let mux = self.mux().ok_or_else(|| anyhow::anyhow!("no folder for the sessions daemon"))?;
        // Its token for the user's agents dies with it (console_bridge.rs).
        crate::agents::console_bridge::release(&id.to_string());
        mux.kill(id).map_err(anyhow::Error::msg)
    }

    /// None for another computer's console: its process id means nothing
    /// here.
    pub fn pid(&self, id: Uuid) -> anyhow::Result<Option<u32>> {
        if self.is_remote(id) {
            return Ok(None);
        }
        match self.mux_for(id) {
            Some(mux) => Ok(mux.pid(id)),
            None => self.local.pid(id),
        }
    }

    /// The consoles closing Termo (or `owner`'s window) leaves running: in
    /// this computer's daemon, or another computer's — with their process
    /// ids here, none for another computer's.
    pub fn kept(&self, owner: Option<&str>) -> Vec<(Uuid, Option<u32>)> {
        let mut kept = self.mux_if_used().map(|mux| mux.shown_of(owner)).unwrap_or_default();
        for remote in self.remotes.iter() {
            kept.extend(remote.shown_of(owner).into_iter().map(|(id, _)| (id, None)));
        }
        kept
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
        for id in self.local.kill_owned_by(owner) {
            self.sharing.forget(self.mux_if_used().map(Arc::as_ref), id);
        }
        if let Some(mux) = self.mux_if_used() {
            mux.detach_owned_by(owner);
        }
        for remote in self.remotes.iter() {
            remote.detach_owned_by(owner);
        }
    }
}

/// Whether the shell `pid` sits at its prompt: nothing running under it.
/// For a phone that resized one of the app's consoles (see sharing.rs).
fn shell_is_idle(pid: u32) -> bool {
    static PROCESSES: std::sync::LazyLock<super::MemorySampler> = std::sync::LazyLock::new(Default::default);
    PROCESSES.busy_roots(&[pid]).is_empty()
}
