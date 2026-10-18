//! What the app's windows tell the user about the agents (see
//! agent-attention.ts and orchestration-attention.ts), told by the agents'
//! own process when no app is there to: a run that ended on its own, one
//! waiting on an answer, an orchestration that's done or needs the user.
//! The same switches (Preferences → AI Mode → Notifications) and the same
//! words. Clicked, it opens Termo on what it was about.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::agents::orchestration::plan::{Orchestration, QuestionState, Status};
use crate::agents::store::{self, RunRecord, RunStatus, RunTrigger};
use crate::config::AppConfig;
use crate::i18n::{tr, Msg};

/// How much of the answer goes in the notification.
const MAX_BODY: usize = 140;

/// The flag the app is started with to open what a notification was about
/// (see lib.rs).
pub const OPEN_FLAG: &str = "--open-notification";

/// Where the app's executable is, as the app last said (see client.rs).
pub const APP_PATH_FILE: &str = "app.path";

#[derive(Default, Clone, Copy)]
struct Switches {
    finished: bool,
    error: bool,
    awaiting: bool,
}

pub struct Notifier {
    root: PathBuf,
    dir: PathBuf,
    switches: Mutex<Switches>,
    /// Each orchestration's state and its questions, as last heard: what's
    /// new is what's worth saying.
    orchestrations: Mutex<HashMap<String, (Status, Vec<String>)>>,
    /// Agents whose run on their own failed and already said so.
    failing: Mutex<HashSet<String>>,
}

/// Runs the user started, and is waiting on.
fn asked(trigger: RunTrigger) -> bool {
    matches!(trigger, RunTrigger::Manual | RunTrigger::Console)
}

/// Whether an ended run is worth telling, as the app decides it
/// (agent-attention.ts): one the user started, always; one that ran on its
/// own only when it fails, and once until it works again. Keeps `failed`
/// up to date either way.
fn worth_telling(run: &RunRecord, failed: &mut HashSet<String>) -> bool {
    if run.status == RunStatus::Success {
        failed.remove(&run.agent_id);
        return asked(run.trigger);
    }
    asked(run.trigger) || failed.insert(run.agent_id.clone())
}

fn clip(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > MAX_BODY {
        format!("{}…", text.chars().take(MAX_BODY).collect::<String>())
    } else {
        text
    }
}

