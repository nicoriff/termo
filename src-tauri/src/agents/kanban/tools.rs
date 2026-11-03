//! What a column's agent gets in Termo's MCP server (see mcp_server.rs) while
//! it works on a task: writing to the task's log, the reviewer's verdict, and
//! the report at the end. The task is the one its run was started for: the
//! session's run id, found on the board its project belongs to.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::super::hub::Hub;
use super::super::projects;
use super::super::store::now_millis;
use super::super::team::Session;
use super::driver::emit;
use super::model::{Actor, KanbanBoard, LogKind, Outcome, Role, Success, Verdict};
use super::store;

pub const TOOLS: &[&str] = &["kanban_log", "kanban_review", "kanban_finish"];
/// The driver saves the run's id on the task right after the run starts: a
/// first call can come just before.
const FIND_WAIT: Duration = Duration::from_secs(5);
const MAX_TEXT: usize = 8000;

/// The board the caller's project runs, and which column agent it is.
fn board_of(root: &Path, session: &Session) -> Option<(KanbanBoard, Role)> {
    let project = projects::get_in(root, &session.caller.project_id)?;
    if project.kanban_id.is_empty() {
        return None;
    }
    let board = store::get_in(root, &project.kanban_id)?;
    let role = board.agents.role_of(&session.caller.id)?;
    Some((board, role))
}

/// The tools for this run: none for an agent that isn't a board's.
pub fn tools(root: &Path, session: &Session) -> Vec<Value> {
    let Some((_, role)) = board_of(root, session) else { return Vec::new() };
    let mut tools = vec![json!({
        "name": "kanban_log",
        "description": "Adds an entry to the task's log on the kanban board: a decision you made, a step done, a problem found. The user and the next agents read it. Short and concrete.",
        "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] },
    })];
    let success = json!({
        "type": "string",
        "enum": ["full", "partial", "failed"],
        "description": "full: every success criterion is met; partial: some are; failed: the task couldn't be done.",
    });
    match role {
        Role::Review => tools.push(json!({
            "name": "kanban_review",
            "description": "Your verdict on the work, required before you end. approve: it meets the task and its success criteria. changes: it doesn't yet — the notes go back to the work agent, so say precisely what to change.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "verdict": { "type": "string", "enum": ["approve", "changes"] },
                    "notes": { "type": "string", "description": "What you checked and, with changes, what has to change." },
                    "summary": { "type": "string", "description": "With approve: what was done, for the user, in a few lines." },
                    "success": success,
                },
                "required": ["verdict", "notes"],
            },
        })),
        Role::Work | Role::Summary => tools.push(json!({
            "name": "kanban_finish",
            "description": if role == Role::Work {
                "Reports your work on the task as you end: what you did, and whether it's done. With success \"failed\", say in notes what stopped you; the task waits for the user."
            } else {
                "Writes the task's final summary: what was done and how well it meets its success criteria."
            },
            "inputSchema": {
                "type": "object",
                "properties": {
                    "summary": { "type": "string" },
                    "success": success,
                    "notes": { "type": "string", "description": "What's left, risks, what the user should check." },
                },
                "required": ["summary", "success"],
            },
        })),
    }
    tools
}

pub fn is_tool(name: &str) -> bool {
    TOOLS.contains(&name)
}

fn clipped(args: &Value, key: &str) -> String {
    let text = args.get(key).and_then(Value::as_str).unwrap_or_default().trim();
    text.chars().take(MAX_TEXT).collect()
}

fn success_of(args: &Value) -> Success {
    match args.get("success").and_then(Value::as_str) {
        Some("partial") => Success::Partial,
        Some("failed") => Success::Failed,
        _ => Success::Full,
    }
}

