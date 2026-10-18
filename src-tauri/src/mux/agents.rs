//! The user's agents, from the tailnet: what the web client shows of them
//! and does with them (see web-client/), through the sessions daemon.
//!
//! Their definitions and history are files, which the daemon reads and
//! writes itself (see agents/store.rs). Running them is the job of whoever
//! runs the agents — the app, or with "Keep agents running when Termo
//! closes" their own process (see agents/host/) —, so starting or stopping
//! a run, or answering what it asks, is handed over: to the app while it's
//! connected, which hands it on as it would its own (see
//! host::client::call_where_they_run); else straight to the agents' process,
//! as a client that only watches (it still notifies the user itself).
//!
//! Anyone let in may look; running takes full access; creating, changing
//! or deleting an agent takes full access and the user's say-so in
//! Preferences (RemoteConfig::agents_edit), since an agent's task runs on
//! this computer.
//!
//! Errors are words the web client translates, or the agents' own message,
//! already in the user's language.

use std::collections::HashMap;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::protocol::{read_frame, write_frame, Frame};
use crate::agents::host::protocol::{FromHost, ToHost, PROTOCOL as HOST_PROTOCOL};
use crate::agents::spec::{AgentSpec, Autonomy, Engine, Schedule};
use crate::agents::store::{self, RunRecord};
use crate::agents::{projects, scheduler};

/// What a call takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// Anyone let in.
    Look,
    /// Full access.
    Run,
    /// Full access, and agents_edit on.
    Edit,
}

/// What `call` takes; None for a call there's no such thing as.
pub fn need(call: &str) -> Option<Need> {
    match call {
        "overview" | "agent" | "runs" | "run_log" | "models" | "watch" => Some(Need::Look),
        "start" | "stop" | "answer" => Some(Need::Run),
        "save" | "delete" | "set_enabled" => Some(Need::Edit),
        _ => None,
    }
}

/// The calls handed to whoever runs the agents (see ops.rs), and only these.
pub const RUNTIME_CALLS: &[&str] = &["agent_start", "agent_stop", "agent_answer_question", "agent_questions", "agents_running", "agents_delete"];

/// The events the web client hears, of all the agents say.
pub fn worth_passing_on(event: &str) -> bool {
    matches!(event, "agent-run" | "agent-run-event" | "agent-question")
}

/// An agent as the web client's form edits it: the everyday part of a
/// definition. The rest — connectors, other triggers, working with other
/// agents… — is the desktop's, and kept as it is (see apply).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Form {
    /// Empty: a new one.
    pub id: String,
    pub name: String,
    pub engine: Engine,
    pub model: String,
    pub work_dir: String,
    pub task: String,
    pub instructions: String,
    pub autonomy: Autonomy,
    pub web: bool,
    pub schedule: Option<Schedule>,
    pub timeout_min: u32,
    pub enabled: bool,
}

impl Default for Form {
    fn default() -> Self {
        form_of(&AgentSpec::default())
    }
}

pub fn form_of(spec: &AgentSpec) -> Form {
    Form {
        id: spec.id.clone(),
        name: spec.name.clone(),
        engine: spec.engine,
        model: spec.model.clone(),
        work_dir: spec.work_dir.clone(),
        task: spec.task.clone(),
        instructions: spec.instructions.clone(),
        autonomy: spec.autonomy,
        web: spec.tools.web,
        schedule: spec.triggers.schedule.clone(),
        timeout_min: spec.limits.timeout_min,
        enabled: spec.enabled,
    }
}

/// The longest a run may be given from there: a day.
const MAX_TIMEOUT_MIN: u32 = 24 * 60;
const MAX_NAME: usize = 120;
const MAX_TEXT: usize = 64 * 1024;
/// The icon a new agent gets, as the desktop's first (agent-spec.ts).
const NEW_ICON: &str = "🤖";

/// The form, onto the agent it edits (`base`) or a new one. Whatever the
/// form doesn't show stays as it was.
pub fn apply(form: Form, base: Option<AgentSpec>, usable_folder: impl Fn(&str) -> bool) -> Result<AgentSpec, String> {
    let name = form.name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err("bad-name".into());
    }
    let work_dir = form.work_dir.trim();
    if !work_dir.is_empty() && !usable_folder(work_dir) {
        return Err("bad-folder".into());
    }
    if form.task.len() > MAX_TEXT || form.instructions.len() > MAX_TEXT {
        return Err("too-long".into());
    }
    if let Some(schedule) = &form.schedule {
        check_schedule(schedule)?;
    }
    let mut spec = base.unwrap_or_else(|| AgentSpec {
        id: uuid::Uuid::new_v4().to_string(),
        icon: NEW_ICON.to_string(),
        ..AgentSpec::default()
    });
    spec.name = name.to_string();
    spec.engine = form.engine;
    spec.model = form.model.trim().to_string();
    spec.work_dir = work_dir.to_string();
    spec.task = form.task;
    spec.instructions = form.instructions;
    spec.autonomy = form.autonomy;
    spec.tools.web = form.web;
    spec.triggers.schedule = form.schedule;
    spec.limits.timeout_min = form.timeout_min.clamp(1, MAX_TIMEOUT_MIN);
    spec.enabled = form.enabled;
    Ok(spec)
}

