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

use super::events::{parse_line, EventKind, RunEvent};
use super::secrets;
use super::spec::{build_invocation, with_team_server, AgentSpec, Engine, GIT_SAFETY_ENV, TEAM_TOKEN_VAR};
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
const INHERITED_NOISE: &[&str] = &[
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "TERMO_SESSION_ID",
    "TERMO_EVENTS_DIR",
];

fn target(engine: Engine) -> AgentTarget {
    match engine {
        Engine::Claude => AgentTarget::Claude,
        Engine::Codex => AgentTarget::Codex,
    }
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

/// Ends the CLI and everything it started (shells, MCP servers).
fn kill_tree(child: &mut Child) {
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
        let mut cmd = Command::new(&bin);
        cmd.args(&invocation.args)
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
            if let Some(path) = &system_prompt {
                let _ = std::fs::remove_file(path);
            }
            if let Some(token) = &team_token {
                team::close_session(token);
            }
            tr(Msg::AgentRunFailed(&err.to_string()))
        })?;
        if let Some(mut stdin) = child.stdin.take() {
            let prompt = invocation.stdin;
            // Its own thread: a CLI that writes before it has read all of its
            // input can't deadlock against this write.
            std::thread::spawn(move || {
                use std::io::Write;
                let _ = stdin.write_all(prompt.as_bytes());
            });
        }

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
        });

        let failed = Arc::new(AtomicBool::new(false));
        let last_text = Arc::new(Mutex::new(String::new()));
        let stdout = child.stdout.take().map(|out| {
            let (log, shared, failed, last_text, engine) = (log.clone(), shared.clone(), failed.clone(), last_text.clone(), spec.engine);
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    let (events, outcome) = parse_line(engine, &line, now_millis());
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
                }
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
                    });
                }
            })
        });

        let timeout_min = spec.limits.timeout_min.max(1);
        let runner = Arc::clone(self);
        let agent_id = spec.id.clone();
        let agent_name = spec.name.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(u64::from(timeout_min) * 60);
            let (exit, forced) = loop {
                match child.try_wait() {
                    Ok(Some(status)) => break (Some(status), None),
                    Ok(None) => {}
                    Err(_) => break (None, None),
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
            if let Some(handle) = stdout {
                let _ = handle.join();
            }
            if let Some(handle) = stderr {
                let _ = handle.join();
            }

            let succeeded = exit.is_some_and(|status| status.success()) && !failed.load(Ordering::Relaxed);
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

            let mut record = shared.lock().unwrap();
            record.status = status;
            record.ended_at = Some(now_millis());
            record.summary = summary;
            record.activity = String::new();
            let finished = record.clone();
            drop(record);

            if let Some(path) = &system_prompt {
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
            });
            runner.active.lock().unwrap().remove(&agent_id);
            let _ = log.app.emit("agent-run", &finished);
            // The agents that come after this one.
            team::on_finished(&log.root, &finished, &agent_name);
        });

        Ok(record)
    }
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
