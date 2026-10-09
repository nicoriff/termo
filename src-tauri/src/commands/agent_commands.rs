use std::path::Path;

use tauri::State;

use crate::agents::draft::{self, Draft};
use crate::agents::events::RunEvent;
use crate::agents::git_hooks;
use crate::agents::github::{self, GithubPreview};
use crate::agents::webhook;
use crate::agents::integrations::{self, AgentMcpRequest, AgentMcpServer, CustomMcpRequest};
use crate::agents::scheduler;
use crate::agents::portable::{self, ImportReport};
use crate::agents::projects;
use crate::agents::run_questions::{self, RunQuestion};
use crate::agents::runner::Runner;
use crate::agents::secrets;
use crate::agents::models;
use crate::agents::spec::{AgentSpec, Engine};
use crate::agents::store::{self, RunRecord, RunTrigger};
use crate::agents::team;
use crate::i18n::{tr, Msg};
use crate::state::AppState;

pub(crate) fn root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    store::root(app).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))
}

/// Runs a read of the agents' files off the main thread. A command that
/// isn't `async` runs on it: the read would wait behind whatever the window
/// is doing, and hold the window up in turn.
async fn off_main<T: Send + 'static>(read: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(read).await.map_err(|err| err.to_string())
}

/// Saves an agent the way every save has to go: checked (see spec::check),
/// its git hook first (if it
/// can't go in, nothing is saved half-done), then the definition, then a
/// version of the project it left and the one it's in.
pub(crate) fn save_agent(root: &Path, spec: AgentSpec) -> Result<Vec<AgentSpec>, String> {
    crate::agents::spec::check(&spec)?;
    let before = store::get_in(root, &spec.id);
    git_hooks::sync(before.as_ref(), Some(&spec))?;
    let project_id = spec.project_id.clone();
    let list = store::save_in(root, spec).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))?;
    if let Some(old) = before.filter(|old| !old.project_id.is_empty() && old.project_id != project_id) {
        projects::snapshot(root, &old.project_id);
    }
    if !project_id.is_empty() {
        projects::snapshot(root, &project_id);
    }
    Ok(list)
}

/// Deletes an agent with everything that's its own — history, secrets, git
/// hook — and every mention of it: contacts, chains, a project's entry.
pub(crate) fn delete_agent(root: &Path, runner: &Runner, id: &str) -> Result<Vec<AgentSpec>, String> {
    runner.stop(id);
    let project_id = store::get_in(root, id).map(|spec| {
        secrets::delete_all(id, &spec.secrets);
        let _ = git_hooks::sync(Some(&spec), None);
        spec.project_id
    });
    for mut other in store::list_in(root) {
        let before = (other.collaboration.contacts.len(), other.triggers.after.is_some());
        other.collaboration.contacts.retain(|contact| contact != id);
        if other.triggers.after.as_ref().is_some_and(|after| after.agent_id == id) {
            other.triggers.after = None;
        }
        if before != (other.collaboration.contacts.len(), other.triggers.after.is_some()) {
            let _ = store::save_in(root, other);
        }
    }
    let list = store::delete_in(root, id).map_err(|err| tr(Msg::AgentSaveFailed(&err.to_string())))?;
    if let Some(project_id) = project_id.filter(|project_id| !project_id.is_empty()) {
        if let Some(mut project) = projects::get_in(root, &project_id).filter(|project| project.entry_agent_id == id) {
            project.entry_agent_id = String::new();
            let _ = projects::save_in(root, project);
        }
        projects::snapshot(root, &project_id);
    }
    Ok(list)
}

#[tauri::command]
pub fn agents_list(app: tauri::AppHandle) -> Result<Vec<AgentSpec>, String> {
    Ok(store::list_in(&root(&app)?))
}

/// Returns the full list after saving, so the caller refreshes in one call.
#[tauri::command]
pub fn agents_save(app: tauri::AppHandle, spec: AgentSpec) -> Result<Vec<AgentSpec>, String> {
    save_agent(&root(&app)?, spec)
}

