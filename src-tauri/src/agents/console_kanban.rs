//! Kanban boards from the user's consoles, through the same MCP server as
//! their agents (see console_bridge.rs): a console's CLI lists the boards,
//! reads their tasks and adds or edits tasks — what the user asks it to
//! write down while working. It's the same board the Kanban tab shows:
//! every change reaches the windows as it happens, and each one is logged
//! as coming from a console.
//!
//! Nothing at all unless the user turned it on (Preferences → Agents →
//! Consoles). A console writes tasks; it never moves them along or switches
//! a board on: that stays with the user. What it writes waits in Not ready,
//! and it edits only what's there: a console can be led by what it reads,
//! and no task it wrote runs, with the board's permissions, until the user
//! moves it to To do.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{json, Value};

use super::console_bridge::clip;
use super::kanban::model::{blockers, next_up, Actor, Column, KanbanBoard, KanbanTask, MAX_DEPENDENCIES, MAX_TAGS};
use super::kanban::ops::{self as kanban, TaskInput};
use super::kanban::store;
use super::ops::Agents;
use crate::config::AppConfig;

/// Whether the user lets consoles add tasks to boards
/// (`AppConfig.kanban_from_consoles`).
static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn configure(config: &AppConfig) {
    ENABLED.store(config.kanban_from_consoles, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

const TOOLS: [&str; 4] = ["kanban_list_boards", "kanban_list_tasks", "kanban_add_task", "kanban_update_task"];
/// What a task's description shows at most in a listing.
const LISTED_SPEC: usize = 300;

pub fn is_tool(name: &str) -> bool {
    TOOLS.contains(&name)
}

pub fn tools() -> Vec<Value> {
    let board = json!({ "type": "string", "description": "The board's name or id, as kanban_list_boards gives it." });
    let fields = json!({
        "title": { "type": "string", "description": "Short and specific." },
        "spec": { "type": "string", "description": "What the task is about, with everything an agent needs to do it without this conversation: the context, what to change, constraints." },
        "success_criteria": { "type": "string", "description": "How to tell it's done: what must be true, what must pass." },
        "depends_on": { "type": "array", "items": { "type": "string" }, "description": "Tasks it waits for, by number (\"3\" or \"#3\")." },
        "tags": { "type": "array", "items": { "type": "string" }, "description": "Short labels, like \"bug\" or \"frontend\". Reuse the board's existing tags when they fit." },
    });
    let mut add = fields.clone();
    add["board"] = board.clone();
    let mut update = fields;
    update["board"] = board.clone();
    update["task"] = json!({ "type": "string", "description": "The task's number (\"3\" or \"#3\")." });
    vec![
        json!({
            "name": "kanban_list_boards",
            "description": "Lists the user's kanban boards in Termo: each one's folder, whether it's on (its agents take tasks) and how many tasks are in each column.",
            "inputSchema": { "type": "object", "properties": {} },
        }),
        json!({
            "name": "kanban_list_tasks",
            "description": "Lists the tasks of a board by column, with what each waits for and which come next.",
            "inputSchema": { "type": "object", "properties": { "board": board }, "required": ["board"] },
        }),
        json!({
            "name": "kanban_add_task",
            "description": "Adds a task to one of the user's kanban boards, in Not ready: the user looks it over and moves it to To do for the board's agent to take. Write it so an agent can do it alone: it won't see this conversation.",
            "inputSchema": { "type": "object", "properties": add, "required": ["board", "title", "spec"] },
        }),
        json!({
            "name": "kanban_update_task",
            "description": "Changes a task in Not ready: its title, description, success criteria, tags or dependencies. Fields left out stay as they are. Tasks the user already moved on can't be changed from here.",
            "inputSchema": { "type": "object", "properties": update, "required": ["board", "task"] },
        }),
    ]
}

fn column_name(column: Column) -> &'static str {
    match column {
        Column::NotReady => "Not ready",
        Column::Todo => "To do",
        Column::InProgress => "In progress",
        Column::Review => "Review",
        Column::Done => "Done",
    }
}

const COLUMNS: [Column; 5] = [Column::NotReady, Column::Todo, Column::InProgress, Column::Review, Column::Done];

fn find(agents: &Agents, wanted: &str) -> Result<KanbanBoard, String> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return Err("Say which board (its name or id).".into());
    }
    let boards = store::list_in(agents.hub.root());
    boards
        .iter()
        .find(|board| board.id == wanted)
        .or_else(|| boards.iter().find(|board| board.name.eq_ignore_ascii_case(wanted)))
        .cloned()
        .ok_or_else(|| format!("No board called \"{wanted}\". Call kanban_list_boards to see them."))
}

