//! Asking the user, mid-run, outside an orchestration (whose questions live
//! on its plan: see orchestration/questions.rs). An agent the user allowed to
//! ask, in a run someone is there for — started by hand, or from one of the
//! user's consoles — gets `ask_user` from Termo's MCP server: the question
//! shows in the run's view, and the tool call that asked waits there for the
//! answer. A run a console is waiting on hands the question to that console
//! as well (see console_bridge.rs); whichever answers first is the answer.
//!
//! A question only matters while its run lives, so they're kept in memory:
//! what was asked and answered stays in the run's log.

use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;

use super::events::{EventKind, RunEvent};
use super::orchestration::plan::QuestionState;
use super::orchestration::questions::{answer_text, tidy, NO_ANSWER, WAIT};
use super::runner;
use super::store::{self, now_millis, RunTrigger};
use super::team::{self, Session};

const POLL: Duration = Duration::from_secs(1);
/// What's left of the run's own time limit after a question gives up, to
/// finish the work in.
const ROOM_TO_FINISH: Duration = Duration::from_secs(2 * 60);

/// Something a run asked the user and is waiting on.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunQuestion {
    pub id: String,
    pub agent_id: String,
    pub run_id: String,
    pub question: String,
    /// Answers it suggests; the user may write another.
    pub options: Vec<String>,
    pub state: QuestionState,
    pub answer: String,
    /// Unix milliseconds.
    pub asked_at: i64,
    /// When it stops waiting and the agent decides alone.
    pub expires_at: i64,
}

fn questions() -> &'static Mutex<Vec<RunQuestion>> {
    static QUESTIONS: OnceLock<Mutex<Vec<RunQuestion>>> = OnceLock::new();
    QUESTIONS.get_or_init(Mutex::default)
}

/// The runs someone is there to answer: the ones the user starts, by hand
/// or from a console, and an orchestration's (asked on its plan). One that
/// starts on its own — a schedule, a change in a folder, another agent —
/// has nobody to ask.
pub fn attended(trigger: RunTrigger) -> bool {
    matches!(trigger, RunTrigger::Manual | RunTrigger::Console | RunTrigger::Orchestrator)
}

/// How long a question asked `now` can wait in a run that started at
/// `started_at` with `timeout_min` to work in: as long as an orchestration's,
/// but never into the room the run needs to finish. None when there's no
/// time left to wait at all.
pub fn wait_for(started_at: i64, timeout_min: u32, now: i64) -> Option<Duration> {
    let ends = started_at + i64::from(timeout_min.max(1)) * 60_000;
    let left = ends - now - ROOM_TO_FINISH.as_millis() as i64;
    (left > 0).then(|| WAIT.min(Duration::from_millis(left as u64)))
}

/// The questions waiting on the user now.
pub fn pending() -> Vec<RunQuestion> {
    questions().lock().unwrap().iter().filter(|question| question.state == QuestionState::Pending).cloned().collect()
}

/// The question a run is waiting on, if any.
pub fn pending_of(run_id: &str) -> Option<RunQuestion> {
    pending().into_iter().find(|question| question.run_id == run_id)
}

fn settle(question_id: &str, state: QuestionState, answer: &str) -> Option<RunQuestion> {
    let mut questions = questions().lock().unwrap();
    let question = questions.iter_mut().find(|question| question.id == question_id && question.state == QuestionState::Pending)?;
    question.state = state;
    question.answer = answer.to_string();
    Some(question.clone())
}

/// The user's answer to a question still waiting. Returns whether it was.
pub fn answer(app: &tauri::AppHandle, question_id: &str, answer: &str) -> bool {
    let (answer, _) = tidy(answer, &[]);
    if answer.is_empty() {
        return false;
    }
    match settle(question_id, QuestionState::Answered, &answer) {
        Some(question) => {
            let _ = app.emit("agent-question", &question);
            true
        }
        None => false,
    }
}

fn log(app: &tauri::AppHandle, root: &Path, question: &RunQuestion, kind: EventKind, text: &str) {
    let event = RunEvent { kind, text: text.to_string(), at: now_millis(), ..RunEvent::default() };
    runner::log_step(app, root, &question.agent_id, &question.run_id, event);
}