#[tauri::command]
pub fn agents_delete(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<Vec<AgentSpec>, String> {
    delete_agent(&root(&app)?, &state.agents, &id)
}

/// The models the engine's CLI offers this account.
#[tauri::command]
pub fn agent_models(engine: Engine) -> Vec<models::ModelOption> {
    models::available(engine)
}

/// Every run of a conversation between agents, oldest first.
#[tauri::command]
pub async fn agent_conversation(app: tauri::AppHandle, conversation_id: String) -> Result<Vec<RunRecord>, String> {
    let root = root(&app)?;
    off_main(move || store::conversation_in(&root, &conversation_id)).await
}

/// A board: a team project's own, or (no project) the one every agent
/// outside a team shares.
#[tauri::command]
pub fn agents_board(app: tauri::AppHandle, project_id: Option<String>) -> Result<team::Board, String> {
    Ok(team::read_board(&projects::board_path(&root(&app)?, project_id.as_deref())))
}

/// Removes one note from a board.
#[tauri::command]
pub fn agents_board_remove(app: tauri::AppHandle, project_id: Option<String>, key: String) -> Result<team::Board, String> {
    let path = projects::board_path(&root(&app)?, project_id.as_deref());
    team::write_board(&path, &key, "", "").map_err(|err| tr(Msg::AgentSaveFailed(&err)))?;
    Ok(team::read_board(&path))
}

/// Runs the spec as given rather than the saved one, so the wizard can try
/// out an agent before it's saved.
#[tauri::command]
pub fn agent_run(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    spec: AgentSpec,
    trigger: RunTrigger,
) -> Result<RunRecord, String> {
    state.agents.start(&app, &spec, trigger, None)
}

/// The questions agents' runs are waiting on the user for (see
/// agents/run_questions.rs).
#[tauri::command]
pub fn agent_questions() -> Vec<RunQuestion> {
    run_questions::pending()
}

/// The user's answer to one of them. False when it wasn't waiting any more.
#[tauri::command]
pub fn agent_answer_question(app: tauri::AppHandle, question_id: String, answer: String) -> bool {
    run_questions::answer(&app, &question_id, &answer)
}

#[tauri::command]
pub fn agent_stop(state: State<'_, AppState>, agent_id: String) {
    state.agents.stop(&agent_id);
}

#[tauri::command]
pub fn agents_running(state: State<'_, AppState>) -> Vec<RunRecord> {
    state.agents.running()
}

/// When each scheduled agent runs next on its own (Unix milliseconds).
#[tauri::command]
pub fn agents_next_runs(app: tauri::AppHandle) -> Result<std::collections::HashMap<String, i64>, String> {
    let root = root(&app)?;
    Ok(store::list_in(&root)
        .iter()
        .filter_map(|spec| scheduler::agent_next_run(&root, spec).map(|at| (spec.id.clone(), at)))
        .collect())
}

/// Newest first.
#[tauri::command]
pub async fn agent_runs(app: tauri::AppHandle, agent_id: String) -> Result<Vec<RunRecord>, String> {
    let root = root(&app)?;
    off_main(move || store::list_runs_in(&root, &agent_id)).await
}

/// "Build it with AI": a draft of the whole agent from a description.
#[tauri::command]
pub async fn agent_draft(app: tauri::AppHandle, description: String, work_dir: String) -> Result<Draft, String> {
    let scratch = root(&app)?.join(".draft");
    tauri::async_runtime::spawn_blocking(move || draft::draft(&scratch, &description, &work_dir))
        .await
        .map_err(|err| tr(Msg::AgentDraftFailed(&err.to_string())))?
}

/// An MCP server from the registry, ready to add to the agent, with its
/// secrets already in the keychain.
#[tauri::command]
pub fn agent_mcp_server(request: AgentMcpRequest) -> Result<AgentMcpServer, String> {
    integrations::build(request)
}

/// Drops secrets the agent no longer uses (an integration removed, a new
/// agent cancelled).
#[tauri::command]
pub fn agent_forget_secrets(agent_id: String, names: Vec<String>) {
    secrets::delete_all(&agent_id, &names);
}

/// What deploying to GitHub Actions would write, without writing it.
#[tauri::command]
pub fn agent_github_preview(spec: AgentSpec) -> Result<GithubPreview, String> {
    github::preview(&spec)
}

/// Writes the agent's workflow into its repository; returns the file's path.
#[tauri::command]
pub fn agent_github_write(spec: AgentSpec) -> Result<String, String> {
    github::write(&spec)
}

/// A connector of the user's own, ready to add to the agent.
#[tauri::command]
pub fn agent_custom_mcp(request: CustomMcpRequest) -> Result<AgentMcpServer, String> {
    integrations::build_custom(request)
}

/// Last month's downloads of an MCP server's package (npm or PyPI), when
/// its registry says.
#[tauri::command]
pub async fn mcp_downloads(registry_type: String, identifier: String) -> Option<u64> {
    tauri::async_runtime::spawn_blocking(move || crate::marketplace::downloads::last_month(&registry_type, &identifier))
        .await
        .ok()
        .flatten()
}

/// Writes the agents with these ids to a file.
#[tauri::command]
pub fn agents_export(app: tauri::AppHandle, ids: Vec<String>, path: String) -> Result<(), String> {
    let root = root(&app)?;
    let specs: Vec<AgentSpec> = ids.iter().filter_map(|id| store::get_in(&root, id)).collect();
    std::fs::write(&path, portable::export(&specs, store::now_seconds()))
        .map_err(|err| tr(Msg::AgentFileFailed(&path, &err.to_string())))
}

/// Saves the agents in these files as new ones; returns the full list and
/// what the import couldn't bring along.
#[tauri::command]
pub fn agents_import(app: tauri::AppHandle, paths: Vec<String>) -> Result<(Vec<AgentSpec>, ImportReport), String> {
    let root = root(&app)?;
    let mut report = ImportReport { imported: vec![], moved_to_own_folder: vec![], needs_secrets: vec![], review: vec![] };
    for path in &paths {
        let text = std::fs::read_to_string(path).map_err(|err| tr(Msg::AgentFileFailed(path, &err.to_string())))?;
        let existing: Vec<String> = store::list_in(&root).into_iter().map(|spec| spec.name).collect();
        let (agents, part) = portable::import(&text, &existing, || uuid::Uuid::new_v4().to_string())?;
        for spec in agents {
            save_agent(&root, spec)?;
        }
        report.imported.extend(part.imported);
        report.moved_to_own_folder.extend(part.moved_to_own_folder);
        report.needs_secrets.extend(part.needs_secrets);
        report.review.extend(part.review);
    }
    Ok((store::list_in(&root), report))
}

/// The local webhook's port, and whether it could take it.
#[tauri::command]
pub fn agents_webhook() -> serde_json::Value {
    serde_json::json!({ "port": webhook::PORT, "listening": webhook::listening() })
}

/// Which of these secrets have no value on this machine.
#[tauri::command]
pub fn agent_missing_secrets(agent_id: String, names: Vec<String>) -> Vec<String> {
    portable::missing_secrets(&agent_id, &names)
}

/// Stores one secret's value in the keychain.
#[tauri::command]
pub fn agent_set_secret(agent_id: String, name: String, value: String) -> Result<(), String> {
    if !crate::agents::spec::is_valid_id(&agent_id) {
        return Err(tr(Msg::AgentSaveFailed("invalid agent id")));
    }
    secrets::set(&agent_id, &name, &value).map_err(|err| tr(Msg::AgentSecretFailed(&err)))
}

#[tauri::command]
pub async fn agent_run_log(app: tauri::AppHandle, agent_id: String, run_id: String) -> Result<Vec<RunEvent>, String> {
    let root = root(&app)?;
    off_main(move || store::read_log_in(&root, &agent_id, &run_id)).await
}

/// Adds the user's agents to every installed CLI's own settings (`enable`),
/// so consoles opened anywhere reach them, or takes them out again. Says
/// how it went for each CLI.
#[tauri::command]
pub async fn agents_console_global(
    app: tauri::AppHandle,
    enable: bool,
) -> Result<Vec<crate::agents::console_bridge::Registration>, String> {
    let root = root(&app)?;
    tauri::async_runtime::spawn_blocking(move || crate::agents::console_bridge::register_global(&root, enable))
        .await
        .map_err(|err| err.to_string())
}

/// A new long-lived token for consoles outside Termo: the old one stops
/// working, and — when that's on — every CLI's settings get the new one.
#[tauri::command]
pub async fn agents_console_rotate(app: tauri::AppHandle) -> Result<Vec<crate::agents::console_bridge::Registration>, String> {
    let root = root(&app)?;
    let global = crate::config::store::load(&app).agents_from_any_console;
    tauri::async_runtime::spawn_blocking(move || {
        crate::agents::console_bridge::rotate(&root)?;
        Ok(if global { crate::agents::console_bridge::register_global(&root, true) } else { Vec::new() })
    })
    .await
    .map_err(|err| err.to_string())?
}