fn check_schedule(schedule: &Schedule) -> Result<(), String> {
    let fine = match schedule {
        Schedule::Interval { minutes } => (1..=7 * 24 * 60).contains(minutes),
        Schedule::Daily { at, weekdays } => {
            let time = at.split_once(':').and_then(|(hours, minutes)| {
                let (hours, minutes) = (hours.parse::<u32>().ok()?, minutes.parse::<u32>().ok()?);
                (at.len() == 5 && hours < 24 && minutes < 60).then_some(())
            });
            time.is_some() && weekdays.iter().all(|day| *day < 7)
        }
    };
    if fine {
        Ok(())
    } else {
        Err("bad-schedule".into())
    }
}

/// What an agent has that the web client's form doesn't show, by the
/// names the page translates: told, so changing it there isn't a surprise.
pub fn advanced(spec: &AgentSpec) -> Vec<&'static str> {
    let triggers = &spec.triggers;
    let collaboration = &spec.collaboration;
    [
        ("watch", triggers.watch.as_ref().is_some_and(|watch| !watch.path.trim().is_empty())),
        ("git", triggers.git.is_some()),
        ("webhook", triggers.webhook.is_some()),
        ("after", triggers.after.is_some()),
        ("connectors", !spec.tools.mcp.is_empty()),
        ("team", collaboration.is_on() || collaboration.from_consoles),
        ("deploy", spec.deploy.github.is_some()),
        ("project", !spec.project_id.is_empty()),
    ]
    .into_iter()
    .filter_map(|(name, has)| has.then_some(name))
    .collect()
}

/// An agent in the web client's list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub engine: Engine,
    pub model: String,
    pub work_dir: String,
    pub enabled: bool,
    pub autonomy: Autonomy,
    pub schedule: Option<Schedule>,
    /// What else sets it off on its own: "watch", "git", "webhook", "after".
    pub triggers: Vec<&'static str>,
    /// Unix milliseconds.
    pub next_run: Option<i64>,
    pub last_run: Option<RunRecord>,
}

/// The user's own agents, as the desktop's sidebar lists them: not the
/// teams orchestrations make.
pub fn summaries(root: &Path) -> Vec<Summary> {
    let teams: Vec<String> = projects::list_in(root)
        .into_iter()
        .filter(|project| !project.orchestration_id.is_empty())
        .map(|project| project.id)
        .collect();
    store::list_in(root)
        .into_iter()
        .filter(|spec| !teams.contains(&spec.project_id))
        .map(|spec| {
            let triggers = &spec.triggers;
            let others = [
                ("watch", triggers.watch.as_ref().is_some_and(|watch| !watch.path.trim().is_empty())),
                ("git", triggers.git.is_some()),
                ("webhook", triggers.webhook.is_some()),
                ("after", triggers.after.is_some()),
            ];
            Summary {
                next_run: scheduler::agent_next_run(root, &spec),
                last_run: store::list_runs_in(root, &spec.id).into_iter().next(),
                triggers: others.into_iter().filter_map(|(name, has)| has.then_some(name)).collect(),
                id: spec.id,
                name: spec.name,
                description: spec.description,
                icon: spec.icon,
                engine: spec.engine,
                model: spec.model,
                work_dir: spec.work_dir,
                enabled: spec.enabled,
                autonomy: spec.autonomy,
                schedule: spec.triggers.schedule,
            }
        })
        .collect()
}

