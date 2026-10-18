//! The app's own consoles, shown from the tailnet too while remote access
//! is on. Only the sessions daemon (see mux/) faces the tailnet, so each
//! console the app runs itself — opened before "Keep consoles running" was
//! on, or with it off — is lent to it: the daemon is told of it (Op::Host),
//! gets what it prints, and passes back what's typed into it from there,
//! and the size a phone asks for. They are shown for as long as the app
//! runs them; closing Termo still ends them.
//!
//! Each console keeps its latest output regardless, so that turning remote
//! access on shows what's on its screen already, not only what comes next.
//!
//! The agents' state goes the same way, for every console the app shows —
//! the daemon's own too: it's the app that hears the agents' hooks.
//!
//! And the app is who opens a new console asked for from the tailnet (see
//! Op::Opener): it tells the daemon what its new-tab menu offers, and opens
//! what's asked as a tab of its own, which is then shared like the rest.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use uuid::Uuid;

use super::{ConsoleEvent, EventSink, SpawnSpec};
use crate::mux::client::{MuxClient, Outgoing};
use crate::mux::protocol::{AgentInfo, HostedConsole, Op, OpenChoice};

/// How much of a console's latest output is kept to show it anew.
const TAIL_BYTES: usize = 256 * 1024;
/// How often it looks whether the daemon's connection is a new one (the
/// daemon restarted, or it dropped): everything is shared again over it.
const RECHECK: Duration = Duration::from_secs(3);

#[derive(Default, Clone)]
pub struct Sharing {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    consoles: DashMap<Uuid, Arc<Shared>>,
    enabled: AtomicBool,
    wake: (Mutex<bool>, Condvar),
    supervisor: OnceLock<()>,
    /// The agent in each console, by its id (see set_agent).
    agents: Mutex<HashMap<Uuid, AgentInfo>>,
    /// The daemon connection they were last all told over.
    agents_over: Mutex<Option<u64>>,
    /// What the app offers to open from the tailnet (and the folders its
    /// agents ran in lately), and the daemon connection it was last told
    /// over.
    choices: Mutex<(Vec<OpenChoice>, Vec<String>)>,
    choices_over: Mutex<Option<u64>>,
    /// Where a new console asked from the tailnet goes: the request, the
    /// choice and the folder (see lib.rs).
    on_open: OnceLock<Box<dyn Fn(u64, String, Option<String>) + Send + Sync>>,
    /// Where a console closed from the tailnet goes: its tab closes.
    on_close: OnceLock<Box<dyn Fn(Uuid) + Send + Sync>>,
    /// Where what the tailnet asks of the agents goes (see mux/agents.rs):
    /// the request, the call and its arguments.
    on_agents_call: OnceLock<Box<dyn Fn(u64, String, serde_json::Value) + Send + Sync>>,
    /// Where an agent saved or deleted from there is told.
    on_agents_changed: OnceLock<Box<dyn Fn() + Send + Sync>>,
    /// Someone on the tailnet watches the agents: what they say is passed on.
    agents_watched: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Shared {
    id: Uuid,
    path: String,
    cwd: Option<String>,
    started_at: i64,
    tap: Mutex<Tap>,
}

struct Tap {
    tail: Vec<u8>,
    cols: u16,
    rows: u16,
    pid: Option<u32>,
    foreground: Option<String>,
    /// The daemon connection it's shared over now, if it is.
    over: Option<u64>,
}

impl Sharing {
    /// The sink to give a console the app runs itself: the same events, and
    /// its output kept, and passed on while shared.
    pub fn tap(&self, mux: Option<Arc<MuxClient>>, id: Uuid, spec: &SpawnSpec, sink: EventSink) -> EventSink {
        let shared = Arc::new(Shared {
            id,
            path: spec.path.clone(),
            cwd: spec.cwd.clone(),
            started_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs() as i64),
            tap: Mutex::new(Tap { tail: Vec::new(), cols: spec.cols, rows: spec.rows, pid: None, foreground: None, over: None }),
        });
        self.inner.consoles.insert(id, Arc::clone(&shared));
        // Shared at once rather than at the next look: it may be one the
        // tailnet just asked for, and is waiting on.
        self.wake();
        let consoles = Arc::downgrade(&self.inner);
        Arc::new(move |event| {
            match &event {
                ConsoleEvent::Output(bytes) => shared.output(mux.as_deref(), bytes),
                ConsoleEvent::Exit(_) => {
                    shared.unshare(mux.as_deref());
                    if let Some(inner) = consoles.upgrade() {
                        inner.consoles.remove(&id);
                    }
                }
                ConsoleEvent::Foreground(path) => shared.foreground(mux.as_deref(), path.clone()),
            }
            sink(event)
        })
    }

