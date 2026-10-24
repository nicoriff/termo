//! The agents' own process (see mod.rs): `termo --agents <build> <folder>`.

use std::collections::HashMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde_json::Value;

use crate::agents::hub::{Hub, Sink};
use crate::agents::ops::{self, Agents};
use crate::agents::orchestration::executor;
use crate::agents::{console_bridge, console_orchestration, runner, store, webhook};
use crate::config::store as config_store;
use crate::i18n::{self, Lang};
use crate::mux::protocol::{read_frame, read_frame_within, write_frame, Frame};

use super::notify::Notifier;
use super::protocol::{FromHost, HostStatus, ToHost, PROTOCOL};
use super::{read_info, remove_info, write_info, HostInfo};

/// How often it looks at the config, its own info file and whether it may
/// leave.
const LOOK: Duration = Duration::from_secs(2);
/// A hello is small: anything bigger before the token is a stranger.
const HELLO_MAX: usize = 4096;
/// The last language the app said, for what this process says when it
/// starts on its own (at sign-in).
const LANGUAGE_FILE: &str = "language";

/// The apps connected — and observers (see ToHost::Hello) —, each with
/// what's waiting to be written to it and whether it only watches.
#[derive(Default)]
struct Clients {
    next: AtomicU64,
    senders: Mutex<HashMap<u64, (Sender<FromHost>, bool)>>,
}

impl Clients {
    fn add(&self, sender: Sender<FromHost>, observer: bool) -> u64 {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        self.senders.lock().unwrap().insert(id, (sender, observer));
        id
    }

    fn remove(&self, id: u64) {
        self.senders.lock().unwrap().remove(&id);
    }

    /// Whether an app is there to tell the user what happens.
    fn any(&self) -> bool {
        self.senders.lock().unwrap().values().any(|(_, observer)| !observer)
    }

    fn broadcast(&self, message: &FromHost) {
        self.senders.lock().unwrap().retain(|_, (sender, _)| sender.send(message.clone()).is_ok());
    }
}

/// Where what the agents say goes: to every app connected and, with none,
/// to the user as a notification.
struct HostSink {
    clients: Arc<Clients>,
    notifier: Arc<Notifier>,
}

impl Sink for HostSink {
    fn emit(&self, event: &str, payload: Value) {
        // Heard either way: what an orchestration was is what tells whether
        // it's worth saying, later, with nobody there.
        self.notifier.heard(event, &payload, !self.clients.any());
        self.clients.broadcast(&FromHost::Event { name: event.to_string(), payload });
    }
}

struct Host {
    dir: PathBuf,
    pid: u32,
    token: String,
    build: String,
    config_path: PathBuf,
    agents: Agents,
    clients: Arc<Clients>,
    notifier: Arc<Notifier>,
    retiring: AtomicBool,
    config_seen: Mutex<Option<SystemTime>>,
    status: Mutex<HostStatus>,
}

fn set_language(language: &str) {
    i18n::set(if language == "en" { Lang::En } else { Lang::Es });
}