impl Notifier {
    pub fn new(root: &Path, dir: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            dir: dir.to_path_buf(),
            switches: Mutex::default(),
            orchestrations: Mutex::default(),
            failing: Mutex::default(),
        }
    }

    pub fn set_config(&self, config: &AppConfig) {
        *self.switches.lock().unwrap() =
            Switches { finished: config.ai_notify_finished, error: config.ai_notify_error, awaiting: config.ai_notify_awaiting };
    }

    /// What the agents said. `alone`: no app is connected to say it.
    pub fn heard(&self, event: &str, payload: &Value, alone: bool) {
        let said = match event {
            "agent-run" => serde_json::from_value::<RunRecord>(payload.clone()).ok().and_then(|run| self.run(&run)),
            "agent-question" => self.question(payload),
            "orchestration" => serde_json::from_value::<Orchestration>(payload.clone()).ok().and_then(|it| self.orchestration(&it)),
            _ => None,
        };
        if let (true, Some((title, body, target))) = (alone, said) {
            show(&self.dir, &title, &body, target);
        }
    }

    /// The agent's name as the app shows it: its icon, then its name.
    fn agent_name(&self, agent_id: &str) -> Option<String> {
        let spec = store::get_in(&self.root, agent_id)?;
        Some(format!("{} {}", spec.icon, spec.name).trim().to_string())
    }

    fn run(&self, run: &RunRecord) -> Option<(String, String, Value)> {
        if run.status == RunStatus::Running {
            return None;
        }
        // Tests are watched as they happen, stopping one was the user's own
        // doing, and an orchestration's tasks speak through it.
        if matches!(run.trigger, RunTrigger::Test | RunTrigger::Orchestrator) || run.status == RunStatus::Stopped {
            return None;
        }
        if !worth_telling(run, &mut self.failing.lock().unwrap()) {
            return None;
        }
        let switches = *self.switches.lock().unwrap();
        let ok = run.status == RunStatus::Success;
        if !(if ok { switches.finished } else { switches.error }) {
            return None;
        }
        let name = self.agent_name(&run.agent_id)?;
        let title = tr(if ok { Msg::AgentNotifySuccess(&name) } else { Msg::AgentNotifyFailed(&name) });
        let plain = store::get_in(&self.root, &run.agent_id).map(|spec| spec.name).unwrap_or_default();
        Some((title, clip(&run.summary), json!({ "kind": "agentRun", "agentId": run.agent_id, "runId": run.id, "title": plain })))
    }

    fn question(&self, question: &Value) -> Option<(String, String, Value)> {
        if question.get("state")?.as_str()? != "pending" || !self.switches.lock().unwrap().awaiting {
            return None;
        }
        let agent_id = question.get("agentId")?.as_str()?;
        let run_id = question.get("runId")?.as_str()?;
        let name = self.agent_name(agent_id)?;
        let plain = store::get_in(&self.root, agent_id).map(|spec| spec.name).unwrap_or_default();
        Some((
            tr(Msg::AgentNotifyQuestion(&name)),
            clip(question.get("question")?.as_str()?),
            json!({ "kind": "agentRun", "agentId": agent_id, "runId": run_id, "title": plain }),
        ))
    }

    fn orchestration(&self, after: &Orchestration) -> Option<(String, String, Value)> {
        let pending: Vec<String> =
            after.questions.iter().filter(|question| question.state == QuestionState::Pending).map(|question| question.id.clone()).collect();
        let before = self.orchestrations.lock().unwrap().insert(after.id.clone(), (after.status, pending.clone()));
        let (before_status, before_questions) = before?;
        let name = format!("{} {}", after.icon, after.name).trim().to_string();
        let target = json!({ "kind": "orchestration", "orchestrationId": after.id, "title": after.name });
        let switches = *self.switches.lock().unwrap();
        if let Some(asked) = after.questions.iter().find(|question| pending.contains(&question.id) && !before_questions.contains(&question.id)) {
            return switches.finished.then(|| (tr(Msg::OrchestrationNotifyQuestion(&asked.agent, &name)), clip(&asked.question), target));
        }
        if before_status != Status::Running || after.status == Status::Running {
            return None;
        }
        let title = match after.status {
            Status::Done => Msg::OrchestrationNotifyDone(&name),
            Status::AwaitingApproval => Msg::OrchestrationNotifyAwaitingApproval(&name),
            Status::Paused => Msg::OrchestrationNotifyPaused(&name),
            Status::Failed => Msg::OrchestrationNotifyFailed(&name),
            _ => return None,
        };
        let wanted = if after.status == Status::Failed { switches.error } else { switches.finished };
        wanted.then(|| (tr(title), clip(&after.note), target))
    }
}