    /// Its process id, once it has started.
    pub fn started(&self, id: Uuid, pid: Option<u32>) {
        if let Some(shared) = self.inner.consoles.get(&id) {
            shared.tap.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).pid = pid;
        }
    }

    /// The app sized it (its tab did): the daemon's copy follows.
    pub fn resized(&self, mux: Option<&MuxClient>, id: Uuid, cols: u16, rows: u16) {
        if let Some(shared) = self.inner.consoles.get(&id) {
            shared.resized(mux, cols, rows, true);
        }
    }

    /// The tailnet sized it: the app already did what it asked.
    pub fn resized_from_tailnet(&self, id: Uuid, cols: u16, rows: u16) {
        if let Some(shared) = self.inner.consoles.get(&id) {
            shared.resized(None, cols, rows, false);
        }
    }

    /// It ended, or never started.
    pub fn forget(&self, mux: Option<&MuxClient>, id: Uuid) {
        if let Some((_, shared)) = self.inner.consoles.remove(&id) {
            shared.unshare(mux);
        }
    }

    /// What the app's status bar knows of the agent in a console now (None:
    /// there's none), passed on while remote access is on.
    pub fn set_agent(&self, mux: Option<&MuxClient>, id: Uuid, agent: Option<AgentInfo>) {
        let mut agents = self.inner.agents.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let changed = match &agent {
            Some(agent) => agents.insert(id, agent.clone()).as_ref() != Some(agent),
            None => agents.remove(&id).is_some(),
        };
        drop(agents);
        let over = *self.inner.agents_over.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let (true, Some(mux), Some(generation)) = (changed, mux, over) {
            mux.post(generation, Outgoing::Op(Op::Agent { session: id, agent }));
        }
    }

    /// What the new-tab menu offers now, to open from the tailnet, and the
    /// folders to offer first.
    pub fn set_choices(&self, choices: Vec<OpenChoice>, recent: Vec<String>) {
        {
            let mut current = lock(&self.inner.choices);
            if current.0 == choices && current.1 == recent {
                return;
            }
            *current = (choices, recent);
        }
        *lock(&self.inner.choices_over) = None;
        self.wake();
    }

    pub fn on_open(&self, handle: impl Fn(u64, String, Option<String>) + Send + Sync + 'static) {
        let _ = self.inner.on_open.set(Box::new(handle));
    }

    /// The tailnet asks for a new console (see HostEvent::Open).
    pub fn open(&self, request: u64, choice: String, cwd: Option<String>) {
        if let Some(handle) = self.inner.on_open.get() {
            handle(request, choice, cwd);
        }
    }

    pub fn on_close(&self, handle: impl Fn(Uuid) + Send + Sync + 'static) {
        let _ = self.inner.on_close.set(Box::new(handle));
    }

    /// The tailnet closed a console (see HostEvent::Close).
    pub fn close(&self, session: Uuid) {
        if let Some(handle) = self.inner.on_close.get() {
            handle(session);
        }
    }

    pub fn on_agents_call(&self, handle: impl Fn(u64, String, serde_json::Value) + Send + Sync + 'static) {
        let _ = self.inner.on_agents_call.set(Box::new(handle));
    }

    /// The tailnet asks something of the agents (see HostEvent::AgentsCall).
    pub fn agents_call(&self, request: u64, name: String, args: serde_json::Value) {
        if let Some(handle) = self.inner.on_agents_call.get() {
            handle(request, name, args);
        }
    }

    pub fn on_agents_changed(&self, handle: impl Fn() + Send + Sync + 'static) {
        let _ = self.inner.on_agents_changed.set(Box::new(handle));
    }

    pub fn agents_changed(&self) {
        if let Some(handle) = self.inner.on_agents_changed.get() {
            handle();
        }
    }

    pub fn set_agents_watched(&self, on: bool) {
        self.inner.agents_watched.store(on, Ordering::SeqCst);
    }

    /// What an agent said, passed on while someone on the tailnet watches —
    /// over the connection the daemon knows this app opens things on.
    pub fn agent_event(&self, mux: Option<&MuxClient>, name: &str, payload: serde_json::Value) {
        if !self.inner.agents_watched.load(Ordering::SeqCst) || !self.inner.enabled.load(Ordering::SeqCst) {
            return;
        }
        let over = *lock(&self.inner.choices_over);
        if let (Some(mux), Some(generation)) = (mux, over) {
            mux.post(generation, Outgoing::Op(Op::AgentsEvent { name: name.to_string(), payload }));
        }
    }

    fn wake(&self) {
        let (flag, wake) = &self.inner.wake;
        *lock(flag) = true;
        wake.notify_all();
    }

    /// Remote access turned on or off: every console the app runs is
    /// shared, or stops being.
    pub fn set_enabled(&self, mux: Option<Arc<MuxClient>>, enabled: bool) {
        let Some(mux) = mux else {
            return;
        };
        self.inner.enabled.store(enabled, Ordering::SeqCst);
        self.inner.supervisor.get_or_init(|| {
            let inner = Arc::clone(&self.inner);
            std::thread::spawn(move || supervise(&inner, &mux));
        });
        self.wake();
    }
}