/// Runs until it's no longer wanted: the setting turned off, Modo IA off,
/// or an app of another build asked it to leave — and nothing runs.
pub fn run(build: &str, dir: Option<&str>) {
    let Some(dir) = dir.filter(|dir| !dir.is_empty()).map(PathBuf::from).or_else(super::dir) else { return };
    let (Some(root), Some(config_path)) = (super::agents_root(), super::config_path()) else { return };
    if another_is_running(&dir) {
        return;
    }
    let config = config_store::load_from(&config_path);
    if !config.background_agents || !config.ai_mode_enabled {
        return;
    }
    let _ = std::fs::create_dir_all(&root);
    let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else { return };
    let Ok(addr) = listener.local_addr() else { return };
    let pid = std::process::id();
    let token = crate::mux::new_token();
    let info = HostInfo { pid, port: addr.port(), token: token.clone(), version: env!("CARGO_PKG_VERSION").to_string(), build: build.to_string() };
    if write_info(&dir, &info).is_err() {
        return;
    }

    super::set_runs_here(true);
    set_language(std::fs::read_to_string(dir.join(LANGUAGE_FILE)).unwrap_or_default().trim());
    crate::marketplace::agent_install::apply(&config.agent_install_dirs);

    let clients = Arc::new(Clients::default());
    let notifier = Arc::new(Notifier::new(&root, &dir));
    notifier.set_config(&config);
    let hub = Hub::new(root.clone(), Arc::new(HostSink { clients: clients.clone(), notifier: notifier.clone() }));
    let agents = Agents { hub: hub.clone(), runner: Arc::default(), orchestrator: Arc::default() };

    // As the app does when it runs them: what a run Termo didn't see end
    // left, and orchestrations left running.
    runner::remove_leftovers(&root);
    executor::recover(&root);
    console_bridge::read_from_disk(&root);
    console_bridge::set_global(config.ai_mode_enabled && config.agents_from_any_console);
    console_orchestration::set_enabled(config.orchestrations_from_consoles);
    crate::ai_mode::init_agents(&agents, config.ai_mode_enabled);

    let host = Arc::new(Host {
        dir: dir.clone(),
        pid,
        token,
        build: build.to_string(),
        config_seen: Mutex::new(modified(&config_path)),
        config_path,
        agents,
        clients,
        notifier,
        retiring: AtomicBool::new(false),
        status: Mutex::default(),
    });

    {
        let host = host.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let host = host.clone();
                std::thread::spawn(move || host.serve(stream));
            }
        });
    }

    loop {
        std::thread::sleep(LOOK);
        host.look_at_config(false);
        host.publish_status();
        // Another one took over the info file (started by mistake beside
        // this one, which can't be reached any more): it's that one's work.
        if read_info(&dir).is_none_or(|info| info.pid != pid) {
            if host.idle() {
                break;
            }
            continue;
        }
        if host.retiring.load(Ordering::SeqCst) && host.idle() {
            break;
        }
    }
    webhook::stop();
    remove_info(&dir, pid);
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

/// Whether the folder's info file is a live process of Termo's other than
/// this one.
fn another_is_running(dir: &Path) -> bool {
    let Some(info) = read_info(dir) else { return false };
    if info.pid == std::process::id() {
        return false;
    }
    is_termo_process(info.pid)
}