/// The newest of a run's steps the page gets: a long run's log can hold
/// thousands, and a phone shows the latest.
pub const MAX_LOG_EVENTS: usize = 400;
/// The runs of an agent the page lists.
pub const MAX_RUNS: usize = 40;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdArgs {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRef {
    pub agent_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRef {
    pub agent_id: String,
    pub run_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartArgs {
    pub agent_id: String,
    /// What to tell it on top of its task, for this run.
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub question_id: String,
    pub answer: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnabledArgs {
    pub id: String,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct EngineArgs {
    pub engine: Engine,
}

#[derive(Deserialize)]
pub struct SaveArgs {
    pub form: Form,
}

/// The arguments of a call, as `T`.
pub fn args<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|_| "bad-arguments".to_string())
}

pub fn start_args(spec: &AgentSpec, context: Option<String>) -> Value {
    let context = context.map(|text| text.trim().to_string()).filter(|text| !text.is_empty());
    json!({ "spec": spec, "trigger": "manual", "context": context })
}

/// A connection to the agents' own process (see agents/host/), made when
/// it's needed and kept while it lasts. It says it only watches, so the
/// process still tells the user itself what happens while no app is there.
pub struct HostLink {
    conn: Mutex<Option<Arc<HostConn>>>,
    on_event: Box<dyn Fn(String, Value) + Send + Sync>,
}

struct HostConn {
    writer: Mutex<TcpStream>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Result<Value, String>>>>,
    next: AtomicU64,
}

/// How long a call to the agents' process may take.
const HOST_WAIT: Duration = Duration::from_secs(30);

impl HostLink {
    pub fn new(on_event: impl Fn(String, Value) + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self { conn: Mutex::new(None), on_event: Box::new(on_event) })
    }

    /// Whether it's connected, connecting first if it isn't (and the
    /// process is there to connect to).
    pub fn reach(self: &Arc<Self>) -> bool {
        self.conn().is_ok()
    }

    fn conn(self: &Arc<Self>) -> Result<Arc<HostConn>, String> {
        let mut slot = self.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(conn) = slot.as_ref() {
            return Ok(Arc::clone(conn));
        }
        let dir = crate::agents::host::dir().ok_or("no-runtime")?;
        let info = crate::agents::host::read_info(&dir).ok_or("no-runtime")?;
        let addr = SocketAddr::from(([127, 0, 0, 1], info.port));
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|_| "no-runtime")?;
        let _ = stream.set_nodelay(true);
        let mut reader = stream.try_clone().map_err(|_| "no-runtime")?;
        let mut writer = stream;
        let hello = ToHost::Hello { protocol: HOST_PROTOCOL, token: info.token, build: "mux".into(), observer: true };
        write_frame(&mut writer, &Frame::Message(hello)).map_err(|_| "no-runtime")?;
        let _ = reader.set_read_timeout(Some(Duration::from_secs(5)));
        let Ok(Some(Frame::Message(FromHost::Welcome { protocol: HOST_PROTOCOL, .. }))) = read_frame::<FromHost>(&mut reader) else {
            return Err("no-runtime".into());
        };
        let _ = reader.set_read_timeout(None);
        let conn = Arc::new(HostConn { writer: Mutex::new(writer), pending: Mutex::default(), next: AtomicU64::new(1) });
        *slot = Some(Arc::clone(&conn));
        let link = Arc::downgrade(self);
        let listening = Arc::clone(&conn);
        std::thread::spawn(move || listen(link, listening, reader));
        Ok(conn)
    }

    /// One of ops::call's, run by the agents' process.
    pub fn call(self: &Arc<Self>, name: &str, args: Value) -> Result<Value, String> {
        let conn = self.conn()?;
        let id = conn.next.fetch_add(1, Ordering::SeqCst);
        let (sender, answer) = mpsc::channel();
        conn.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id, sender);
        let sent = {
            let mut writer = conn.writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            write_frame(&mut *writer, &Frame::Message(ToHost::Call { id, name: name.to_string(), args })).and_then(|()| writer.flush())
        };
        if sent.is_err() {
            conn.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&id);
            return Err("no-runtime".into());
        }
        answer.recv_timeout(HOST_WAIT).unwrap_or_else(|_| {
            conn.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&id);
            Err("timeout".into())
        })
    }
}