/// Runs a kanban tool for this run.
pub fn call(hub: &Hub, session: &Session, name: &str, args: &Value) -> Result<String, String> {
    let root = hub.root();
    if session.rehearsal {
        return Ok("Rehearsal: nothing was written.".into());
    }
    let Some((board, role)) = board_of(root, session) else {
        return Err(format!("There's no tool called \"{name}\"."));
    };
    // The run's task, once the driver has saved which one it is.
    let deadline = Instant::now() + FIND_WAIT;
    let task_id = loop {
        let board = store::get_in(root, &board.id).ok_or("The board no longer exists.")?;
        if let Some(task) = board.tasks.iter().find(|task| task.run_id == session.run_id) {
            break task.id.clone();
        }
        if Instant::now() >= deadline {
            return Err("This run isn't working on a task of the board any more.".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let column = board.agents.get(role).cloned().unwrap_or_default();
    let actor = Actor::Agent { role, engine: column.engine, model: column.model };
    let run_id = session.run_id.clone();
    let changed = store::try_update_in(root, &board.id, |board| {
        let task = board.task_mut(&task_id).filter(|task| task.run_id == run_id).ok_or("The task was moved: stop working on it.")?;
        let now = now_millis();
        match name {
            "kanban_log" => {
                let text = clipped(args, "text");
                if text.is_empty() {
                    return Err("Write the entry.".into());
                }
                task.note(now, actor, LogKind::Note, text);
                Ok("Logged.")
            }
            "kanban_review" if role == Role::Review => {
                let verdict = match args.get("verdict").and_then(Value::as_str) {
                    Some("approve") => Verdict::Approve,
                    Some("changes") => Verdict::Changes,
                    _ => return Err("verdict is \"approve\" or \"changes\".".into()),
                };
                let notes = clipped(args, "notes");
                if verdict == Verdict::Changes && notes.is_empty() {
                    return Err("Say in notes what has to change.".into());
                }
                task.verdict = Some(verdict);
                task.review_notes = notes.clone();
                let summary = clipped(args, "summary");
                task.report = (verdict == Verdict::Approve && !summary.is_empty())
                    .then(|| Outcome { summary, success: success_of(args), notes, at: now });
                Ok("Verdict saved. End your turn now.")
            }
            "kanban_finish" if role != Role::Review => {
                let summary = clipped(args, "summary");
                if summary.is_empty() {
                    return Err("Write the summary.".into());
                }
                task.report = Some(Outcome { summary, success: success_of(args), notes: clipped(args, "notes"), at: now });
                Ok("Saved. End your turn now.")
            }
            _ => Err(format!("There's no tool called \"{name}\".")),
        }
    });
    match changed {
        None => Err("The board no longer exists.".into()),
        Some(Err(error)) => Err(error),
        Some(Ok((board, text))) => {
            emit(hub, &board);
            Ok(text.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::tests::{board, task};
    use super::super::model::Column;
    use super::*;
    use crate::agents::projects::Project;
    use crate::agents::spec::AgentSpec;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("termo-kanban-tools-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn session(agent: &str) -> Session {
        let caller = AgentSpec { id: agent.into(), project_id: "p".into(), ..AgentSpec::default() };
        Session { caller, run_id: "run".into(), conversation_id: "run".into(), path: vec![], rehearsal: false }
    }

    fn setup(name: &str) -> (PathBuf, Hub) {
        let root = temp_root(name);
        projects::save_in(&root, Project { id: "p".into(), kanban_id: "b".into(), ..Project::default() }).unwrap();
        let mut board = board(vec![task("a", Column::Review, &[])]);
        board.agents.work.agent_ids = vec!["w".into()];
        board.agents.review.agent_ids = vec!["r".into()];
        board.tasks[0].run_id = "run".into();
        store::save_in(&root, board).unwrap();
        let hub = Hub::new(root.clone(), Arc::new(crate::agents::hub::Silent));
        (root, hub)
    }

    #[test]
    fn each_column_gets_its_tools() {
        let (root, _) = setup("list");
        let names = |agent: &str| -> Vec<String> {
            tools(&root, &session(agent)).iter().map(|tool| tool["name"].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(names("r"), ["kanban_log", "kanban_review"]);
        assert_eq!(names("w"), ["kanban_log", "kanban_finish"]);
        assert!(names("stranger").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_reviewer_leaves_its_verdict_on_its_task() {
        let (root, hub) = setup("review");
        let reviewer = session("r");
        assert!(call(&hub, &reviewer, "kanban_review", &json!({ "verdict": "changes" })).is_err());
        call(&hub, &reviewer, "kanban_review", &json!({ "verdict": "changes", "notes": "add a test" })).unwrap();
        call(&hub, &reviewer, "kanban_log", &json!({ "text": "ran the suite" })).unwrap();
        let task = store::get_in(&root, "b").unwrap().tasks.remove(0);
        assert_eq!(task.verdict, Some(Verdict::Changes));
        assert_eq!(task.review_notes, "add a test");
        assert_eq!(task.log.last().unwrap().text, "ran the suite");
        // The work agent can't review.
        assert!(call(&hub, &session("w"), "kanban_review", &json!({ "verdict": "approve", "notes": "x" })).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