pub(super) fn is_termo_process(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::nothing());
    system.process(pid).is_some_and(|process| process.name().to_string_lossy().to_lowercase().starts_with("termo"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mux::protocol::{read_frame, write_frame, Frame};

    /// The whole of it, in a folder of its own: started, reached with its
    /// token only, answering calls, telling its status, and leaving when
    /// asked. It switches process-wide things on (Modo IA, the consoles'
    /// tokens read from disk), hence run alone:
    /// `cargo test --lib host::server -- --ignored --test-threads=1`.
    #[test]
    #[ignore]
    fn runs_answers_and_leaves_when_asked() {
        let base = std::env::temp_dir().join(format!("termo-host-{}", uuid::Uuid::new_v4().simple()));
        let (dir, root, config) = (base.join("host"), base.join("agents"), base.join("config.json"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&config, r#"{ "background_agents": true, "ai_mode_enabled": true }"#).unwrap();
        std::env::set_var("TERMO_AGENTS_ROOT", &root);
        std::env::set_var("TERMO_CONFIG_PATH", &config);

        let host_dir = dir.display().to_string();
        let host = std::thread::spawn(move || run("test-build", Some(&host_dir)));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let info = loop {
            if let Some(info) = read_info(&dir) {
                break info;
            }
            assert!(std::time::Instant::now() < deadline, "it never started");
            std::thread::sleep(Duration::from_millis(50));
        };

        // A wrong token gets nothing at all.
        let mut stranger = TcpStream::connect(("127.0.0.1", info.port)).unwrap();
        let hello = ToHost::Hello { protocol: PROTOCOL, token: "nope".into(), build: "x".into(), observer: false };
        write_frame(&mut stranger, &Frame::Message(hello)).unwrap();
        stranger.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        assert!(matches!(read_frame::<FromHost>(&mut stranger), Ok(None) | Err(_)));

        let mut reader = TcpStream::connect(("127.0.0.1", info.port)).unwrap();
        reader.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut writer = reader.try_clone().unwrap();
        let mut send = |message: ToHost| write_frame(&mut writer, &Frame::Message(message)).unwrap();
        send(ToHost::Hello { protocol: PROTOCOL, token: info.token.clone(), build: "test-build".into(), observer: false });
        let mut next = || loop {
            match read_frame::<FromHost>(&mut reader).unwrap() {
                // Status comes whenever it changes; the replies are what's asked.
                Some(Frame::Message(FromHost::Status(_))) => continue,
                Some(Frame::Message(message)) => break message,
                other => panic!("unexpected {other:?}"),
            }
        };
        assert!(matches!(next(), FromHost::Welcome { protocol: PROTOCOL, .. }));

        send(ToHost::Call { id: 1, name: "agents_running".into(), args: serde_json::json!({}) });
        assert_eq!(next(), FromHost::Reply { id: 1, result: Ok(serde_json::json!([])) });
        send(ToHost::Call { id: 2, name: "agent_stop".into(), args: serde_json::json!({ "agent_id": "nobody" }) });
        assert_eq!(next(), FromHost::Reply { id: 2, result: Ok(Value::Null) });
        send(ToHost::Call { id: 3, name: "agent_stop".into(), args: serde_json::json!({}) });
        assert!(matches!(next(), FromHost::Reply { id: 3, result: Err(_) }));
        send(ToHost::Call { id: 4, name: "no_such_call".into(), args: serde_json::json!({}) });
        assert!(matches!(next(), FromHost::Reply { id: 4, result: Err(_) }));
        send(ToHost::Call { id: 5, name: "agents_delete".into(), args: serde_json::json!({ "id": "nobody" }) });
        assert_eq!(next(), FromHost::Reply { id: 5, result: Ok(serde_json::json!([])) });

        // The sessions daemon, for the web client: reaches it as an observer.
        std::env::set_var("TERMO_AGENTS_HOST_DIR", &dir);
        let link = crate::mux::agents::HostLink::new(|_, _| {});
        assert_eq!(link.call("agents_running", serde_json::json!({})), Ok(serde_json::json!([])));
        std::env::remove_var("TERMO_AGENTS_HOST_DIR");

        // Asked to leave with nothing running: it does, and its info goes.
        send(ToHost::Retire);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !host.is_finished() {
            assert!(std::time::Instant::now() < deadline, "it never left");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(read_info(&dir).is_none());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn an_info_file_counts_only_for_a_live_termo() {
        let dir = std::env::temp_dir().join(format!("termo-host-info-{}", uuid::Uuid::new_v4().simple()));
        assert!(!another_is_running(&dir));
        // Left by one that's gone: an id no process has.
        let gone = HostInfo { pid: u32::MAX - 7, port: 1, token: "t".into(), version: "v".into(), build: "b".into() };
        write_info(&dir, &gone).unwrap();
        assert!(!another_is_running(&dir));
        // This very process: not another one.
        write_info(&dir, &HostInfo { pid: std::process::id(), ..gone }).unwrap();
        assert!(!another_is_running(&dir));
        let _ = std::fs::remove_dir_all(dir);
    }
}

impl Host {
    /// Nothing that leaving would cut short.
    fn idle(&self) -> bool {
        self.agents.runner.running().is_empty() && self.agents.orchestrator.active_count() == 0
    }

    /// Follows the config the app saves: Modo IA, the consoles' access, the
    /// notifications' switches — and leaves once it's no longer wanted.
    fn look_at_config(&self, now: bool) {
        let seen = modified(&self.config_path);
        {
            let mut last = self.config_seen.lock().unwrap();
            if !now && *last == seen {
                return;
            }
            *last = seen;
        }
        let config = config_store::load_from(&self.config_path);
        crate::marketplace::agent_install::apply(&config.agent_install_dirs);
        crate::ai_mode::set_agents(&self.agents, config.ai_mode_enabled);
        console_bridge::set_global(config.ai_mode_enabled && config.agents_from_any_console);
        console_orchestration::set_enabled(config.orchestrations_from_consoles);
        self.notifier.set_config(&config);
        if !config.background_agents || !config.ai_mode_enabled {
            self.retiring.store(true, Ordering::SeqCst);
        }
    }

    fn current_status(&self) -> HostStatus {
        let specs: Vec<_> = store::list_in(self.agents.hub.root()).into_iter().filter(|spec| spec.enabled).collect();
        let count = |has: fn(&crate::agents::spec::AgentSpec) -> bool| specs.iter().filter(|spec| has(spec)).count();
        HostStatus {
            listening: webhook::listening(),
            running_runs: self.agents.runner.running().len(),
            running_orchestrations: self.agents.orchestrator.active_count(),
            scheduled: count(|spec| spec.triggers.schedule.is_some()),
            watching: count(|spec| spec.triggers.watch.as_ref().is_some_and(|watch| !watch.path.trim().is_empty())),
            on_git: count(|spec| spec.triggers.git.is_some()),
            on_webhook: count(|spec| spec.triggers.webhook.is_some()),
            retiring: self.retiring.load(Ordering::SeqCst),
        }
    }

    /// Tells the apps connected how it's doing, when that changed.
    fn publish_status(&self) {
        let status = self.current_status();
        let mut last = self.status.lock().unwrap();
        if *last != status {
            *last = status.clone();
            drop(last);
            self.clients.broadcast(&FromHost::Status(status));
        }
    }

    fn serve(self: Arc<Self>, stream: TcpStream) {
        let _ = stream.set_nodelay(true);
        let Ok(mut reader) = stream.try_clone() else { return };
        let _ = reader.set_read_timeout(Some(Duration::from_secs(5)));
        let Ok(Some(Frame::Message(ToHost::Hello { token, observer, .. }))) = read_frame_within::<ToHost>(&mut reader, HELLO_MAX) else {
            return;
        };
        if !webhook::token_matches(&token, &self.token) {
            return;
        }
        let _ = reader.set_read_timeout(None);

        let (sender, outgoing) = mpsc::channel::<FromHost>();
        let mut writer = stream;
        std::thread::spawn(move || {
            for message in outgoing {
                if write_frame(&mut writer, &Frame::Message(message)).is_err() || writer.flush().is_err() {
                    break;
                }
            }
        });
        let _ = sender.send(FromHost::Welcome { protocol: PROTOCOL, build: self.build.clone(), pid: self.pid });
        let status = self.current_status();
        *self.status.lock().unwrap() = status.clone();
        let _ = sender.send(FromHost::Status(status));
        let id = self.clients.add(sender.clone(), observer);

        while let Ok(Some(Frame::Message(message))) = read_frame::<ToHost>(&mut reader) {
            match message {
                ToHost::Hello { .. } => {}
                ToHost::Call { id, name, args } => {
                    let (host, sender) = (self.clone(), sender.clone());
                    // Its own thread: some take as long as an agent's answer.
                    std::thread::spawn(move || {
                        let result = ops::call(&host.agents, &name, args);
                        let _ = sender.send(FromHost::Reply { id, result });
                        host.publish_status();
                    });
                }
                ToHost::ConfigChanged => self.look_at_config(true),
                ToHost::Retire => {
                    self.retiring.store(true, Ordering::SeqCst);
                    self.publish_status();
                }
                ToHost::Language { language } => {
                    set_language(&language);
                    let _ = std::fs::write(self.dir.join(LANGUAGE_FILE), &language);
                }
            }
        }
        self.clients.remove(id);
    }
}