/// Keeps every console shared over the daemon's current connection while
/// remote access is on, and none while it's off.
fn supervise(inner: &Inner, mux: &MuxClient) {
    loop {
        let consoles: Vec<Arc<Shared>> = inner.consoles.iter().map(|entry| Arc::clone(entry.value())).collect();
        if inner.enabled.load(Ordering::SeqCst) {
            if let Ok(generation) = mux.connected() {
                for shared in consoles {
                    shared.share(mux, generation);
                }
                let mut over = inner.agents_over.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if *over != Some(generation) {
                    let agents = inner.agents.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
                    for (session, agent) in agents {
                        mux.post(generation, Outgoing::Op(Op::Agent { session, agent: Some(agent) }));
                    }
                    *over = Some(generation);
                }
                let mut over = lock(&inner.choices_over);
                if *over != Some(generation) {
                    let (choices, recent) = lock(&inner.choices).clone();
                    // A new connection: nobody watches the agents on it
                    // until the daemon says so (ServerMessage::AgentsWatch).
                    inner.agents_watched.store(false, Ordering::SeqCst);
                    mux.post(generation, Outgoing::Op(Op::Opener { choices, recent, agents: true }));
                    *over = Some(generation);
                }
            }
        } else {
            for shared in consoles {
                shared.unshare(Some(mux));
            }
            *inner.agents_over.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
            *lock(&inner.choices_over) = None;
        }
        let (flag, wake) = &inner.wake;
        let mut woken = flag.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !*woken {
            woken = wake.wait_timeout(woken, RECHECK).unwrap_or_else(|poisoned| poisoned.into_inner()).0;
        }
        *woken = false;
    }
}

impl Shared {
    fn tap(&self) -> std::sync::MutexGuard<'_, Tap> {
        self.tap.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Under the same lock as its output: what the daemon gets is what it
    /// printed so far, then everything after, nothing twice or missed.
    fn share(&self, mux: &MuxClient, generation: u64) {
        let mut tap = self.tap();
        if tap.over == Some(generation) {
            return;
        }
        let console = HostedConsole {
            path: self.path.clone(),
            cwd: self.cwd.clone(),
            started_at: self.started_at,
            cols: tap.cols,
            rows: tap.rows,
            pid: tap.pid,
        };
        mux.post(generation, Outgoing::Op(Op::Host { session: self.id, console }));
        if !tap.tail.is_empty() {
            mux.post(generation, Outgoing::Output { session: self.id, bytes: tap.tail.clone() });
        }
        if tap.foreground.is_some() {
            mux.post(generation, Outgoing::Op(Op::Foreground { session: self.id, path: tap.foreground.clone() }));
        }
        tap.over = Some(generation);
    }

    fn unshare(&self, mux: Option<&MuxClient>) {
        let over = self.tap().over.take();
        if let (Some(mux), Some(generation)) = (mux, over) {
            mux.post(generation, Outgoing::Op(Op::Unhost { session: self.id }));
        }
    }

    fn output(&self, mux: Option<&MuxClient>, bytes: &[u8]) {
        let mut tap = self.tap();
        tap.tail.extend_from_slice(bytes);
        if tap.tail.len() > 2 * TAIL_BYTES {
            let cut = tap.tail.len() - TAIL_BYTES;
            tap.tail.drain(..cut);
        }
        if let (Some(mux), Some(generation)) = (mux, tap.over) {
            mux.post(generation, Outgoing::Output { session: self.id, bytes: bytes.to_vec() });
        }
    }

    fn foreground(&self, mux: Option<&MuxClient>, path: Option<String>) {
        let mut tap = self.tap();
        tap.foreground = path.clone();
        if let (Some(mux), Some(generation)) = (mux, tap.over) {
            mux.post(generation, Outgoing::Op(Op::Foreground { session: self.id, path }));
        }
    }

    fn resized(&self, mux: Option<&MuxClient>, cols: u16, rows: u16, tell: bool) {
        let mut tap = self.tap();
        tap.cols = cols;
        tap.rows = rows;
        if let (true, Some(mux), Some(generation)) = (tell, mux, tap.over) {
            mux.post(generation, Outgoing::Op(Op::Resize { session: self.id, cols, rows }));
        }
    }
}