fn listen(link: std::sync::Weak<HostLink>, conn: Arc<HostConn>, mut reader: TcpStream) {
    while let Ok(Some(Frame::Message(message))) = read_frame::<FromHost>(&mut reader) {
        match message {
            FromHost::Reply { id, result } => {
                if let Some(waiting) = conn.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&id) {
                    let _ = waiting.send(result);
                }
            }
            FromHost::Event { name, payload } if worth_passing_on(&name) => {
                if let Some(link) = link.upgrade() {
                    (link.on_event)(name, payload);
                }
            }
            _ => {}
        }
    }
    for (_, waiting) in conn.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).drain() {
        let _ = waiting.send(Err("no-runtime".into()));
    }
    if let Some(link) = link.upgrade() {
        let mut slot = link.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.as_ref().is_some_and(|current| Arc::ptr_eq(current, &conn)) {
            *slot = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::spec::{McpServer, Watch};

    #[test]
    fn every_call_says_what_it_takes() {
        assert_eq!(need("overview"), Some(Need::Look));
        assert_eq!(need("start"), Some(Need::Run));
        assert_eq!(need("save"), Some(Need::Edit));
        assert_eq!(need("delete"), Some(Need::Edit));
        assert_eq!(need("agent_start"), None, "ops' own names aren't the page's");
        assert_eq!(need("orchestration_approve"), None);
    }

    #[test]
    fn a_new_agent_gets_what_the_form_says_and_the_defaults() {
        let form = Form {
            name: "  Reviewer ".into(),
            engine: Engine::Codex,
            task: "Review the last commit".into(),
            schedule: Some(Schedule::Daily { at: "09:00".into(), weekdays: vec![1, 2, 3, 4, 5] }),
            timeout_min: 9999,
            enabled: true,
            ..Form::default()
        };
        let spec = apply(form, None, |_| true).unwrap();
        assert_eq!(spec.name, "Reviewer");
        assert_eq!(spec.engine, Engine::Codex);
        assert!(crate::agents::spec::is_valid_id(&spec.id));
        assert_eq!(spec.icon, NEW_ICON);
        assert_eq!(spec.autonomy, Autonomy::Read, "only looks unless told otherwise");
        assert_eq!(spec.limits.timeout_min, MAX_TIMEOUT_MIN);
        assert_eq!(spec.created_at, 0, "the store sets it");
    }

    #[test]
    fn editing_keeps_what_the_form_does_not_show() {
        let mut base = AgentSpec { id: "a1".into(), name: "Old".into(), project_id: "p".into(), ..AgentSpec::default() };
        base.tools.mcp.push(McpServer { name: "github".into(), config: json!({ "url": "https://x" }) });
        base.triggers.watch = Some(Watch { path: "C:\\src".into() });
        base.secrets.push("TOKEN".into());
        let form = Form { name: "New".into(), web: true, ..form_of(&base) };
        let spec = apply(form, Some(base.clone()), |_| true).unwrap();
        assert_eq!(spec.id, "a1");
        assert_eq!(spec.name, "New");
        assert!(spec.tools.web);
        assert_eq!(spec.tools.mcp, base.tools.mcp);
        assert_eq!(spec.triggers.watch, base.triggers.watch);
        assert_eq!(spec.secrets, base.secrets);
        assert_eq!(spec.project_id, "p");
        assert_eq!(advanced(&spec), vec!["watch", "connectors", "project"]);
    }

    #[test]
    fn what_will_not_do_is_refused() {
        let named = |form: Form| Form { name: "x".into(), ..form };
        assert_eq!(apply(Form::default(), None, |_| true).unwrap_err(), "bad-name");
        assert_eq!(apply(named(Form { work_dir: "Z:\\nope".into(), ..Form::default() }), None, |_| false).unwrap_err(), "bad-folder");
        for schedule in [
            Schedule::Interval { minutes: 0 },
            Schedule::Daily { at: "25:00".into(), weekdays: vec![] },
            Schedule::Daily { at: "9:00".into(), weekdays: vec![] },
            Schedule::Daily { at: "09:00".into(), weekdays: vec![7] },
        ] {
            let form = named(Form { schedule: Some(schedule.clone()), ..Form::default() });
            assert_eq!(apply(form, None, |_| true).unwrap_err(), "bad-schedule", "{schedule:?}");
        }
        let fine = named(Form { schedule: Some(Schedule::Interval { minutes: 30 }), ..Form::default() });
        assert!(apply(fine, None, |_| true).is_ok());
    }

    #[test]
    fn the_list_leaves_out_orchestration_teams() {
        let root = std::env::temp_dir().join(format!("termo-remote-agents-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&root).unwrap();
        let team = projects::Project { id: "team".into(), orchestration_id: "o1".into(), ..Default::default() };
        projects::save_in(&root, team).unwrap();
        store::save_in(&root, AgentSpec { id: "mine".into(), name: "Mine".into(), ..AgentSpec::default() }).unwrap();
        store::save_in(&root, AgentSpec { id: "theirs".into(), name: "Theirs".into(), project_id: "team".into(), ..AgentSpec::default() })
            .unwrap();
        let list = summaries(&root);
        assert_eq!(list.iter().map(|agent| agent.id.as_str()).collect::<Vec<_>>(), vec!["mine"]);
        let _ = std::fs::remove_dir_all(root);
    }
}
