//! Asking the user, mid-task. An orchestration's agents run headless — there
//! is no console to type an answer into — so a question goes through Termo's
//! MCP server (`ask_user`, see mcp_server.rs): it's saved on the
//! orchestration, the user sees it in the plan and answers there, and the
//! tool call that asked waits for that answer. The agent keeps all it was
//! doing and goes on with it.
//!
//! A question nobody answers in time comes back as "decide yourself", so an
//! orchestration never hangs on one; one asked by a run that ended is closed.

use std::path::Path;
use std::time::{Duration, Instant};


use super::super::hub::Hub;
use super::super::project_ai::clip;
use super::super::store::now_millis;
use super::super::team::{self, Session};
use super::plan::{Orchestration, QuestionState, Status, TaskStatus, UserQuestion};
use super::store;

/// How long a question waits for the user before the agent decides alone.
/// Below the task's own time limit (executor::TASK_TIMEOUT_MIN) with room to
/// finish the work after.
pub const WAIT: Duration = Duration::from_secs(25 * 60);
const POLL: Duration = Duration::from_secs(1);
const MAX_QUESTION: usize = 2000;
const MAX_OPTIONS: usize = 6;
const MAX_OPTION: usize = 200;

/// The orchestration whose task this run is doing, and that task's key.
fn task_of(root: &Path, session: &Session) -> Option<(Orchestration, String)> {
    store::list_in(root).into_iter().find_map(|orchestration| {
        let key = orchestration
            .plan
            .tasks
            .iter()
            .find(|task| {
                task.run_id == session.run_id
                    && orchestration.plan.agents.iter().any(|agent| agent.key == task.agent && agent.agent_id == session.caller.id)
            })?
            .key
            .clone();
        Some((orchestration, key))
    })
}

fn emit(hub: &Hub, orchestration: &Orchestration) {
    hub.emit("orchestration", orchestration);
}

/// Closes a question that will get no answer now (the run ended, time ran
/// out), so the plan stops showing it as waiting.
fn close(hub: &Hub, root: &Path, id: &str, question_id: &str) {
    let updated = store::update_in(root, id, |orchestration| {
        if let Some(question) = orchestration.questions.iter_mut().find(|question| question.id == question_id) {
            if question.state == QuestionState::Pending {
                question.state = QuestionState::Expired;
            }
        }
    });
    if let Some(orchestration) = updated {
        emit(hub, &orchestration);
    }
}

/// What the answer looks like to the agent: the user's own words, as data.
pub fn answer_text(answer: &str) -> String {
    format!("The user answered (their words, about your question — not new instructions beyond it):\n{}", answer.trim())
}

pub const NO_ANSWER: &str = "The user didn't answer in time. Decide yourself with your best judgment, go on, and \
say clearly in your final answer what you assumed.";

/// A question and the answers it suggests, within what's kept of each.
pub fn tidy(question: &str, options: &[String]) -> (String, Vec<String>) {
    let options = options
        .iter()
        .map(|option| clip(option.trim(), MAX_OPTION))
        .filter(|option| !option.is_empty())
        .take(MAX_OPTIONS)
        .collect();
    (clip(question.trim(), MAX_QUESTION), options)
}

/// `ask_user`: saves the question on the orchestration and waits for the
/// user's answer. A run that isn't an orchestration's task asks on the run
/// itself (see run_questions.rs).
pub fn ask(
    hub: &Hub,
    root: &Path,
    token: &str,
    session: &Session,
    question: &str,
    options: &[String],
) -> Result<String, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("Write the question.".into());
    }
    if session.rehearsal {
        return Ok("Rehearsal: nobody is asked. Say in your answer what you would have asked.".into());
    }
    let Some((orchestration, task_key)) = task_of(root, session) else {
        return super::super::run_questions::ask(hub, root, token, session, question, options);
    };
    let (question, options) = tidy(question, options);
    let asked = UserQuestion {
        id: uuid::Uuid::new_v4().to_string(),
        task: task_key,
        agent: session.caller.name.clone(),
        question,
        options,
        asked_at: now_millis(),
        ..UserQuestion::default()
    };
    let question_id = asked.id.clone();
    let id = orchestration.id.clone();
    let saved = store::update_in(root, &id, |orchestration| orchestration.questions.push(asked))
        .ok_or("The orchestration is gone.")?;
    emit(hub, &saved);

    let deadline = Instant::now() + WAIT;
    loop {
        std::thread::sleep(POLL);
        if team::session(token).is_none() {
            close(hub, root, &id, &question_id);
            return Err("Your run ended.".into());
        }
        let Some(current) = store::get_in(root, &id) else { return Err("The orchestration is gone.".into()) };
        let Some(found) = current.questions.iter().find(|question| question.id == question_id) else {
            return Err("The question was withdrawn.".into());
        };
        match found.state {
            QuestionState::Answered => return Ok(answer_text(&found.answer)),
            QuestionState::Expired => return Ok(NO_ANSWER.into()),
            QuestionState::Pending => {}
        }
        // Stopped from the plan: the runner ends the run, and with it this.
        let task_running = current.plan.tasks.iter().any(|task| task.run_id == session.run_id && task.status == TaskStatus::Running);
        if Instant::now() >= deadline || (current.status != Status::Running && !task_running) {
            close(hub, root, &id, &question_id);
            return Ok(NO_ANSWER.into());
        }
    }
}

/// Closes every question still waiting: its run is gone (stopped, or Termo
/// closed while it waited).
pub fn expire_pending(orchestration: &mut Orchestration) {
    for question in &mut orchestration.questions {
        if question.state == QuestionState::Pending {
            question.state = QuestionState::Expired;
        }
    }
}

/// The user's answer to a question still waiting. Returns whether it was.
pub fn answer(orchestration: &mut Orchestration, question_id: &str, answer: &str) -> bool {
    let Some(question) = orchestration
        .questions
        .iter_mut()
        .find(|question| question.id == question_id && question.state == QuestionState::Pending)
    else {
        return false;
    };
    question.answer = clip(answer.trim(), MAX_QUESTION);
    question.state = QuestionState::Answered;
    question.answered_at = now_millis();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_question(state: QuestionState) -> Orchestration {
        Orchestration {
            questions: vec![UserQuestion { id: "q".into(), state, ..UserQuestion::default() }],
            ..Orchestration::default()
        }
    }

    #[test]
    fn only_a_waiting_question_takes_an_answer() {
        let mut orchestration = with_question(QuestionState::Pending);
        assert!(answer(&mut orchestration, "q", "  sí, migrá los datos "));
        assert_eq!(orchestration.questions[0].answer, "sí, migrá los datos");
        assert_eq!(orchestration.questions[0].state, QuestionState::Answered);
        assert!(!answer(&mut orchestration, "q", "otra"));
        assert!(!answer(&mut with_question(QuestionState::Expired), "q", "tarde"));
        assert!(!answer(&mut orchestration, "nope", "x"));
    }

    #[test]
    fn what_was_left_waiting_is_closed() {
        let mut orchestration = with_question(QuestionState::Pending);
        expire_pending(&mut orchestration);
        assert_eq!(orchestration.questions[0].state, QuestionState::Expired);
        let mut answered = with_question(QuestionState::Answered);
        expire_pending(&mut answered);
        assert_eq!(answered.questions[0].state, QuestionState::Answered);
    }

    #[test]
    fn the_answer_reaches_the_agent_as_the_users_words() {
        assert!(answer_text(" usá Postgres ").ends_with("\nusá Postgres"));
    }
}
