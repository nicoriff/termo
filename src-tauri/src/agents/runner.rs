//! Running agents in the background: the agent's CLI as a plain child process
//! (no console, no tab), its output read line by line into events, and one
//! run per agent at a time.
//!
//! Every run reports to the frontend through two events: `agent-run` with the
//! run's record whenever its state changes (started, doing something new,
//! ended), and `agent-run-event` with each step for whoever shows its log.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;

use crate::i18n::{tr, Msg};
use crate::marketplace::agent_cli;
use crate::marketplace::model::AgentTarget;

use super::diagnose::{diagnose, Facts};
use super::events::{EventKind, Outcome, Parser, RunEvent, SessionInfo};
use super::secrets;
use super::spec::{build_invocation, copilot_foreign_server_args, with_team_server, AgentSpec, Engine, GIT_SAFETY_ENV, TEAM_TOKEN_VAR};
use super::store::{self, now_millis, RunLink, RunRecord, RunStatus, RunTrigger};
use super::{projects, team, webhook};

/// Lines of stderr kept to explain a failure.
const STDERR_TAIL: usize = 12;

struct Active {
    record: Arc<Mutex<RunRecord>>,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct Runner {
    active: Mutex<HashMap<String, Active>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StepPayload<'a> {
    agent_id: &'a str,
    run_id: &'a str,
    event: &'a RunEvent,
}

/// Variables that would make the CLI think it's running inside another
/// session (see the same list in pty/session.rs).
pub(super) const INHERITED_NOISE: &[&str] = &[
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "TERMO_SESSION_ID",
    "TERMO_EVENTS_DIR",
];

pub(super) fn target(engine: Engine) -> AgentTarget {
    match engine {
        Engine::Claude => AgentTarget::Claude,
        Engine::Codex => AgentTarget::Codex,
        Engine::Gemini => AgentTarget::Gemini,
        Engine::Copilot => AgentTarget::Copilot,
        Engine::Grok => AgentTarget::Grok,
    }
}

/// Removes the files runs leave next to their records while they last (the
/// logic for Claude, a CLI's settings, which for Copilot hold the agent's
/// secrets): a run that ended normally already took its own, so whatever is
/// left at startup is from one Termo didn't see end — it was closed or
/// crashed mid-run.
pub fn remove_leftovers(app: &tauri::AppHandle) {
    let Ok(root) = store::root(app) else { return };
    let Ok(agents) = std::fs::read_dir(&root) else { return };
    for agent in agents.flatten() {
        let Ok(files) = std::fs::read_dir(agent.path().join("runs")) else { continue };
        for file in files.flatten() {
            let name = file.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".system.md") || name.ends_with("-settings.json") {
                let _ = std::fs::remove_file(file.path());
            }
        }
    }
}

/// The MCP configs Copilot CLI reads on its own: the user's, and the ones a
/// folder can bring.
fn copilot_configs(dir: &std::path::Path) -> Vec<serde_json::Value> {
    let mut files: Vec<PathBuf> = [".mcp.json", ".github/mcp.json", ".vscode/mcp.json"]
        .iter()
        .map(|file| dir.join(file))
        .collect();
    if let Some(home) = dirs::home_dir() {
        files.push(crate::agents::spec::copilot_home(&home).join("mcp-config.json"));
    }
    files
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect()
}

/// The folder the run works in: the agent's own, or a private workspace next
/// to its definition when none was chosen.
fn work_dir(root: &std::path::Path, spec: &AgentSpec) -> Result<PathBuf, String> {
    if spec.work_dir.trim().is_empty() {
        let dir = root.join(&spec.id).join("workspace");
        std::fs::create_dir_all(&dir).map_err(|err| tr(Msg::AgentRunFailed(&err.to_string())))?;
        return Ok(dir);
    }
    let dir = PathBuf::from(spec.work_dir.trim());
    if !dir.is_dir() {
        return Err(tr(Msg::AgentWorkDirMissing(spec.work_dir.trim())));
    }
    Ok(dir)
}

/// How long an ACP agent gets to exit once its turn is over and its input
/// closed (Grok takes a few seconds) before it's ended.
const ACP_EXIT_GRACE: Duration = Duration::from_secs(20);

/// Ends the CLI and everything it started (shells, MCP servers).
pub(crate) fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(unix)]
    {
        // The child leads its own process group (see `start`).
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGTERM);
        }
    }
    let _ = child.kill();
}