/// Opens Termo on `target`: started with it, or — already open — handed it
/// (the single-instance check passes it on, see lib.rs).
fn open_app(dir: &Path, target: &Value) {
    let Some(exe) = std::fs::read_to_string(dir.join(APP_PATH_FILE)).ok().map(|text| PathBuf::from(text.trim())) else {
        return;
    };
    if !exe.is_file() {
        return;
    }
    let mut command = std::process::Command::new(exe);
    command
        .arg(OPEN_FLAG)
        .arg(target.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let _ = crate::mux::detach(&mut command);
}

#[cfg(windows)]
fn show(dir: &Path, title: &str, body: &str, target: Value) {
    use tauri_winrt_notification::{Duration, Toast};
    // Termo's own id when installed; a dev build has no shortcut that
    // registers it, and Windows drops toasts from an unknown id.
    let app_id = if cfg!(debug_assertions) { Toast::POWERSHELL_APP_ID.to_string() } else { super::IDENTIFIER.to_string() };
    let dir = dir.to_path_buf();
    let _ = Toast::new(&app_id)
        .title(title)
        .text1("")
        .text2(body)
        .duration(Duration::Short)
        .on_activated(move |_| {
            open_app(&dir, &target);
            Ok(())
        })
        .show();
}

#[cfg(target_os = "macos")]
fn show(_dir: &Path, title: &str, body: &str, _target: Value) {
    // Clicked, macOS opens the app the notification is from: Termo.
    let _ = notify_rust::set_application(super::IDENTIFIER);
    let _ = notify_rust::Notification::new().summary(title).body(body).show();
}

#[cfg(all(unix, not(target_os = "macos")))]
fn show(dir: &Path, title: &str, body: &str, target: Value) {
    let shown = notify_rust::Notification::new().appname("Termo").summary(title).body(body).action("default", "Termo").show();
    let Ok(handle) = shown else { return };
    let dir = dir.to_path_buf();
    // Waits for the click (or the notification closing) on its own thread.
    std::thread::spawn(move || {
        handle.wait_for_action(|action| {
            if action == "default" {
                open_app(&dir, &target);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notifier() -> (Notifier, PathBuf) {
        let root = std::env::temp_dir().join(format!("termo-notify-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&root).unwrap();
        let notifier = Notifier::new(&root, &root);
        notifier.set_config(&AppConfig { ai_notify_finished: true, ai_notify_error: true, ai_notify_awaiting: true, ..AppConfig::default() });
        (notifier, root)
    }

    fn orchestration(status: Status) -> Orchestration {
        Orchestration { id: "o1".into(), name: "Docs".into(), status, ..Orchestration::default() }
    }

    #[test]
    fn an_orchestration_speaks_when_it_stops_running() {
        let (notifier, root) = notifier();
        // Heard for the first time: nothing to compare with.
        assert!(notifier.orchestration(&orchestration(Status::Running)).is_none());
        assert!(notifier.orchestration(&orchestration(Status::Running)).is_none());
        let (title, _, target) = notifier.orchestration(&orchestration(Status::Done)).unwrap();
        assert_eq!(title, tr(Msg::OrchestrationNotifyDone("Docs")));
        assert_eq!(target["kind"], "orchestration");
        // Stopped by the user: their own doing.
        notifier.orchestration(&orchestration(Status::Running));
        assert!(notifier.orchestration(&orchestration(Status::Stopped)).is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn quiet_runs_say_nothing() {
        let (notifier, root) = notifier();
        let run = RunRecord {
            id: "r1".into(),
            agent_id: "a1".into(),
            trigger: RunTrigger::Test,
            status: RunStatus::Success,
            started_at: 0,
            ended_at: Some(1),
            summary: String::new(),
            activity: String::new(),
            cost_usd: None,
            link: None,
            rehearsal: false,
            failure: None,
        };
        assert!(notifier.run(&run).is_none());
        // An agent that isn't there any more has nobody to name.
        assert!(notifier.run(&RunRecord { trigger: RunTrigger::Manual, ..run }).is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runs_on_their_own_tell_only_a_new_failure() {
        let ended = |trigger, status| RunRecord {
            id: "r1".into(),
            agent_id: "a1".into(),
            trigger,
            status,
            started_at: 0,
            ended_at: Some(1),
            summary: String::new(),
            activity: String::new(),
            cost_usd: None,
            link: None,
            rehearsal: false,
            failure: None,
        };
        let mut failed = HashSet::new();
        assert!(worth_telling(&ended(RunTrigger::Manual, RunStatus::Success), &mut failed));
        assert!(worth_telling(&ended(RunTrigger::Console, RunStatus::Error), &mut failed));
        assert!(!worth_telling(&ended(RunTrigger::Schedule, RunStatus::Success), &mut failed));
        assert!(worth_telling(&ended(RunTrigger::Schedule, RunStatus::Error), &mut failed));
        assert!(!worth_telling(&ended(RunTrigger::Watch, RunStatus::TimedOut), &mut failed));
        // Worked again: the next failure is news.
        assert!(!worth_telling(&ended(RunTrigger::Schedule, RunStatus::Success), &mut failed));
        assert!(worth_telling(&ended(RunTrigger::Webhook, RunStatus::Error), &mut failed));
    }

    #[test]
    fn long_answers_are_cut() {
        let long = "word ".repeat(100);
        let cut = clip(&long);
        assert!(cut.chars().count() <= MAX_BODY + 1 && cut.ends_with('…'));
        assert_eq!(clip("  two\n words "), "two words");
    }
}