fn list_boards(boards: &[KanbanBoard]) -> String {
    if boards.is_empty() {
        return "The user has no kanban boards yet. They create them in Termo (Kanban in the activity bar).".into();
    }
    boards
        .iter()
        .map(|board| {
            let counts: Vec<String> = COLUMNS
                .iter()
                .map(|column| format!("{} {}", column_name(*column), board.tasks.iter().filter(|task| task.column == *column).count()))
                .collect();
            let state = if board.enabled { "on" } else { "off" };
            format!("- {} (id {}, {state}) — folder {}\n  {}", board.name, board.id, board.work_dir, counts.join(" · "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn describe_task(board: &KanbanBoard, task: &KanbanTask, next: &[String]) -> String {
    let mut line = format!("  #{} {}", task.number, task.title);
    if !task.tags.is_empty() {
        line.push_str(&format!(" [{}]", task.tags.join(", ")));
    }
    let waits = blockers(board, task);
    if !waits.is_empty() {
        let list: Vec<String> = waits.iter().map(|number| format!("#{number}")).collect();
        line.push_str(&format!(" (waits for {})", list.join(", ")));
    }
    if let Some(place) = next.iter().position(|id| *id == task.id) {
        line.push_str(&format!(" (next #{})", place + 1));
    }
    if task.is_running() {
        line.push_str(" (an agent is on it)");
    }
    if !task.needs_you.is_empty() {
        line.push_str(" (waiting for the user)");
    }
    if !task.spec.trim().is_empty() {
        line.push_str(&format!("\n    {}", clip(task.spec.trim(), LISTED_SPEC).replace('\n', "\n    ")));
    }
    line
}

fn list_tasks(board: &KanbanBoard) -> String {
    let next = next_up(board);
    let mut out = format!("{} — {}", board.name, if board.enabled { "on" } else { "off" });
    let mut in_use: Vec<&str> = Vec::new();
    for tag in board.tasks.iter().flat_map(|task| task.tags.iter()) {
        if !in_use.iter().any(|known| known.eq_ignore_ascii_case(tag)) {
            in_use.push(tag);
        }
    }
    if !in_use.is_empty() {
        out.push_str(&format!("\nTags in use: {}", in_use.join(", ")));
    }
    for column in COLUMNS {
        let tasks: Vec<&KanbanTask> = board.tasks.iter().filter(|task| task.column == column).collect();
        out.push_str(&format!("\n\n{} ({})", column_name(column), tasks.len()));
        for task in tasks {
            out.push('\n');
            out.push_str(&describe_task(board, task, &next));
        }
    }
    out
}

/// Task numbers a console gave, as ids of the board.
fn dependencies(board: &KanbanBoard, args: &Value) -> Result<Option<Vec<String>>, String> {
    let Some(list) = args.get("depends_on").and_then(Value::as_array) else { return Ok(None) };
    if list.len() > MAX_DEPENDENCIES {
        return Err(format!("A task can wait for {MAX_DEPENDENCIES} others at most."));
    }
    let mut ids = Vec::new();
    for item in list {
        let reference = item.as_str().map(str::to_string).or_else(|| item.as_u64().map(|number| number.to_string())).unwrap_or_default();
        let task = board.find(&reference).ok_or_else(|| format!("There's no task {reference} on this board."))?;
        ids.push(task.id.clone());
    }
    Ok(Some(ids))
}

/// The tags a console gave, when it gave any.
fn tags(args: &Value) -> Option<Vec<String>> {
    let list = args.get("tags")?.as_array()?;
    Some(list.iter().filter_map(Value::as_str).take(MAX_TAGS).map(str::to_string).collect())
}

/// One tool call from a console.
pub fn call(agents: &Agents, name: &str, args: &Value) -> Result<String, String> {
    if !enabled() {
        return Err("Kanban boards aren't available from consoles. The user can allow them in Termo: Preferences → Agents → Consoles.".into());
    }
    let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or_default().trim().to_string();
    match name {
        "kanban_list_boards" => Ok(list_boards(&store::list_in(agents.hub.root()))),
        "kanban_list_tasks" => Ok(list_tasks(&find(agents, &text("board"))?)),
        "kanban_add_task" => {
            let board = find(agents, &text("board"))?;
            let input = TaskInput {
                id: String::new(),
                title: text("title"),
                spec: text("spec"),
                success_criteria: text("success_criteria"),
                depends_on: dependencies(&board, args)?.unwrap_or_default(),
                tags: tags(args).unwrap_or_default(),
                column: Some(Column::NotReady),
            };
            let (board, task) = kanban::upsert_task(agents, &board.id, input, Actor::Console)?;
            Ok(format!(
                "Added #{} \"{}\" to {}, in Not ready: the user moves it to To do when it's ready.",
                task.number, task.title, board.name
            ))
        }
        "kanban_update_task" => {
            let board = find(agents, &text("board"))?;
            let task = board.find(&text("task")).ok_or_else(|| format!("There's no task {} on this board.", text("task")))?;
            if task.column != Column::NotReady {
                return Err(format!(
                    "#{} is in {}: only tasks in Not ready can be changed from a console. Ask the user to change it in Termo.",
                    task.number,
                    column_name(task.column)
                ));
            }
            let keep = |key: &str, current: &str| if args.get(key).is_some() { text(key) } else { current.to_string() };
            let input = TaskInput {
                id: task.id.clone(),
                title: keep("title", &task.title),
                spec: keep("spec", &task.spec),
                success_criteria: keep("success_criteria", &task.success_criteria),
                depends_on: dependencies(&board, args)?.unwrap_or_else(|| task.depends_on.clone()),
                tags: tags(args).unwrap_or_else(|| task.tags.clone()),
                column: None,
            };
            let (board, task) = kanban::upsert_task(agents, &board.id, input, Actor::Console)?;
            Ok(format!("Updated #{} \"{}\" on {}.", task.number, task.title, board.name))
        }
        _ => Err(format!("There's no tool called \"{name}\".")),
    }
}

#[cfg(test)]
mod tests {
    use super::super::kanban::model::tests::{board, task};
    use super::*;

    #[test]
    fn a_listing_shows_columns_what_waits_and_what_is_next() {
        let board = board(vec![task("a", Column::Todo, &["b"]), task("b", Column::Todo, &[]), task("c", Column::Done, &[])]);
        let text = list_tasks(&board);
        assert!(text.contains("To do (2)"));
        assert!(text.contains("#1 do a (waits for #2)"));
        assert!(text.contains("#2 do b (next #1)"));
        assert!(text.contains("Done (1)"));
    }

    #[test]
    fn too_many_dependencies_are_refused() {
        let board = board(vec![task("a", Column::Todo, &[])]);
        let many: Vec<u32> = (0..=MAX_DEPENDENCIES as u32).collect();
        assert!(dependencies(&board, &json!({ "depends_on": many })).is_err());
    }

    #[test]
    fn numbers_become_ids() {
        let board = board(vec![task("a", Column::Todo, &[]), task("b", Column::Todo, &[])]);
        let ids = dependencies(&board, &json!({ "depends_on": ["#2", 1] })).unwrap().unwrap();
        assert_eq!(ids, ["b", "a"]);
        assert!(dependencies(&board, &json!({ "depends_on": ["9"] })).is_err());
        assert!(dependencies(&board, &json!({})).unwrap().is_none());
    }
}