impl Runner {
    /// The runs going on right now.
    pub fn running(&self) -> Vec<RunRecord> {
        let active = self.active.lock().unwrap();
        active.values().map(|run| run.record.lock().unwrap().clone()).collect()
    }

    pub fn is_running(&self, agent_id: &str) -> bool {
        self.active.lock().unwrap().contains_key(agent_id)
    }

    /// Whether this particular run of the agent is still going.
    pub fn is_running_run(&self, agent_id: &str, run_id: &str) -> bool {
        self.active
            .lock()
            .unwrap()
            .get(agent_id)
            .is_some_and(|run| run.record.lock().unwrap().id == run_id)
    }

    pub fn stop(&self, agent_id: &str) {
        if let Some(run) = self.active.lock().unwrap().get(agent_id) {
            run.stop.store(true, Ordering::Relaxed);
        }
    }

    /// Starts a run and returns its record; the run itself goes on in the
    /// background.
    pub fn start(
        self: &Arc<Self>,
        app: &tauri::AppHandle,
        spec: &AgentSpec,
        trigger: RunTrigger,
        context: Option<&str>,
    ) -> Result<RunRecord, String> {
        self.start_linked(app, spec, trigger, context, None, false)
    }

    /// The same, for a run another agent's run set off (see team.rs), or a
    /// rehearsal (see projects::rehearsal_spec) — which a linked run is too
    /// when the run that set it off was one.
    pub fn start_linked(
        self: &Arc<Self>,
        app: &tauri::AppHandle,
        spec: &AgentSpec,
        trigger: RunTrigger,
        context: Option<&str>,
        link: Option<RunLink>,
        rehearsal: bool,
    ) -> Result<RunRecord, String> {
        // Every way a run starts comes through here: none while Modo IA is off.
        if !crate::ai_mode::enabled() {
            return Err(tr(Msg::AiModeOff));
        }
        if spec.task.trim().is_empty() {
            return Err(tr(Msg::AgentTaskMissing));
        }
        let root = store::root(app).map_err(|err| tr(Msg::AgentRunFailed(&err.to_string())))?;
        if trigger != RunTrigger::Test {
            projects::check_daily_limit(&root, spec)?;
        }
        let rehearsal = rehearsal || link.as_ref().is_some_and(|link| link.rehearsal);
        let rehearsed;
        let spec = if rehearsal {
            rehearsed = projects::rehearsal_spec(spec);
            &rehearsed
        } else {
            spec
        };
        // Nobody to ask in a run that started on its own (run_questions.rs).
        let unattended;
        let spec = if spec.collaboration.ask_user && !super::run_questions::attended(trigger) {
            unattended = AgentSpec {
                collaboration: super::spec::Collaboration { ask_user: false, ..spec.collaboration.clone() },
                ..spec.clone()
            };
            &unattended
        } else {
            spec
        };
        let dir = work_dir(&root, spec)?;
        let bin = agent_cli::require(target(spec.engine), None)?;

        let mut active = self.active.lock().unwrap();
        if active.contains_key(&spec.id) {
            return Err(tr(Msg::AgentAlreadyRunning));
        }
        if let Some(link) = &link {
            team::admit(&link.conversation_id)?;
        }

        let run_id = uuid::Uuid::new_v4().to_string();
        // The agent's logic, for `--append-system-prompt-file`: a file rather
        // than an argument (see spec::Invocation). Removed when the run ends.
        let system_prompt = (spec.engine == Engine::Claude && !spec.instructions.trim().is_empty())
            .then(|| root.join(&spec.id).join("runs").join(format!("{run_id}.system.md")));
        if let Some(path) = &system_prompt {
            let written = std::fs::create_dir_all(path.parent().expect("has a folder"))
                .and_then(|_| std::fs::write(path, spec.instructions.trim()));
            written.map_err(|err| tr(Msg::AgentRunFailed(&err.to_string())))?;
        }
        let mut secret_values = secrets::load(&spec.id, &spec.secrets);
        // Working with other agents: Termo's MCP server, reached with a token
        // only this run holds. Without the port there's no server to reach,
        // and the run simply goes without those tools.
        let team_token = (spec.collaboration.is_on() && webhook::listening())
            .then(|| team::open_session(spec, &run_id, link.as_ref(), rehearsal));
        let team_spec = team_token.as_ref().map(|_| with_team_server(spec, &webhook::mcp_url()));
        if let Some(token) = &team_token {
            secret_values.insert(TEAM_TOKEN_VAR.to_string(), token.clone());
        }
        let invocation = build_invocation(
            team_spec.as_ref().unwrap_or(spec),
            context,
            system_prompt.as_deref().and_then(|path| path.to_str()),
            &secret_values,
        );
        // The CLI's settings for this run (see spec::Invocation), removed when
        // the run ends.
        let file_name = match spec.engine {
            Engine::Copilot => "copilot-settings.json",
            _ => "gemini-policy.toml",
        };
        let run_settings =
            invocation.settings.as_ref().map(|_| root.join(&spec.id).join("runs").join(format!("{run_id}.{file_name}")));
        if let (Some(path), Some(text)) = (&run_settings, &invocation.settings) {
            // They can hold the agent's secrets and its team token.
            let written = std::fs::create_dir_all(path.parent().expect("has a folder"))
                .and_then(|_| super::console_bridge::write_private(path, text));
            written.map_err(|err| tr(Msg::AgentRunFailed(&err.to_string())))?;
        }
        let mut args = invocation.args.clone();
        if let Some(path) = run_settings.as_ref().filter(|_| spec.engine == Engine::Gemini) {
            args.extend(super::spec::gemini_policy_args(path, &dir, super::console_bridge::home().as_deref()));
        }
        if spec.engine == Engine::Copilot {
            if let Some(path) = &run_settings {
                args.push("--additional-mcp-config".into());
                args.push(format!("@{}", path.display()));
            }
            args.extend(copilot_foreign_server_args(team_spec.as_ref().unwrap_or(spec), &copilot_configs(&dir)));
        }
        let mut cmd = Command::new(&bin);
        cmd.args(&args)
            .current_dir(&dir)
            // The prompt goes in through stdin, which is then closed: a CLI
            // that decides to ask something gets EOF instead of hanging.
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("NO_COLOR", "1");
        for key in INHERITED_NOISE {
            cmd.env_remove(key);
        }
        cmd.envs(invocation.env.iter().map(|(name, value)| (name, value)));
        cmd.envs(GIT_SAFETY_ENV.iter().copied());
        if team_token.is_some() && spec.engine == Engine::Claude {
            // Asking another agent takes as long as that agent's run; Claude
            // Code would give up on the call well before.
            let timeout_ms = u64::from(spec.limits.timeout_min.max(1)) * 60_000;
            cmd.env("MCP_TOOL_TIMEOUT", timeout_ms.to_string());
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut child = cmd.spawn().map_err(|err| {
            for path in system_prompt.iter().chain(&run_settings) {
                let _ = std::fs::remove_file(path);
            }
            if let Some(token) = &team_token {
                team::close_session(token);
            }
            tr(Msg::AgentRunFailed(&err.to_string()))
        })?;
        // An ACP session (see acp.rs) is a conversation: its input stays open,
        // in the hands of whoever reads its output, until the turn is over.
        let mut conversation = None;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            match invocation.acp {
                Some(run) => {
                    let driver = super::acp::Driver::new(run, dir.display().to_string());
                    let _ = writeln!(stdin, "{}", driver.start());
                    conversation = Some((driver, stdin));
                }
                None => {
                    let prompt = invocation.stdin;
                    // Its own thread: a CLI that writes before it has read all
                    // of its input can't deadlock against this write.
                    std::thread::spawn(move || {
                        let _ = stdin.write_all(prompt.as_bytes());
                    });
                }
            }
        }
        // Set once an ACP turn is over: the agent then exits on its own, and
        // is ended if it doesn't.
        let turn_over = Arc::new(Mutex::new(None::<Instant>));

        let record = RunRecord {
            id: run_id,
            agent_id: spec.id.clone(),
            trigger,
            status: RunStatus::Running,
            started_at: now_millis(),
            ended_at: None,
            summary: String::new(),
            activity: String::new(),
            cost_usd: None,
            link,
            rehearsal,
            failure: None,
        };
        let _ = store::save_run_in(&root, &record);
        let shared = Arc::new(Mutex::new(record.clone()));
        let stop = Arc::new(AtomicBool::new(false));
        active.insert(
            spec.id.clone(),
            Active {
                record: shared.clone(),
                stop: stop.clone(),
            },
        );
        drop(active);

        let _ = app.emit("agent-run", &record);
        let log = Logger {
            app: app.clone(),
            root: root.clone(),
            agent_id: spec.id.clone(),
            run_id: record.id.clone(),
        };
        log.step(RunEvent {
            kind: EventKind::Started,
            tool: String::new(),
            text: format!("{} {}", bin.display(), dir.display()),
            at: now_millis(),
            // What Termo asked for; the CLI's own report follows, when it
            // gives one (EventKind::Session).
            session: Some(SessionInfo {
                engine: spec.engine,
                model: spec.model.clone(),
                cwd: dir.display().to_string(),
                command: bin.display().to_string(),
                ..SessionInfo::default()
            }),
            ..RunEvent::default()
        });

        let failed = Arc::new(AtomicBool::new(false));
        let last_text = Arc::new(Mutex::new(String::new()));
        let stdout = child.stdout.take().map(|out| {
            let (log, shared, failed, last_text, engine) = (log.clone(), shared.clone(), failed.clone(), last_text.clone(), spec.engine);
            let turn_over = turn_over.clone();
            std::thread::spawn(move || {
                let mut conversation = conversation;
                let mut parser = Parser::new(engine);
                let handle = |events: Vec<RunEvent>, outcome: Outcome| {
                    if outcome.failed {
                        failed.store(true, Ordering::Relaxed);
                    }
                    for event in events {
                        let activity = match event.kind {
                            EventKind::Tool if event.text.is_empty() => Some(event.tool.clone()),
                            EventKind::Tool => Some(format!("{} · {}", event.tool, event.text)),
                            _ => None,
                        };
                        if matches!(event.kind, EventKind::Text | EventKind::Result) && !event.text.is_empty() {
                            *last_text.lock().unwrap() = event.text.clone();
                        }
                        let mut record = shared.lock().unwrap();
                        if let Some(cost) = outcome.cost_usd {
                            record.cost_usd = Some(cost);
                        }
                        if let Some(activity) = activity {
                            record.activity = activity;
                            let _ = log.app.emit("agent-run", &*record);
                        }
                        drop(record);
                        log.step(event);
                    }
                };
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    let (events, outcome) = parser.line(&line, now_millis());
                    handle(events, outcome);
                    let Some((driver, stdin)) = conversation.as_mut() else { continue };
                    use std::io::Write;
                    let step = driver.line(&line);
                    for reply in &step.send {
                        let _ = writeln!(stdin, "{reply}");
                    }
                    if step.done {
                        // Closing its input is what tells the agent to leave.
                        conversation = None;
                        *turn_over.lock().unwrap() = Some(Instant::now());
                    }
                }
                // What a CLI said before its output ended without a result.
                handle(parser.finish(now_millis()), Outcome::default());
                // To explain a failure: what it reported, and its model.
                (parser.errors, parser.model)
            })
        });
        let stderr_tail = Arc::new(Mutex::new(Vec::<String>::new()));
        let stderr = child.stderr.take().map(|err| {
            let (log, tail) = (log.clone(), stderr_tail.clone());
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let mut tail = tail.lock().unwrap();
                    tail.push(line.clone());
                    if tail.len() > STDERR_TAIL {
                        tail.remove(0);
                    }
                    drop(tail);
                    log.step(RunEvent {
                        kind: EventKind::Stderr,
                        tool: String::new(),
                        text: line,
                        at: now_millis(),
                        ..RunEvent::default()
                    });
                }
            })
        });

        let timeout_min = spec.limits.timeout_min.max(1);
        let runner = Arc::clone(self);
        let agent_id = spec.id.clone();
        let agent_name = spec.name.clone();
        let (engine_kind, engine_model) = (spec.engine, spec.model.clone());
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(u64::from(timeout_min) * 60);
            let mut left_open = false;
            let (exit, forced) = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break (Some(status), None),
                    Ok(None) => {}
                    Err(_) => break (None, None),
                }
                // An ACP agent that stays after its turn: the run is done.
                if turn_over.lock().unwrap().is_some_and(|since| since.elapsed() >= ACP_EXIT_GRACE) {
                    left_open = true;
                    kill_tree(&mut child);
                    break (child.wait().ok(), None);
                }
                if stop.load(Ordering::Relaxed) {
                    kill_tree(&mut child);
                    break (child.wait().ok(), Some(RunStatus::Stopped));
                }
                if Instant::now() >= deadline {
                    kill_tree(&mut child);
                    break (child.wait().ok(), Some(RunStatus::TimedOut));
                }
                std::thread::sleep(Duration::from_millis(300));
            };
            // Whatever the CLI printed last is in by now.
            let (reported_errors, reported_model) =
                stdout.and_then(|handle| handle.join().ok()).unwrap_or_default();
            if let Some(handle) = stderr {
                let _ = handle.join();
            }

            let succeeded = (left_open || exit.is_some_and(|status| status.success())) && !failed.load(Ordering::Relaxed);
            let status = forced.unwrap_or(if succeeded { RunStatus::Success } else { RunStatus::Error });
            let answer = last_text.lock().unwrap().clone();
            let summary = match status {
                RunStatus::Stopped => tr(Msg::AgentStopped),
                RunStatus::TimedOut => tr(Msg::AgentTimedOut(timeout_min)),
                RunStatus::Error if answer.is_empty() => {
                    let tail = stderr_tail.lock().unwrap().join("\n");
                    if tail.is_empty() {
                        let code = exit.and_then(|status| status.code()).map_or("?".into(), |code| code.to_string());
                        tr(Msg::AgentExitedWith(&code))
                    } else {
                        tail
                    }
                }
                _ => answer,
            };

            // Why it failed, said precisely: which service, which status.
            let failure = matches!(status, RunStatus::Error | RunStatus::TimedOut).then(|| {
                let stderr = stderr_tail.lock().unwrap().clone();
                let model = if reported_model.is_empty() { engine_model.as_str() } else { reported_model.as_str() };
                diagnose(&Facts {
                    engine: engine_kind,
                    model,
                    errors: &reported_errors,
                    stderr: &stderr,
                    exit_code: exit.and_then(|status| status.code()),
                    timed_out: status == RunStatus::TimedOut,
                })
            });
            if let Some(failure) = &failure {
                log.step(RunEvent {
                    kind: EventKind::Failure,
                    text: failure.message.clone(),
                    at: now_millis(),
                    failure: Some(failure.clone()),
                    ..RunEvent::default()
                });
            }

            let mut record = shared.lock().unwrap();
            record.status = status;
            record.ended_at = Some(now_millis());
            record.summary = summary;
            record.activity = String::new();
            record.failure = failure;
            let finished = record.clone();
            drop(record);

            for path in system_prompt.iter().chain(&run_settings) {
                let _ = std::fs::remove_file(path);
            }
            // Its tools stop working with it: a call still waiting on another
            // agent gives up (and stops that agent's run).
            if let Some(token) = &team_token {
                team::close_session(token);
            }
            let _ = store::save_run_in(&log.root, &finished);
            store::prune_runs_in(&log.root, &finished.agent_id);
            log.step(RunEvent {
                kind: EventKind::Ended,
                tool: String::new(),
                text: finished.summary.clone(),
                at: now_millis(),
                ..RunEvent::default()
            });
            runner.active.lock().unwrap().remove(&agent_id);
            let _ = log.app.emit("agent-run", &finished);
            // The agents that come after this one.
            team::on_finished(&log.root, &finished, &agent_name);
        });

        Ok(record)
    }
}

/// A step of a run that didn't come from its CLI's output (a question to
/// the user: see run_questions.rs), into its log and to the UI like the rest.
pub fn log_step(app: &tauri::AppHandle, root: &std::path::Path, agent_id: &str, run_id: &str, event: RunEvent) {
    let log = Logger { app: app.clone(), root: root.to_path_buf(), agent_id: agent_id.to_string(), run_id: run_id.to_string() };
    log.step(event);
}

/// Writes a run's steps to its log file and forwards them to the UI.
#[derive(Clone)]
struct Logger {
    app: tauri::AppHandle,
    root: PathBuf,
    agent_id: String,
    run_id: String,
}

impl Logger {
    fn step(&self, event: RunEvent) {
        let _ = store::append_log_in(&self.root, &self.agent_id, &self.run_id, &event);
        let _ = self.app.emit(
            "agent-run-event",
            StepPayload {
                agent_id: &self.agent_id,
                run_id: &self.run_id,
                event: &event,
            },
        );
    }
}