/// `ask_user`, from a run that isn't an orchestration's task: shows the
/// question on the run and waits for the user's answer.
pub fn ask(
    app: &tauri::AppHandle,
    root: &Path,
    token: &str,
    session: &Session,
    question: &str,
    options: &[String],
) -> Result<String, String> {
    let (question, options) = tidy(question, options);
    let started_at = store::get_run_in(root, &session.caller.id, &session.run_id).map_or_else(now_millis, |run| run.started_at);
    let Some(wait) = wait_for(started_at, session.caller.limits.timeout_min, now_millis()) else {
        return Ok(NO_ANSWER.into());
    };
    let asked = RunQuestion {
        id: uuid::Uuid::new_v4().to_string(),
        agent_id: session.caller.id.clone(),
        run_id: session.run_id.clone(),
        question,
        options,
        asked_at: now_millis(),
        expires_at: now_millis() + wait.as_millis() as i64,
        ..RunQuestion::default()
    };
    questions().lock().unwrap().push(asked.clone());
    let _ = app.emit("agent-question", &asked);
    log(app, root, &asked, EventKind::Question, &asked.question);

    let deadline = Instant::now() + wait;
    let settled = loop {
        std::thread::sleep(POLL);
        let current = questions().lock().unwrap().iter().find(|question| question.id == asked.id).cloned();
        match current {
            Some(question) if question.state == QuestionState::Pending => {}
            Some(question) => break question,
            None => break RunQuestion { state: QuestionState::Expired, ..asked.clone() },
        }
        // The run ended (stopped, timed out) or the wait did.
        if team::session(token).is_none() || Instant::now() >= deadline {
            let expired = settle(&asked.id, QuestionState::Expired, "");
            if let Some(expired) = &expired {
                let _ = app.emit("agent-question", expired);
            }
            break expired.unwrap_or_else(|| RunQuestion { state: QuestionState::Expired, ..asked.clone() });
        }
    };
    questions().lock().unwrap().retain(|question| question.id != asked.id);
    let answered = settled.state == QuestionState::Answered;
    log(app, root, &asked, EventKind::Answer, if answered { &settled.answer } else { "" });
    if team::session(token).is_none() {
        return Err("Your run ended.".into());
    }
    Ok(if answered { answer_text(&settled.answer) } else { NO_ANSWER.into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waiting(id: &str, run_id: &str) -> RunQuestion {
        RunQuestion { id: id.into(), run_id: run_id.into(), question: "¿Migro los datos?".into(), ..RunQuestion::default() }
    }

    /// Only the runs the user starts, and an orchestration's.
    #[test]
    fn only_a_run_someone_is_there_for_can_ask() {
        for trigger in [RunTrigger::Manual, RunTrigger::Console, RunTrigger::Orchestrator] {
            assert!(attended(trigger), "{trigger:?}");
        }
        for trigger in [
            RunTrigger::Test,
            RunTrigger::Schedule,
            RunTrigger::Watch,
            RunTrigger::Git,
            RunTrigger::Webhook,
            RunTrigger::Chain,
            RunTrigger::Agent,
            RunTrigger::Message,
        ] {
            assert!(!attended(trigger), "{trigger:?}");
        }
    }

    #[test]
    fn a_question_never_waits_into_the_time_to_finish() {
        let minute = 60_000;
        // Plenty of time left: an orchestration's wait.
        assert_eq!(wait_for(0, 120, 0), Some(WAIT));
        // Ten minutes in all, one gone: seven to wait, two to finish.
        assert_eq!(wait_for(0, 10, minute), Some(Duration::from_secs(7 * 60)));
        // Inside the last two minutes there's nothing to wait for.
        assert_eq!(wait_for(0, 10, 8 * minute), None);
        assert_eq!(wait_for(0, 1, 0), None);
    }

    /// The first answer is the answer; a question that isn't waiting any
    /// more (answered, expired, gone) takes none.
    #[test]
    fn a_waiting_question_is_settled_once() {
        questions().lock().unwrap().push(waiting("q-settle", "run-settle"));
        assert_eq!(pending_of("run-settle").map(|question| question.id), Some("q-settle".into()));
        let settled = settle("q-settle", QuestionState::Answered, "sí").unwrap();
        assert_eq!((settled.state, settled.answer.as_str()), (QuestionState::Answered, "sí"));
        assert!(settle("q-settle", QuestionState::Answered, "no").is_none());
        assert!(settle("q-settle", QuestionState::Expired, "").is_none());
        assert!(pending_of("run-settle").is_none());
        assert!(settle("q-nowhere", QuestionState::Answered, "x").is_none());
        questions().lock().unwrap().retain(|question| question.id != "q-settle");
    }
}
